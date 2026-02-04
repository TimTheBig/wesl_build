# Wesl Build

A <u>**simple**</u> and <u>**extensible**</u> build system for [wesl-rs](https://github.com/wgsl-tooling-wg/wesl-rs), the compiler for WESL (WGSL Extended) shaders.

## Example
```sh
cargo add wesl_build --build
```
In `build.rs`:
```rs
use wesl_build::{build_shader_dir, WeslBuildError};
use wesl_build::{extensions, extension::WeslBuildExtension};

fn main() {
    build_shader_dir("src/shaders", extensions![/* Extension::new() */]).expect("Building shaders failed");
}
```

Now all shaders in `src/shaders` will be compiled with subdirectories accting as modules, which can be nested

## Features
* logging - enables logging of the build along with fuerther logging in the built-in extensions

## Faster Shader Build Times

For faster builds add this to your Cargo.toml, it will speed up builds after the first one:
```toml
[profile.dev.package."wesl"]
opt-level = 3
[profile.dev.package."naga"]
opt-level = 3
# optional
[profile.dev.package."wesl_build"]
opt-level = 3
```
