#![cfg(feature = "wgpu_bindings")]

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::{fs, path::Path};

use wesl::ModulePath;
use wesl::Mangler;
use wgsl_to_wgpu::WriteOptions;

use crate::WeslBuildExtension;

/// Generate bindings for your wgsl/wesl with wgpu_to_wgsl
///
/// Note this will set the `ManglerKind` to `Escape`
pub struct WgpuBindingsExtension<W: Write> {
    /// The path to output the rust bindings for shaders
    binding_root_path: &'static str,
    /// The courrent module
    bindings_mod_file: W,
    /// The courrent modules path
    bindings_mod_path: PathBuf,
}

impl WgpuBindingsExtension<BufWriter<fs::File>> {
    pub fn new(binding_root_path: &'static str) -> Result<Self, std::io::Error> {
        let bindings_mod_path = Path::new(binding_root_path).join("mod.rs");
        println!("root: {}", bindings_mod_path.display());

        Ok(Self {
            binding_root_path,
            bindings_mod_file: BufWriter::new(fs::File::create(
                &bindings_mod_path,
            )?),
            bindings_mod_path,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WgpuBindingsError {
    #[error(transparent)]
    IoErr(#[from] std::io::Error),
    #[error(transparent)]
    CreateBindingsModuleErr(#[from] wgsl_to_wgpu::CreateModuleError),
}

impl<WeslResolver: wesl::Resolver> WeslBuildExtension<WeslResolver> for WgpuBindingsExtension<BufWriter<fs::File>> {
    fn name<'n>(&self) -> std::borrow::Cow<'n, str> {
        "WgpuBindingsExtension".into()
    }

    fn init_root(
        &mut self,
        _shader_path: &str,
        res: &mut wesl::Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        res.set_mangler(wesl::ManglerKind::Escape);

        writeln!(self.bindings_mod_file, "#![allow(unused)]\n")?;

        Ok(())
    }

    fn into_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // println!("base: {}", self.bindings_mod_path.display());

        let dir_name = dir_path.file_stem().unwrap().to_str().expect("mod path must be valid UTF-8");
        writeln!(self.bindings_mod_file, "pub(crate) mod {dir_name};")?;

        // remove last mod
        self.bindings_mod_path.pop();
        // move into {dir_name}/"mod.rs
        self.bindings_mod_path.push(dir_name);
        self.bindings_mod_path.push("mod.rs");

        if let Some(dir_to_mod) = self.bindings_mod_path.parent() {
            fs::create_dir_all(dir_to_mod)?;
        }
        // println!("creating: {}", self.bindings_mod_path.display());
        let sub_bindings_mod_file = BufWriter::new(std::fs::File::create(&self.bindings_mod_path)?);
        self.bindings_mod_file = sub_bindings_mod_file;

        Ok(())
    }

    fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // println!("{}", self.bindings_mod_path.display());
        self.bindings_mod_path.pop();
        self.bindings_mod_path.pop();
        self.bindings_mod_path.push("mod.rs");
        // println!("{}", self.bindings_mod_path.display());

        self.bindings_mod_file = BufWriter::new(std::fs::File::open(&self.bindings_mod_path)?);

        Ok(())
    }

    fn post_build(&mut self, mod_path: &ModulePath, wgsl_source_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        generate_bindings(
            self.binding_root_path, &mut self.bindings_mod_file,
            mod_path, wgsl_source_path
        ).map_err(|e| Box::<_>::from(e))
    }
}


#[cfg(feature = "wgpu_bindings")]
fn generate_bindings(
    binding_root_path: &str,
    bindings_mod_file: &mut impl Write,
    mod_path: &ModulePath,
    wgsl_source_path: &str,
) -> Result<(), WgpuBindingsError> {
    use wgsl_to_wgpu::MatrixVectorTypes;

    let wgsl_source = std::fs::read_to_string(&wgsl_source_path).unwrap();

    // Configure the output based on the dependencies for the project
    let options = WriteOptions {
        derive_bytemuck_vertex: true,
        derive_encase_host_shareable: true,
        matrix_vector_types: MatrixVectorTypes::Nalgebra,
        ..Default::default()
    };

    // Generate the bindings
    let text = create_shader_module(
        &wgsl_source,
        &wgsl_source_path,
        options
    ).unwrap();

    let binding_path = format!(
        "{}/{}.rs",
        binding_root_path.to_owned(),
        mod_path.components.join("/")
    );
    let binding_path = PathBuf::from(binding_path);

    std::fs::create_dir_all(binding_path.parent().unwrap())?;
    std::fs::write(&binding_path, text.as_bytes())?;

    // Add entry to `mod.rs`
    writeln!(
        bindings_mod_file,
        "pub(crate) mod {};",
        binding_path.file_stem().unwrap().to_str().unwrap()
    )?;

    Ok(())
}

fn create_shader_module(
    wgsl_source: &str,
    wgsl_include_path: &str,
    options: WriteOptions,
) -> Result<String, wgsl_to_wgpu::CreateModuleError> {
    let mut root = wgsl_to_wgpu::Module::default();
    root.add_shader_module(
        wgsl_source,
        Some(wgsl_include_path),
        options,
        wgsl_to_wgpu::ModulePath::default(),
        demangle_wesl,
    )?;
    Ok(root.to_generated_bindings(options))
}

#[cfg(feature = "wgpu_bindings")]
fn demangle_wesl(name: &str) -> wgsl_to_wgpu::TypePath {
    // todo handle `super` paths
    // Assume all paths are absolute paths.
    if name.starts_with("package_") {
        // Use the root module if unmangle fails.
        let mangler = wesl::EscapeMangler;
        let (path, name) = mangler
            .unmangle(name)
            .unwrap_or((ModulePath::new_root(), name.to_string()));

        // Assume all wesl paths are absolute paths.
        wgsl_to_wgpu::TypePath {
            parent: wgsl_to_wgpu::ModulePath {
                components: path.components,
            },
            name,
        }
    } else {
        // Use the root module if the name is not mangled.
        wgsl_to_wgpu::TypePath {
            parent: wgsl_to_wgpu::ModulePath::default(),
            name: name.to_string(),
        }
    }
}
