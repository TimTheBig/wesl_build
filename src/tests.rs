use super::*;
use crate::wgpu_bindings_ext::WgpuBindingsExtension;

#[test]
fn test_bindings_ext() {
    #[cfg(feature = "logging")]
    init_build_logger();

    build_shader_dir(
        "./test/src/shader",
        &mut [WgpuBindingsExtension::new("./test/src/shader_bindings").unwrap()]
    ).unwrap()
}
