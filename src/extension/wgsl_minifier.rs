#![cfg(feature = "wgsl_minifier")]

use std::{fs, path::Path};

use wesl::ModulePath;

use crate::WeslBuildExtension;

/// Removes all the characters it can from our built shaders.
pub struct WgslMinifierExtension {
    /// Whether shaders should only be minified in release builds
    pub release_only: bool,
}

impl<WeslResolver: wesl::Resolver> WeslBuildExtension<WeslResolver> for WgslMinifierExtension {
    fn name<'n>(&self) -> std::borrow::Cow<'n, str> {
        "WgslMinifierExtension".into()
    }

    fn init_root(
        &mut self,
        _shader_path: &str,
        _res: &mut wesl::Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    fn enter_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
    fn exit_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> { Ok(()) }

    fn post_build(
        &mut self,
        _mod_path: &ModulePath,
        wgsl_source_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.release_only {
            let profile = std::env::var("PROFILE")?;
            match profile.as_str() {
                "debug" => return Ok(()),
                "release" => (),
                _ => return Ok(()),
            };
        }
        let wgsl_source = fs::read_to_string(wgsl_source_path)?;

        let mut module = naga::front::wgsl::parse_str(&wgsl_source)?;

        // strip and minify
        wgsl_minifier::minify_module(&mut module);

        // Write to WGSL string
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        let info = validator.validate(&module)?;
        let output = naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())?;

        // remove whitespace and minify string
        let output = wgsl_minifier::minify_wgsl_source(&output);

        // replace built file with minified file
        fs::write(wgsl_source_path, output)?;

        Ok(())
    }
}
