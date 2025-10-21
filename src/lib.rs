use std::{
    ffi::OsStr,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use wesl::{Mangler, ModulePath, Resolver, StandardResolver, Wesl};
use wgsl_to_wgpu::{MatrixVectorTypes, WriteOptions};

mod wgpu_bindings_ext;

#[cfg(test)]
mod tests;

/// Init logging for better error msgs
#[cfg(feature = "logging")]
pub fn init_build_logger() {
    use log::LevelFilter;

    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("naga::front", LevelFilter::Info)
        .format_timestamp(None)
        .init();
}

#[derive(Debug, thiserror::Error)]
pub enum WeslBuildError {
    #[error(transparent)]
    IoErr(#[from] std::io::Error),
    #[error(transparent)]
    StripPrefixErr(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    ExtensionErr(#[from] Box<dyn std::error::Error>),
}

pub trait WeslBuildExtension<WeslResolver: Resolver> {
    type ExtensionError: std::error::Error;

    fn init_root(
        &mut self, shader_path: &str, res: &Wesl<WeslResolver>
    ) -> Result<(), Self::ExtensionError>;

    /// Go one level into a shader module
    fn into_mod(&mut self, dir_path: &Path) -> Result<(), Self::ExtensionError>;
    /// Go one level out of a shader module
    fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Self::ExtensionError>;

    /// Run after a `wesl` file is compiled
    fn post_build(
        &mut self,
        mod_path: &ModulePath,
        wgsl_source_path: &str,
    ) -> Result<(), Self::ExtensionError>;
}

/// ## Args
/// * shader_path - Root dir of all your shaders
/// * binding_root_path - The path to output the rust bindings for shaders
pub fn build_shader_dir(
    shader_path: &str,
    // binding_root_path: &str,
    extensions: &mut [impl WeslBuildExtension<StandardResolver>],
) -> Result<(), WeslBuildError> {
    let wesl = Wesl::new(shader_path);
    // #[cfg(feature = "wgpu_bindings")]
    // let mut bindings_mod_file =
    //     BufWriter::new(std::fs::File::create("src/shader_bindings/mod.rs")?);
    // #[cfg(feature = "wgpu_bindings")]
    // writeln!(bindings_mod_file, "#![allow(unused)]\n")?;
    for ext in extensions.iter_mut() {
        ext.init_root(shader_path, &wesl);
            // .map_err(|e| Box::<dyn std::error::Error>::from(e))?;
    }

    // todo delete all in BINDING_ROOT_PATH before regen add some cashing(if wgsl_to_wgpu does not have it built-in)

    build_all_in_dir(
        shader_path,
        // binding_root_path,
        Path::new(shader_path),
        &wesl,
        extensions,
        // #[cfg(feature = "wgpu_bindings")]
        // &mut bindings_mod_file
    )
}

fn build_all_in_dir<WeslResolver: Resolver>(
    root_shader_path: &str,
    path: &Path,
    wesl: &Wesl<WeslResolver>,
    mut extensions: &mut [impl WeslBuildExtension<StandardResolver>],
    // #[cfg(feature = "wgpu_bindings")]
    // bindings_mod_file: &mut impl Write,
) -> Result<(), WeslBuildError> {
    for entry in std::fs::read_dir(path)?.filter_map(|entry| entry.ok()) {
        if entry.metadata()?.is_dir() {
            // make new mod per dir recurce to use mod structure
            let dir_path = entry.path();
            for ext in extensions.iter_mut() {
                ext.into_mod(&dir_path).unwrap();
            }
            // let dir_name = dir_path.file_stem().unwrap().to_str().unwrap();
            // writeln!(bindings_mod_file, "pub(crate) mod {};", dir_name)?;
            // let mut sub_bindings_mod_file = BufWriter::new(std::fs::File::create(format!(
            //     "src/shader_bindings/{}/mod.rs",
            //     dir_name
            // ))?);

            build_all_in_dir(root_shader_path, &dir_path, wesl, &mut extensions)?;
        } else {
            let entry_path = entry.path();

            if !(entry_path.extension() == Some(OsStr::new("wgsl"))
                || entry_path.extension() == Some(OsStr::new("wesl")))
            {
                continue;
            }
            println!("cargo::rerun-if-changed={}", entry_path.display());

            // module path from shader folder to entry
            let mut out_name = entry_path.strip_prefix(root_shader_path)?.to_owned();
            out_name.pop();
            out_name = PathBuf::from(
                out_name
                    .join(PathBuf::from(entry.file_name()).file_stem().unwrap())
                    .to_str()
                    .unwrap()
                    .replace('/', "::"),
            );

            let out_name_str = out_name.to_str().unwrap();
            let mod_path = ModulePath::new(
                wesl::syntax::PathOrigin::Absolute,
                out_name_str
                    .split("::")
                    .map(|str| str.to_owned())
                    .collect::<Vec<_>>(),
            );
            wesl.build_artifact(&mod_path, out_name_str);

            let wgsl_source_path = format!(
                "{}/{}.wgsl",
                std::env::var("OUT_DIR").unwrap(),
                out_name_str
            );

            for ext in &mut *extensions {
                ext.post_build(&mod_path, &wgsl_source_path).unwrap();
            }
            // #[cfg(feature = "wgpu_bindings")]
            // generate_bindings(binding_root_path, bindings_mod_file, mod_path, wgsl_source_path)?;
        }
    }

    Ok(())
}
