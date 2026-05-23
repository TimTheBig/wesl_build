use std::{
    borrow::Cow, error::Error, marker::PhantomData, path::Path
};

use wesl::{BasicSourceMap, ModulePath, Resolver, Wesl};

use crate::WeslBuildError;

#[cfg(feature = "wgpu_bindings_ext")]
pub mod wgpu_bindings;

#[cfg(feature = "wgsl_minifier_ext")]
pub mod wgsl_minifier;

#[cfg(feature = "math_consts_ext")]
pub mod math_consts;

/// A utility that improves the readability of the extensions input into [`build_shader_dir`](`crate::build_shader_dir`)
///
/// ## Example
/// ```no_run
/// use wesl_build::{build_shader_dir, WeslBuildError};
/// use wesl_build::{extensions, extension::{WeslBuildExtension, wgpu_bindings::WgpuBindingsExtension}};
///
/// build_shader_dir(
///     "test/src/shaders",
///     wesl::CompileOptions::default(),
///     extensions![
///         WgpuBindingsExtension::new("test/src/shader_bindings").unwrap(),
//          # Test multiple inputs
///         # WgpuBindingsExtension::new("test/src/shader_bindings").unwrap()
///     ],
/// ).expect("Building shaders failed");
/// ```
#[macro_export]
macro_rules! extensions {
    () => (
        &mut []
    );
    // box inputs
    ($($ext:expr),+ $(,)?) => (
        &mut [$(::std::boxed::Box::new($ext)),+]
    );
}

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
    /// * `res` - the wesl resolver being used by `wesl_build`
    fn init_root(
        &mut self,
        shader_root_path: &str,
        res: &mut Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn Error>>;

    /// The last time the extension is called this is in the root after all files/modules are covered
    ///
    /// ### Args
    /// * `shader_path` - the root dir of the shaders we are building
    /// * `res` - the wesl resolver being used by `wesl_build`
    fn exit_root(
        &mut self,
        _shader_root_path: &str,
        _res: &Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Go one level into a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are entering
    fn enter_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn Error>>;
    /// Go one level out of a shader module
    ///
    /// ### Args
    /// * `dir_path` - the current dir of the mod we are exiting
    fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn Error>>;

    /// Run after a `wesl` file is compiled
    ///
    /// ### Args
    /// * `wesl_path` - the path to the wesl file
    /// * `wgsl_built_path` - the path to the built wgsl file
    fn post_build(
        &mut self,
        wesl_path: &ModulePath,
        wgsl_built_path: &str,
        source_map: Option<&BasicSourceMap>,
    ) -> Result<(), Box<dyn Error>>;
}

// todo make user configiruble struct extension to access and mut Resolver
pub struct ResolverConfigurator<
    WeslResolver: Resolver,
    SetupFunc: FnMut(&mut Wesl<WeslResolver>) -> Result<(), Box<dyn Error>>,
> {
    /// The setup function to run with access to the Wesl compiler
    setup_fn: SetupFunc,
    #[doc(hidden)]
    _res: PhantomData<WeslResolver>
}

impl<WeslResolver: Resolver, SetupFunc> ResolverConfigurator<WeslResolver, SetupFunc>
where SetupFunc: FnMut(&mut Wesl<WeslResolver>) -> Result<(), Box<dyn Error>> {
    pub const fn new(setup_fn: SetupFunc) -> Self {
        Self { setup_fn, _res: PhantomData }
    }
}

impl<WeslResolver: Resolver, SetupFunc> WeslBuildExtension<WeslResolver> for ResolverConfigurator<WeslResolver, SetupFunc>
where SetupFunc: FnMut(&mut Wesl<WeslResolver>) -> Result<(), Box<dyn Error>> {
    fn name<'n>(&self) -> Cow<'n, str> {
        "ResolverConfigurator".into()
    }

    // fn stage(&self) -> ExtensionStage {
    //     ExtensionStage::SetupOverride
    // }

    fn init_root(
        &mut self,
        _shader_root_path: &str,
        res: &mut Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn Error>> {
        (self.setup_fn)(res)
    }

    fn enter_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn exit_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn post_build(
        &mut self,
        _wesl_path: &ModulePath,
        _wgsl_built_path: &str,
        _source_map: Option<&BasicSourceMap>,
    ) -> Result<(), Box<dyn Error>> { Ok(()) }
}

// todo should each be a trait, in a composible way where all that are implemented are run
// or a different input struct for the args that implements a shared trait
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
enum ExtensionStage {
    /// Setup state and configuration
    Setup = 0,
    /// Configure resolver and user selected options
    SetupOverride,
    /// Read the WESL source of any module
    SourceRead,
    /// Read the compiled WGSL code of each module before any, possibly symbol removing, alterations are made
    // inspect, readonly wesl and built wgsl, (size logger, bindings)
    BuiltSymbols,
    /// May modify the code of each compiled module
    // post user, output is not for user cunsumtion, modifiys names in destructive way (ie. minify)
    BuiltWrite,
    /// Read the code of each compiled module after all alterations are complete
    // post user inspect, readonly wesl and built wgsl
    BuiltRead,
    /// Run after all files are built
    Cleanup,
}

impl Ord for ExtensionStage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // higher less then lower, so stages can be ordered by running order
        (*self as u8).cmp(&(*other as u8)).reverse()
    }
}

impl PartialOrd for ExtensionStage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Util for wrapping an extensions error in a [`WeslBuildError`]
pub(crate) fn extension_error(
    ext: &dyn WeslBuildExtension<impl Resolver>,
    error: Box<dyn Error>,
) -> WeslBuildError {
    WeslBuildError::ExtensionErr {
        extension_name: ext.name().into_owned(),
        error,
    }
}
