use std::path::Path;

use wesl_build::wgpu_bindings_ext::WgpuBindingsExtension;
use wesl_build::{WeslBuildError, WeslBuildExtension, build_shader_dir};

use wesl::Wesl;

struct WeslSizeLogger {
    messages: Vec<String>,
}

impl WeslSizeLogger {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
}

impl<WeslResolver: wesl::Resolver> WeslBuildExtension<WeslResolver> for WeslSizeLogger {
    fn name<'n>(&self) -> std::borrow::Cow<'n, str> {
        "WeslSizeLogger".into()
    }

    fn init_root(
        &mut self,
        _shader_root_path: &str,
        _res: &mut Wesl<WeslResolver>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name = wesl_path.last().expect("file must have an element in path");

        let source_lines = std::fs::read_to_string(wesl_path.to_path_buf())
            .unwrap()
            .lines()
            .count();
        let built_lines = std::fs::read_to_string(wgsl_built_path)
            .unwrap()
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
        &mut [
            Box::new(WgpuBindingsExtension::new("binding_root_path").unwrap()),
            Box::new(WeslSizeLogger::new()),
        ],
    )
}
