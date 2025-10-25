use std::{
    borrow::Cow,
    path::Path,
};

use wesl::{ModulePath, Resolver, StandardResolver, Wesl};

use crate::WeslBuildError;

#[cfg(feature = "wgpu_bindings")]
pub mod wgpu_bindings;

#[cfg(feature = "wgsl_minifier")]
pub mod wgsl_minifier;

/// An extension that runs before and after all shaders are built and after each file is built
///
/// Extensions are **always** run one at a time (sequentially)
/// so they can replace `wgsl_built_path` post-build with there output.
/// But the order is set by how the user orders them,
/// if your extension needs to run before/after extensions that changes something it must be documented
pub trait WeslBuildExtension<WeslResolver: Resolver> {
    /// The name to report in errors as the source extension
    fn name<'n>(&self) -> Cow<'n, str>;

    /// The first time the extension is called this is in the root before any files/modules are entered
    ///
    /// ### Args
    /// * `shader_path` - the root dir of the shaders we are building
    /// * `res` - the wesl resolver being used by wesl_build
    fn init_root(
        &mut self,
        shader_root_path: &str,
        res: &mut Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// The last time the extension is called this is in the root after all files/modules are covered
    ///
    /// ### Args
    /// * `shader_path` - the root dir of the shaders we are building
    /// * `res` - the wesl resolver being used by wesl_build
    fn exit_root(
        &mut self,
        _shader_root_path: &str,
        _res: &Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Go one level into a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are entering
    fn enter_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>>;
    /// Go one level out of a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are exiting
    fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>>;

    /// Run after a `wesl` file is compiled
    ///
    /// ### Args
    /// * `wesl_path` - the path to the wesl file
    /// * `wgsl_built_path` - the path to the built wgsl file
    fn post_build(
        &mut self,
        wesl_path: &ModulePath,
        wgsl_built_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

/// Util for wrapping an extensions error in a [`WeslBuildError`]
pub(crate) fn extension_error(
    ext: &Box<dyn WeslBuildExtension<StandardResolver>>,
    error: Box<dyn std::error::Error>,
) -> WeslBuildError {
    WeslBuildError::ExtensionErr {
        extension_name: ext.name().into_owned(),
        error,
    }
}
