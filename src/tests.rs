use super::*;
use crate::wgpu_bindings_ext::WgpuBindingsExtension;

#[test]
fn test_bindings_ext() {
    #[cfg(feature = "logging")]
    init_build_logger();

    build_shader_dir(
        "./test/src/shaders",
        &mut [WgpuBindingsExtension::new("./test/src/shader_bindings").unwrap()]
    ).unwrap();

    assert!(std::fs::exists("test/src/shaders/test.wgsl").unwrap(), "test_bindings_ext requires shaders/test.wgsl to exist");
    assert!(std::fs::exists("test/src/shader_bindings/test.rs").unwrap(), "no shader_binding was generated for test.wgsl");
    assert!(std::fs::exists("test/src/shader_bindings/mod.rs").unwrap(), "no mod.rs was generated for shader_bindings root mod");
}
