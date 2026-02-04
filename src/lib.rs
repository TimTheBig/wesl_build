#![doc = include_str!("../README.md")]

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    fs,
};

use itertools::Itertools;
use wesl::{BasicSourceMap, Mangler, ModulePath, Resolver, StandardResolver, Wesl};

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
        .parse_env("WESL_BUILD_LOG_LEVEL")
        .filter_module("naga::front", LevelFilter::Info)
        // don't spam every type selection
        .filter_module("naga::proc::typifier", LevelFilter::Info)
        .filter_module("naga::compact", LevelFilter::Debug)
        .filter_module("naga::proc", LevelFilter::Debug)
        .format_timestamp(None)
        .try_init();

    if let Err(could_not_init) = could_init {
        log::warn!("wesl_build::init_build_logger was called after logger was already initialized: {could_not_init}");
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
/// # use std::process::ExitCode;
/// use wesl_build::{build_shader_dir, WeslBuildError};
/// use wesl_build::{extensions, extension::WeslBuildExtension};
///
/// match build_shader_dir(
///     # "test/src/shaders",
///     # /*
///     "src/shaders",
///     # */
///     wesl::CompileOptions::default(),
///     extensions![/* Extension::new() */]
/// ) {
///    Ok(_) => ExitCode::SUCCESS,
///    Err(err) => {
#[cfg_attr(feature = "logging", doc = r#"        log::error!("{err}");"#)]
#[cfg_attr(not(feature = "logging"), doc = r#"        println!("{err}");"#)]
///
///        ExitCode::FAILURE
///   },
/// };
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
        log::debug!("initializing extension: {}", ext.name());

        ext.init_root(shader_path, &mut wesl)
            .map_err(|e| extension_error(ext.as_ref(), e))?;
    }

    // todo delete all in BINDING_ROOT_PATH before regen add some cashing(if wgsl_to_wgpu does not have it built-in),
    // so bindings for deleted shaders are removed

    build_all_in_dir::<StandardResolver>(
        shader_path, Path::new(shader_path),
        &wesl, extensions,
        &wesl,
    )?;

    for ext in extensions.iter_mut() {
        ext.exit_root(shader_path, &wesl)
            .map_err(|e| extension_error(ext.as_ref(), e))?;
    }

    // output shader_path to OUT_DIR/wesl_build_tree.path
    // fs::write(
    //     PathBuf::from(std::env::var_os("OUT_DIR").expect("wesl_build must be run in build.rs or in an env with the OUT_DIR environment variable set")).join("wesl_build_tree.path"),
    //     shader_path,
    // )?;
    // This env var should only used by wesl_build_import's derive macro after build scripts are run
    unsafe { std::env::set_var("WESL_BUILD_DIR_ROOT_PATH", shader_path) };

    Ok(())
}

fn build_all_in_dir<WeslResolver: Resolver>(
    root_shader_path: &str,
    path: &Path,
    wesl: &Wesl<WeslResolver>,
    extensions: &mut [Box<dyn WeslBuildExtension<StandardResolver>>],
    res: &Wesl<WeslResolver>,
) -> Result<(), WeslBuildError> {
    fs::read_dir(path)?.filter_map(|entry| entry.ok().map(|en| (en.metadata(), en)))
    // run dirs after files to insure correct recursion
    .sorted_by(|(a_meta, _), (b_meta, _)| {
        (a_meta.as_ref().ok().is_some_and(fs::Metadata::is_file)).cmp(
            &b_meta.as_ref().ok().is_some_and(fs::Metadata::is_file)
        ).reverse()
    }).try_for_each(|(metadata, entry)| -> Result<(), WeslBuildError> {
        if metadata?.is_dir() {
            // make new mod per dir recurce to use mod structure
            let dir_path = entry.path();
            for ext in extensions.iter_mut() {
                ext.enter_mod(&dir_path)
                    .map_err(|e| extension_error(ext.as_ref(), e))?;
            }

            build_all_in_dir(root_shader_path, &dir_path, wesl, extensions, res)?;

            if path != Path::new(root_shader_path) {
                for ext in extensions.iter_mut() {
                    ext.exit_mod(&dir_path)
                        .map_err(|e| extension_error(ext.as_ref(), e))?;
                }
            }
        } else {
            let entry_path = entry.path();

            if !(entry_path.extension() == Some(OsStr::new("wgsl"))
                || entry_path.extension() == Some(OsStr::new("wesl")))
            {
                return Ok(());
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
                    // todo mangle in place of :: use wesl mangler
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
            // wesl::emit_rerun_if_changed(&[mod_path], res);

            // !! keep in sync with mangler used in wesl_build_import !!
            let name_mangler = wesl::EscapeMangler;
            let mangled_name = &name_mangler.mangle(
                &mod_path,
                out_name.file_stem().and_then(|os_str| os_str.to_str()).unwrap()
            );

            let source_map = build_artifact(
                wesl, &mod_path, mangled_name
            );
            #[cfg(feature = "logging")]
            log::info!("built: {}", &mod_path);

            let wgsl_source_path = format!(
                "{}/{}.wgsl",
                std::env::var("OUT_DIR").expect(
                    "OUT_DIR env var must be set by cargo, any project with a build.rs will have this set"
                ),
                mangled_name,
            );

            for ext in &mut *extensions {
                ext.post_build(&mod_path, &wgsl_source_path, &source_map)
                    .map_err(|e| extension_error(ext.as_ref(), e))?;
            }
            Ok(())
        }
    })
}

/// Compile a WESL program from a root file and output the result in Rust's `OUT_DIR`.
///
/// This function is meant to be used in a `build.rs` workflow. The output WGSL will
/// be accessed with the [`include_wesl`] macro. See the crate documentation for a
/// usage example.
///
/// * The first argument is the path to the root module relative to the base
///   directory.
/// * The second argument is the name of the artifact, used in [`include_wesl`].
///
/// Will emit `rerun-if-changed` instructions so the build script reruns only if the
/// shader files are modified.
///
/// # Panics
/// Panics when compilation fails or if the output file cannot be written.
/// Pretty-prints the WESL error message to stderr.
fn build_artifact(res: &Wesl<impl Resolver>, root: &ModulePath, artifact_name: &str) -> Option<BasicSourceMap> {
    let compiled = res
        .compile(root)
        .inspect_err(|e| {
            eprintln!("failed to build WESL shader `{root}`.\n{e}");
            panic!();
        })
        .unwrap();
    wesl::emit_rerun_if_changed(&compiled.modules, &res.resolver());
    compiled.write_artifact(artifact_name);

    compiled.sourcemap
}
