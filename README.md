# Wesl Build

A <u>**simple**</u> and <u>**extensible**</u> build system for [wesl-rs](https://github.com/wgsl-tooling-wg/wesl-rs), the compiler for WESL (WGSL Extended) shaders.

## Example
```sh
cargo add wesl_build --build
```
In `build.rs`:
```rs
use wesl_build::{build_shader_dir, WeslBuildError, WeslBuildExtension};

fn main() {
    build_shader_dir("src/shaders", &mut [/* Box::new(Extension::new()) */]).expect("Building shaders failed");
}
```

Now all shaders in `src/shaders` will be compiled with subdirectories accting as modules
