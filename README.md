# Wesl Build

A <u>**simple**</u> and <u>**extensible**</u> build system for [wesl-rs](https://github.com/wgsl-tooling-wg/wesl-rs), the compiler for WESL (WGSL Extended) shaders. For cargo level convenience in shader dev.

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
* `logging` - enables logging of builds along with logging in the built-in extensions
* `wgpu_bindings_ext` (default) - enables an extension that uses `wgsl_to_wgpu` to generate binding modules for your shaders. Note this requires you use the `wgpu` version expected by `wgsl_to_wgpu`
* `math_consts_ext` (default) - enables an extension that adds math and type specific consts at compile-time to `constants`\
**Consts**: `{u32, i32, f32}_{MAX, MIN}`, `{f32, f64}_{MIN_POSITIVE, EPSILON}`, `{DEG_TO_RAD, RAD_TO_DEG}`, and all stable rust math f64 constants, note: `FRAC_1_*` becomes `INV_*`
* `wgsl_minifier_ext` - enables an extension that minifies shaders, optionally only in release builds

Note: features post-fixed with `_ext` enable one or more implementors of `WeslBuildExtension`

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
