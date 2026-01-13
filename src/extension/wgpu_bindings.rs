#![cfg(feature = "wgpu_bindings_ext")]

use std::fmt::Display;
use std::io::{BufWriter, Write};
use std::{fs, path::{Path, PathBuf}};

use wesl::{BasicSourceMap, Mangler};
use wesl::ModulePath;
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
    // todo take `wgsl_to_wgpu` options as args, storing `WriteOptions` in struct
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
    IoErr(#[from] std::io::Error),
    /// spans and paths are that of the compiled files
    CreateBindingsModuleErr {
        inner: wgsl_to_wgpu::CreateModuleError,
        wgsl_source: String,
        path: PathBuf,
    },
}

impl Display for WgpuBindingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WgpuBindingsError::IoErr(io_err) => io_err.fmt(f),
            // use `emit_to_string_with_path` for output with span and labels
            // todo use source map to modify span and path
            WgpuBindingsError::CreateBindingsModuleErr { inner, wgsl_source, path } =>
                inner.emit_to_string_with_path(wgsl_source, path).fmt(f),
        }
    }
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

    fn enter_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let dir_name = dir_path.file_stem().expect("module must have a name in path")
            .to_str().expect("mod path must be valid UTF-8");
        writeln!(self.bindings_mod_file, "pub(crate) mod {dir_name};")?;

        // remove last mod
        self.bindings_mod_path.pop();
        // move into {dir_name}/"mod.rs
        self.bindings_mod_path.push(dir_name);
        self.bindings_mod_path.push("mod.rs");

        if let Some(dir_to_mod) = self.bindings_mod_path.parent() {
            fs::create_dir_all(dir_to_mod)?;
        }
        #[cfg(feature = "logging")]
        log::trace!("creating wgpu binding module for: {}", self.bindings_mod_path.display());

        let sub_bindings_mod_file = BufWriter::new(fs::File::create(&self.bindings_mod_path)?);
        self.bindings_mod_file = sub_bindings_mod_file;

        Ok(())
    }

    fn exit_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.bindings_mod_path.pop();

        #[cfg(feature = "logging")]
        if let Some(mod_name) = self.bindings_mod_path.file_stem() {
            log::debug!("exiting mod: {}", mod_name.display());
        }

        self.bindings_mod_path.pop();
        self.bindings_mod_path.push("mod.rs");

        self.bindings_mod_file = BufWriter::new(fs::File::open(&self.bindings_mod_path)?);

        Ok(())
    }

    fn post_build(
        &mut self,
        mod_path: &ModulePath,
        wgsl_source_path: &str,
        _source_map: &Option<BasicSourceMap>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        generate_bindings(
            self.binding_root_path,
            &mut self.bindings_mod_file,
            mod_path,
            wgsl_source_path,
        )
        // todo don't double box
        .map_err(Box::<_>::from)
    }
}

fn generate_bindings(
    binding_root_path: &str,
    bindings_mod_file: &mut impl Write,
    mod_path: &ModulePath,
    wgsl_source_path: &str,
) -> Result<(), Box<WgpuBindingsError>> {
    use wgsl_to_wgpu::MatrixVectorTypes;

    let wgsl_source = fs::read_to_string(wgsl_source_path)
        .map_err(|e| Box::new(WgpuBindingsError::IoErr(e)))?;

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
        wgsl_source_path,
        options,
    )?;

    let binding_path = format!(
        "{}/{}.rs",
        binding_root_path.to_owned(),
        mod_path.components.join("/")
    );
    let binding_path = PathBuf::from(binding_path);

    fs::create_dir_all(
        binding_path.parent().expect("binding must have a parent mod or be in root")
    ).map_err(|e| Box::from(WgpuBindingsError::IoErr(e)))?;
    fs::write(&binding_path, text.as_bytes())
        .map_err(|e| Box::from(WgpuBindingsError::IoErr(e)))?;

    // Add entry to `mod.rs`
    writeln!(
        bindings_mod_file,
        "pub(crate) mod {};",
        binding_path.file_stem().expect("binding must have a name in path")
            .to_str().expect("mod path must be valid UTF-8")
    ).map_err(|e| Box::from(WgpuBindingsError::IoErr(e)))?;

    Ok(())
}

fn create_shader_module(
    wgsl_source: &str,
    // path to the compiled file
    wgsl_include_path: &str,
    options: WriteOptions,
) -> Result<String, Box<WgpuBindingsError>> {
    let mut root = wgsl_to_wgpu::Module::default();
    root.add_shader_module(
        wgsl_source,
        Some(wgsl_include_path),
        options,
        wgsl_to_wgpu::ModulePath::default(),
        demangle_wesl,
        // |str| source_map.unwrap().get_decl(str).unwrap().0
    ).map_err(|e| {
        let home_dir = std::env::home_dir();
        let wgsl_path = if let Some(home_dir) = home_dir
            && cfg!(target_family = "unix")
            && let Some(home_dir) = home_dir.to_str()
        {
            wgsl_include_path.replace(home_dir, "~")
        } else {
            wgsl_include_path.to_owned()
        };

        Box::from(WgpuBindingsError::CreateBindingsModuleErr {
            inner: e,
            wgsl_source: wgsl_source.to_owned(),
            path: PathBuf::from(wgsl_path)
        })
    })?;
    Ok(root.to_generated_bindings(options))
}

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
