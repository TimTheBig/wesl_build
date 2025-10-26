#![doc = include_str!("../README.md")]

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use wesl::{ModulePath, Resolver, StandardResolver, Wesl};

pub mod extension;
use extension::{WeslBuildExtension, extension_error};

#[cfg(test)]
mod tests;

/// An error from some stage of the build system, possibly from an extension
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

/// Init logging for better error messages
///
/// Note: To ensure color output set `RUST_LOG_STYLE=always`
#[cfg(feature = "logging")]
pub fn init_build_logger() {
    use log::LevelFilter;

    let could_init = env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .filter_module("naga::front", LevelFilter::Info)
        .format_timestamp(None)
        .try_init();

    if let Err(could_not_init) = could_init {
        log::warn!("wesl_build::init_build_logger was called after logger was already initialized: {}", could_not_init);
    }
}

/// A simple and extensible build system for wesl
///
/// ## Args
/// * `shader_path` - Root dir of all your shaders
/// * `extensions` - An array of extensions you would like to run, see [`WeslBuildExtension`](`extension::WeslBuildExtension`)
///
/// ## Example
/// In `build.rs`:
/// ```
/// use wesl_build::{build_shader_dir, WeslBuildError};
/// use wesl_build::{extensions, extension::WeslBuildExtension};
///
/// build_shader_dir(
///     # "test/src/shaders",
///     # /*
///     "src/shaders",
///     # */
///     wesl::CompileOptions::default(),
///     extensions![/* Extension::new() */]
/// ).expect("Building shaders failed");
/// ```
pub fn build_shader_dir(
    shader_path: &str,
    wesl_config: wesl::CompileOptions,
    extensions: &mut [Box<dyn WeslBuildExtension<StandardResolver>>],
) -> Result<(), WeslBuildError> {
    let mut wesl = Wesl::new(shader_path);
    wesl.set_options(wesl_config);
    // todo allow `use_sourcemap` override

    for ext in extensions.iter_mut() {
        #[cfg(feature = "logging")]
        log::debug!("initializing: {}", ext.name());

        ext.init_root(shader_path, &mut wesl)
            .map_err(|e| extension_error(ext, e))?;
    }

    // todo delete all in BINDING_ROOT_PATH before regen add some cashing(if wgsl_to_wgpu does not have it built-in),
    // so bindings for deleted shaders are removed

    build_all_in_dir(
        shader_path, Path::new(shader_path),
        &wesl, extensions,
    )?;

    for ext in extensions.iter_mut() {
        ext.exit_root(shader_path, &wesl)
            .map_err(|e| extension_error(ext, e))?;
    }

    Ok(())
}

fn build_all_in_dir<WeslResolver: Resolver>(
    root_shader_path: &str,
    path: &Path,
    wesl: &Wesl<WeslResolver>,
    extensions: &mut [Box<dyn WeslBuildExtension<StandardResolver>>],
) -> Result<(), WeslBuildError> {
    for entry in std::fs::read_dir(path)?.filter_map(|entry| entry.ok()) {
        if entry.metadata()?.is_dir() {
            // make new mod per dir recurce to use mod structure
            let dir_path = entry.path();
            for ext in extensions.iter_mut() {
                ext.enter_mod(&dir_path)
                    .map_err(|e| extension_error(ext, e))?;
            }

            build_all_in_dir(root_shader_path, &dir_path, wesl, extensions)?;

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

            // module from root(absolute) path to entry
            let mut out_name = entry_path.strip_prefix(root_shader_path)?.to_owned();
            out_name.pop();
            out_name = PathBuf::from(
                out_name
                    .join(PathBuf::from(entry.file_name()).file_stem()
                        .expect("shader file must have a name in path")
                    )
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
            #[cfg(feature = "logging")]
            log::info!("built: {}", &mod_path);

            let wgsl_source_path = format!(
                "{}/{}.wgsl",
                std::env::var("OUT_DIR").expect(
                    "OUT_DIR env var must be set by cargo"/* any project with a build.rs will have this set */
                ),
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
