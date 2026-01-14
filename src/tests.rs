use std::{fs::read_to_string, iter::once};

use insta::assert_snapshot;
use itertools::Itertools;

use super::*;

#[cfg(feature = "wgpu_bindings_ext")]
use crate::extension::wgpu_bindings::WgpuBindingsExtension;

#[cfg(feature = "wgpu_bindings_ext")]
#[test]
fn test_bindings_ext() {
    std::fs::create_dir_all("./test/src/shader_bindings").unwrap();

    #[cfg(feature = "logging")]
    crate::init_build_logger();

    build_shader_dir(
        "./test/src/shaders",
        wesl::CompileOptions::default(),
        &mut [Box::new(
            WgpuBindingsExtension::new("./test/src/shader_bindings").unwrap(),
        )],
    )
    .unwrap();

    // shaders
    assert!(std::fs::exists("test/src/shaders/test.wgsl").unwrap(), "test_bindings_ext requires shaders/test.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test2.wgsl").unwrap(), "test_bindings_ext requires shaders/test2.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test_mod/test_mod_file.wgsl").unwrap(), "test_bindings_ext requires shaders/test_mod/test_mod_file.wgsl to exist");

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

    let test_bindings = [
        "test/src/shader_bindings/test.rs",
        "test/src/shader_bindings/test2.rs",
        "test/src/shader_bindings/test_mod/test_mod_file.rs",
    ];

    for binding in test_bindings {
        settings.set_input_file(binding);
        settings.bind(|| assert_snapshot!({
            let binding_file = read_to_string(binding).unwrap();
            // remove `SOURCE` const
            let source_line_num = binding_file.lines()
                .find_position(|line| line.contains("pub const SOURCE")).unwrap();
            binding_file.lines().enumerate()
                .filter(|(i, _)| !(source_line_num.0..source_line_num.0 + 3).contains(i))
                // de-enumerate
                .map(|(_, l)| l)
                .interleave_shortest(once("\n").cycle())
                .collect::<String>()
                // if HOME is still in there somehow remove it just to be safe
                .replace(env!("HOME"), "~")
        }));
    }
}

#[cfg(feature = "wgsl_minifier_ext")]
#[test]
fn test_minifier_ext() {
    #[cfg(feature = "logging")]
    crate::init_build_logger();

    build_shader_dir(
        "./test/src/shaders",
        wesl::CompileOptions::default(),
        &mut [Box::new(crate::extension::wgsl_minifier::WgslMinifierExtension { release_only: false })],
    )
    .unwrap();

    // shaders
    assert!(std::fs::exists("test/src/shaders/test.wgsl").unwrap(), "test_bindings_ext requires shaders/test.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test2.wgsl").unwrap(), "test_bindings_ext requires shaders/test2.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test_mod/test_mod_file.wgsl").unwrap(), "test_bindings_ext requires shaders/test_mod/test_mod_file.wgsl to exist");

    // todo test that output size is <= pre-minification, using extension to log before size and another for after size
}

#[test]
fn test_build_shader_dir() {
    #[cfg(feature = "logging")]
    crate::init_build_logger();

    build_shader_dir(
        "./test/src/shaders",
        wesl::CompileOptions::default(),
        &mut [],
    )
    .unwrap();

    // shaders
    assert!(std::fs::exists("test/src/shaders/test.wgsl").unwrap(), "test_bindings_ext requires shaders/test.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test2.wgsl").unwrap(), "test_bindings_ext requires shaders/test2.wgsl to exist");
    assert!(std::fs::exists("test/src/shaders/test_mod/test_mod_file.wgsl").unwrap(), "test_bindings_ext requires shaders/test_mod/test_mod_file.wgsl to exist");
}

mod build_tests {
    use std::{
        borrow::Cow,
        error::Error,
        fmt::Write as _,
        fs::{self, File},
        io::Write,
        path::Path,
        sync::{Arc, Mutex},
    };

    use tempfile::tempdir;

    use crate::*;
    use crate::extension::WeslBuildExtension;

    use wesl::{BasicSourceMap, ModulePath, StandardResolver, Wesl};

    /// MockExtension records lifecycle calls into a shared Arc<Mutex<Vec<String>>>,
    /// so tests can both hand the extension to the build system and still inspect
    /// the recorded calls afterwards.
    // used to trace the build cycle for testing
    #[derive(Clone)]
    struct MockExtension {
        /// Record sequence of calls (strings) so tests can assert call order/contents.
        calls: Arc<Mutex<Vec<String>>>,
        /// If set, calling certain lifecycle methods will return an error for testing.
        fail_on: Option<&'static str>,
    }

    impl MockExtension {
        /// normal instance + shared buffer
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    calls: calls.clone(),
                    fail_on: None,
                },
                calls,
            )
        }

        /// failing instance: init/exit/enter/post points may return Err depending on `fail_on`
        fn new_failing(fail_on: &'static str) -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    calls: calls.clone(),
                    fail_on: Some(fail_on),
                },
                calls,
            )
        }

        fn record(&self, s: impl Into<String>) {
            self.calls.lock().unwrap().push(s.into());
        }

        fn should_fail(&self, point: &str) -> bool {
            self.fail_on == Some(point)
        }
    }

    impl WeslBuildExtension<StandardResolver> for MockExtension {
        fn name<'n>(&self) -> Cow<'n, str> {
            "MockExtension".into()
        }

        fn init_root(
            &mut self,
            shader_root_path: &str,
            _res: &mut Wesl<StandardResolver>,
        ) -> Result<(), Box<dyn Error>> {
            self.record(format!("init_root:{}", shader_root_path));
            if self.should_fail("init_root") {
                return Err("init_root failed".into());
            }
            Ok(())
        }

        fn exit_root(
            &mut self,
            shader_root_path: &str,
            _res: &Wesl<StandardResolver>,
        ) -> Result<(), Box<dyn Error>> {
            self.record(format!("exit_root:{}", shader_root_path));
            if self.should_fail("exit_root") {
                return Err("exit_root failed".into());
            }
            Ok(())
        }

        fn enter_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn Error>> {
            self.record(format!("enter_mod:{}", dir_path.display()));
            if self.should_fail("enter_mod") {
                return Err("enter_mod failed".into());
            }
            Ok(())
        }

        fn exit_mod(&mut self, dir_path: &Path) -> Result<(), Box<dyn Error>> {
            self.record(format!("exit_mod:{}", dir_path.display()));
            if self.should_fail("exit_mod") {
                return Err("exit_mod failed".into());
            }
            Ok(())
        }

        fn post_build(
            &mut self,
            wesl_path: &ModulePath,
            wgsl_built_path: &str,
            source_map: &Option<BasicSourceMap>,
        ) -> Result<(), Box<dyn Error>> {
            let mut msg = String::new();
            write!(
                &mut msg,
                "post_build:{}:{}:{}",
                wesl_path,
                wgsl_built_path,
                if source_map.is_some() { "sourcemap" } else { "no_sourcemap" }
            )
            .unwrap();
            self.record(msg);

            if self.should_fail("post_build") {
                return Err("post_build failed".into());
            }
            Ok(())
        }
    }

    // =======< tests >=======

    #[test]
    fn extension_lifecycle_order_is_correct() {
        // create extension and shared recorder
        let (mut ext, calls) = MockExtension::new();

        // create a dummy Wesl instance (no compiling done here)
        let mut wesl = Wesl::<StandardResolver>::new("shaders");

        ext.init_root("shaders", &mut wesl).unwrap();
        ext.enter_mod(Path::new("shaders/foo")).unwrap();
        ext.exit_mod(Path::new("shaders/foo")).unwrap();
        ext.exit_root("shaders", &wesl).unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(
            &*calls,
            &[
                "init_root:shaders",
                "enter_mod:shaders/foo",
                "exit_mod:shaders/foo",
                "exit_root:shaders",
            ]
        );
    }

    #[test]
    fn mock_extension_records_lifecycle_calls_directly() {
        // another direct invocation test demonstrating the shared buffer API
        let (mut ext, calls) = MockExtension::new();

        ext.init_root("root/path", &mut Wesl::new("root/path")).unwrap();
        ext.enter_mod(Path::new("root/path/submod")).unwrap();
        ext.exit_mod(Path::new("root/path/submod")).unwrap();
        ext.exit_root("root/path", &Wesl::new("root/path")).unwrap();

        let calls_vec = calls.lock().unwrap().clone();
        assert_eq!(calls_vec.len(), 4, "expected four lifecycle calls recorded");
        assert_eq!(calls_vec[0], "init_root:root/path");
        assert!(calls_vec[1].starts_with("enter_mod:"));
        assert!(calls_vec[2].starts_with("exit_mod:"));
        assert_eq!(calls_vec[3], "exit_root:root/path");
    }

    #[test]
    fn build_shader_dir_propagates_extension_error_on_init_root_and_records_call() {
        // Create a temporary shader tree so build_shader_dir has at least one file.
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("shaders");
        fs::create_dir_all(&root).unwrap();

        let wgsl_text = r#"
            @vertex
            fn vs_main() -> void { return; }
        "#;
        let f1 = root.join("a.wgsl");
        let mut fh1 = File::create(&f1).unwrap();
        write!(fh1, "{}", wgsl_text).unwrap();

        // failing extension with shared recorder
        let (ext, calls) = MockExtension::new_failing("init_root");

        // move extension into boxed slice and call build_shader_dir
        let boxed_ext = Box::new(ext);
        let result = build_shader_dir(
            root.to_str().unwrap(),
            wesl::CompileOptions::default(),
            &mut [boxed_ext],
        );

        // Expect ExtensionErr variant and that init_root was recorded
        match result {
            Ok(()) => panic!("expected build_shader_dir to return Err when extension init fails"),
            Err(WeslBuildError::ExtensionErr { extension_name, error, .. }) => {
                assert!(extension_name == "MockExtension");
                assert!(format!("{}", error).contains("init_root failed"));
            }
            Err(other) => panic!("expected ExtensionErr variant, got {:?}", other),
        }

        let recorded = calls.lock().unwrap().clone();
        assert!(recorded.iter().any(|c| c.starts_with("init_root:")), "init_root should have been recorded");
    }
}
