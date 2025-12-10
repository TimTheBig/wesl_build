use wesl_build::{WeslBuildError, build_shader_dir, extensions};

fn main() -> Result<(), WeslBuildError> {
    build_shader_dir(
        "../test/src/shaders",
        wesl::CompileOptions::default(),
        extensions![],
    )
}
