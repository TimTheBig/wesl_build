use std::{fs::read_to_string, iter::once};

use insta::assert_snapshot;
use itertools::Itertools;

use super::*;
use crate::wgpu_bindings_ext::WgpuBindingsExtension;

#[test]
fn test_bindings_ext() {
    std::fs::create_dir_all("./test/src/shader_bindings").unwrap();

    #[cfg(feature = "logging")]
    init_build_logger();

    build_shader_dir(
        "./test/src/shaders",
        &mut [Box::new(WgpuBindingsExtension::new("./test/src/shader_bindings").unwrap())]
    ).unwrap();

    // shaders
    assert!(std::fs::exists("test/src/shaders/test.wgsl").unwrap(), "test_bindings_ext requires shaders/test.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test2.wgsl").unwrap(), "test_bindings_ext requires shaders/test2.wgsl to exist");

    // bindings
    assert!(std::fs::exists("test/src/shader_bindings/mod.rs").unwrap(), "no mod.rs was generated for shader_bindings root mod");
    assert!(std::fs::exists("test/src/shader_bindings/test_mod/mod.rs").unwrap(), "no mod.rs was generated for shader_bindings test_mod mod");

    assert!(std::fs::exists("test/src/shader_bindings/test.rs").unwrap(), "no shader_binding was generated for test.wgsl");
    assert!(std::fs::exists("test/src/shader_bindings/test2.rs").unwrap(), "no shader_binding was generated for test2.wgsl");
    assert!(std::fs::exists("test/src/shader_bindings/test_mod/test_mod_file.rs").unwrap(), "no shader_binding was generated for test_mod_file.wgsl");

    let mut settings = insta::Settings::new();
    // todo use
    // ("SOURCE", |val, path| {
    //     let source_path = Path::new(val.as_str().unwrap());
    //     assert_eq!(source_path.extension().unwrap(), "wgsl");
    //     assert!(source_path.starts_with(std::env::home_dir().unwrap()));

    //     "source path"
    // });

    let test_bindings = ["test/src/shader_bindings/test.rs", "test/src/shader_bindings/test2.rs", "test/src/shader_bindings/test_mod/test_mod_file.rs"];

    for binding in test_bindings {
        settings.set_input_file(binding);
        settings.bind(|| assert_snapshot!(
            read_to_string(binding).unwrap()
                // skip `SOURCE` const
                .lines().skip(3)
                .interleave_shortest(once("\n").cycle())
                .collect::<String>()
        ));
    }
}
