use std::path::Path;

#[cfg(feature = "wgpu_bindings_ext")]
use wesl_build::extension::wgpu_bindings::WgpuBindingsExtension;
use wesl_build::{WeslBuildError, extension::WeslBuildExtension, build_shader_dir};

use wesl::{BasicSourceMap, Wesl};

struct WeslSizeLogger {
    messages: Vec<String>,
    shader_root_path: String,
}

impl WeslSizeLogger {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            shader_root_path: String::new(),
        }
    }
}

impl<WeslResolver: wesl::Resolver> WeslBuildExtension<WeslResolver> for WeslSizeLogger {
    fn name<'n>(&self) -> std::borrow::Cow<'n, str> {
        "WeslSizeLogger".into()
    }

    fn init_root(
        &mut self,
        shader_root_path: &str,
        _res: &mut Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.shader_root_path = shader_root_path.to_owned();

        Ok(())
    }

    fn exit_root(
        &mut self,
        _shader_root_path: &str,
        _res: &Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("name | source_lines | built_lines");
        println!("----------------------------------------------------");
        for massage in &self.messages {
            println!("{massage}");
        }

        Ok(())
    }

    fn enter_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn exit_mod(&mut self, _dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn post_build(
        &mut self,
        wesl_path: &wesl::ModulePath,
        wgsl_built_path: &str,
        _source_map: &Option<BasicSourceMap>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name = wesl_path.last().expect("file must have an element in path");

        // ModulePath::to_path_buf adds a prefix '/' to absolute paths
        let source_lines = std::fs::read_to_string(format!("{}{}", self.shader_root_path, wesl_path.to_path_buf().display()))?
            .lines()
            .count();
        let built_lines = std::fs::read_to_string(wgsl_built_path)?
            .lines()
            .count();

        self.messages
            .push(format!("{name} | {source_lines} | {built_lines}"));

        Ok(())
    }
}

fn main() -> Result<(), WeslBuildError> {
    build_shader_dir(
        "./test/src/shaders",
        wesl::CompileOptions::default(),
        &mut [
            #[cfg(feature = "wgpu_bindings_ext")]
            Box::new(WgpuBindingsExtension::new("binding_root_path").unwrap()),
            Box::new(WeslSizeLogger::new()),
        ],
    )
}
