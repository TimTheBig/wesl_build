use std::{
    borrow::Cow, ffi::OsStr, path::{Path, PathBuf}
};

use wesl::{ModulePath, Resolver, StandardResolver, Wesl};

#[cfg(feature = "wgpu_bindings")]
pub mod wgpu_bindings_ext;

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
    #[error("Extension {} error: {}", .extension_name, .error)]
    ExtensionErr {
        extension_name: String,
        error: Box<dyn std::error::Error>,
    },
}

/// An extension that runs before and after all shaders are built and after each file is built
pub trait WeslBuildExtension<WeslResolver: Resolver> {
    /// The name to report in errors as the source extension
    fn name<'n>(&self) -> Cow<'n, str>;

    /// The first time the extension is called this is in the root before any files/modules are entered
    ///
    /// ### Args
    /// * `shader_path` - the root dir of the shaders we are building
    /// * `res` - the wesl resolver being used by wesl_build
    fn init_root(
        &mut self, shader_root_path: &str, res: &mut Wesl<WeslResolver>
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// The last time the extension is called this is in the root after all files/modules are covered
    ///
    /// ### Args
    /// * `shader_path` - the root dir of the shaders we are building
    /// * `res` - the wesl resolver being used by wesl_build
    fn exit_root(
        &mut self, shader_root_path: &str, res: &Wesl<WeslResolver>
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Go one level into a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are entering
    fn into_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>>;
    /// Go one level out of a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are exiting
    fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>>;

    /// Run after a `wesl` file is compiled
    ///
    /// ### Args
    /// * `wesl_path` - the path to the wesl file
    /// * `wgsl_path` - the path to the built wgsl file
    fn post_build(
        &mut self,
        wesl_path: &ModulePath,
        wgsl_built_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

fn extension_error(ext: &Box<dyn WeslBuildExtension<StandardResolver>>, error: Box<dyn std::error::Error>) -> WeslBuildError {
    WeslBuildError::ExtensionErr {
        extension_name: ext.name().into_owned(),
        error,
    }
}

/// ## Args
/// * `shader_path` - Root dir of all your shaders
/// * `extensions` - An array of extensions you would like to run, see [`WeslBuildExtension`]
pub fn build_shader_dir(
    shader_path: &str,
    extensions: &mut [Box<dyn WeslBuildExtension<StandardResolver>>],
) -> Result<(), WeslBuildError> {
    let mut wesl = Wesl::new(shader_path);

    for ext in extensions.iter_mut() {
        ext.init_root(shader_path, &mut wesl)
            .map_err(|e| extension_error(ext, e))?;
    }

    // todo delete all in BINDING_ROOT_PATH before regen add some cashing(if wgsl_to_wgpu does not have it built-in)

    build_all_in_dir(
        shader_path,
        Path::new(shader_path),
        &wesl,
        extensions,
    )?;

    for ext in extensions.iter_mut() {
        ext.exit_root(shader_path, &mut wesl)
            .map_err(|e| extension_error(ext, e))?;
    }

    Ok(())
}

fn build_all_in_dir<WeslResolver: Resolver>(
    root_shader_path: &str,
    path: &Path,
    wesl: &Wesl<WeslResolver>,
    mut extensions: &mut [Box<dyn WeslBuildExtension<StandardResolver>>],
) -> Result<(), WeslBuildError> {
    for entry in std::fs::read_dir(path)?.filter_map(|entry| entry.ok()) {
        if entry.metadata()?.is_dir() {
            // make new mod per dir recurce to use mod structure
            let dir_path = entry.path();
            for ext in extensions.iter_mut() {
                // println!("running: {}", ext.name());
                ext.into_mod(&dir_path)
                    .map_err(|e| extension_error(ext, e))?;
            }
            // let dir_name = dir_path.file_stem().unwrap().to_str().unwrap();
            // writeln!(bindings_mod_file, "pub(crate) mod {};", dir_name)?;
            // let mut sub_bindings_mod_file = BufWriter::new(std::fs::File::create(format!(
            //     "src/shader_bindings/{}/mod.rs",
            //     dir_name
            // ))?);

            build_all_in_dir(root_shader_path, &dir_path, wesl, &mut extensions)?;

            if path != Path::new(root_shader_path) {
                for ext in extensions.iter_mut() {
                    ext.exit_mod(&dir_path)
                        .map_err(|e| extension_error(ext, e))?;
                }
            }
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
                ext.post_build(&mod_path, &wgsl_source_path)
                    .map_err(|e| extension_error(ext, e))?;
            }
        }
    }

    Ok(())
}
