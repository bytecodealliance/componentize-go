#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use core::panic;
    use once_cell::sync::Lazy;
    use std::{
        net::TcpListener,
        path::PathBuf,
        process::{Child, Command, Stdio},
        time::Duration,
    };

    static COMPONENTIZE_GO_PATH: once_cell::sync::Lazy<PathBuf> = Lazy::new(|| {
        let test_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root_manifest = test_manifest.parent().unwrap();
        let build_output = Command::new("cargo")
            .arg("build")
            .arg("--release")
            .args([
                "--manifest-path",
                root_manifest.join("Cargo.toml").to_str().unwrap(),
            ])
            .output()
            .expect("failed to build componentize-go");

        if !build_output.status.success() {
            panic!("{}", String::from_utf8_lossy(&build_output.stderr));
        }

        root_manifest.join("target/release/componentize-go")
    });

    struct App {
        /// The path to the example application
        path: String,
        /// The WIT world to target
        world: Option<String>,
        /// The output path of the wasm file
        wasm_path: String,
        /// The path to the directory containing the WIT files
        wit_path: Option<String>,
        /// The child process ID of a running wasm app
        process: Option<Child>,
        /// Any tests that need to be compiled and run as such
        tests: Option<Vec<Test>>,
    }

    #[derive(Clone)]
    struct Test {
        should_fail: bool,
        pkg_path: String,
    }

    impl App {
        /// Create a new app runner.
        fn new(
            path: &str,
            world: Option<&str>,
            tests: Option<Vec<Test>>,
            is_component: bool,
        ) -> Self {
            let path = componentize_go::utils::make_path_absolute(&PathBuf::from(path))
                .expect("failed to make app path absolute");

            let world = if is_component {
                Some(world.expect("WIT world must be specified").to_string())
            } else {
                None
            };

            let wit_path = if is_component {
                Some(
                    path.join("wit")
                        .to_str()
                        .expect("wit_path is not valid unicode")
                        .to_string(),
                )
            } else {
                None
            };

            App {
                path: path
                    .clone()
                    .to_str()
                    .expect("app path is not valid unicode")
                    .to_string(),
                world,
                wasm_path: path
                    .join("main.wasm")
                    .to_str()
                    .expect("wasm_path is not valid unicode")
                    .to_string(),
                wit_path,
                process: None,
                tests,
            }
        }

        fn build_test_modules(&self) -> Result<()> {
            let test_pkgs = self.tests.as_ref().expect("missing test_pkg_paths");

            self.generate_bindings()?;

            let mut test_cmd = Command::new(COMPONENTIZE_GO_PATH.as_path());
            test_cmd
                .args(["-w", self.world.as_ref().expect("missing WIT world")])
                .args(["-d", self.wit_path.as_ref().expect("missing WIT path")])
                .arg("test")
                .arg("--wasip1");

            // Add all the paths to the packages that have unit tests to compile
            for test in test_pkgs.iter() {
                test_cmd.args(["--pkg", &test.pkg_path]);
            }

            // `go test -c` needs to be in the same path as the go.mod file.
            test_cmd.current_dir(&self.path);

            let test_output = test_cmd.output().expect(&format!(
                "failed to execute componentize-go for \"{}\"",
                self.path
            ));

            if !test_output.status.success() {
                return Err(anyhow!(
                    "failed to build application \"{}\": {}",
                    self.path,
                    String::from_utf8_lossy(&test_output.stderr)
                ));
            }

            Ok(())
        }

        fn run_module(&self) -> Result<()> {
            let output = Command::new("wasmtime")
                .arg("run")
                .arg(&self.wasm_path)
                .output()?;

            if !output.status.success() {
                return Err(anyhow!(
                    "Failed to run wasm module for application at '{}':\n{} ",
                    &self.wasm_path,
                    String::from_utf8_lossy(&output.stdout)
                ));
            }

            Ok(())
        }

        fn run_test_modules(&self) -> Result<()> {
            let example_dir = PathBuf::from(&self.path);
            if let Some(tests) = &self.tests {
                let mut test_errors: Vec<String> = vec![];
                for test in tests.iter() {
                    let wasm_file = example_dir.join(componentize_go::cmd_test::get_test_filename(
                        &PathBuf::from(&test.pkg_path),
                    ));
                    match Command::new("wasmtime")
                        .args(["run", wasm_file.to_str().unwrap()])
                        .output()
                    {
                        Ok(output) => {
                            let succeeded = output.status.success();
                            if test.should_fail && succeeded {
                                test_errors.push(format!(
                                    "The '{}' tests should have failed",
                                    test.pkg_path
                                ));
                            } else if !test.should_fail && !succeeded {
                                test_errors.push(format!("The '{}' tests should have passed, but failed with the following output:\n\n{}", test.pkg_path, String::from_utf8_lossy(&output.stdout)));
                            }
                        }
                        Err(e) => {
                            test_errors.push(format!(
                                "Failed to run wasmtime for '{}': {}",
                                test.pkg_path, e
                            ));
                        }
                    }
                }

                if !test_errors.is_empty() {
                    let err_msg = format!(
                        "{}{}{}",
                        "\n====================\n",
                        &test_errors.join("\n\n====================\n"),
                        "\n\n====================\n"
                    );
                    return Err(anyhow!(err_msg));
                }
            } else {
                return Err(anyhow!(
                    "Please include the test_pkg_paths when creating App::new()"
                ));
            }

            Ok(())
        }

        fn build_module(&self) -> Result<()> {
            // Build component
            let mut build_cmd = Command::new(COMPONENTIZE_GO_PATH.as_path());
            build_cmd
                .arg("build")
                .arg("--wasip1")
                .args(["-o", &self.wasm_path]);

            // Run `go build` in the same directory as the go.mod file.
            build_cmd.current_dir(&self.path);

            let build_output = build_cmd.output().expect(&format!(
                "failed to execute componentize-go for \"{}\"",
                self.path
            ));

            if !build_output.status.success() {
                return Err(anyhow!(
                    "failed to build application \"{}\": {}",
                    self.path,
                    String::from_utf8_lossy(&build_output.stderr)
                ));
            }

            Ok(())
        }

        fn build_component(&self) -> Result<()> {
            self.generate_bindings()?;

            // Build component
            let mut build_cmd = Command::new(COMPONENTIZE_GO_PATH.as_path());
            build_cmd
                .args(["-w", self.world.as_ref().expect("missing WIT world")])
                .args(["-d", self.wit_path.as_ref().expect("missing WIT path")])
                .arg("build")
                .args(["-o", &self.wasm_path]);

            // Run `go build` in the same directory as the go.mod file.
            build_cmd.current_dir(&self.path);

            let build_output = build_cmd.output().expect(&format!(
                "failed to execute componentize-go for \"{}\"",
                self.path
            ));

            if !build_output.status.success() {
                return Err(anyhow!(
                    "failed to build application \"{}\": {}",
                    self.path,
                    String::from_utf8_lossy(&build_output.stderr)
                ));
            }

            Ok(())
        }

        fn generate_bindings(&self) -> Result<()> {
            let bindings_output = Command::new(COMPONENTIZE_GO_PATH.as_path())
                .args(["-w", self.world.as_ref().expect("missing WIT world")])
                .args(["-d", self.wit_path.as_ref().expect("missing WIT path")])
                .arg("bindings")
                .args(["-o", &self.path])
                .current_dir(&self.path)
                .output()
                .expect(&format!(
                    "failed to generate bindings for application \"{}\"",
                    &self.path
                ));
            if !bindings_output.status.success() {
                return Err(anyhow!(
                    "{}",
                    String::from_utf8_lossy(&bindings_output.stderr)
                ));
            }

            // Tidy Go mod
            let tidy_output = Command::new("go")
                .arg("mod")
                .arg("tidy")
                .current_dir(&self.path)
                .output()
                .expect("failed to tidy Go mod");
            if !tidy_output.status.success() {
                return Err(anyhow!("{}", String::from_utf8_lossy(&tidy_output.stderr)));
            }

            Ok(())
        }

        /// Run the app and check the output.
        async fn run_component(&mut self, route: &str, expected_response: &str) -> Result<()> {
            let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a free port");
            let addr = listener.local_addr().expect("Failed to get local address");
            let port = addr.port();
            drop(listener);

            let child = Command::new("wasmtime")
                .arg("serve")
                .args(["--addr", &format!("0.0.0.0:{port}")])
                .arg("-Sp3,cli")
                .arg("-Wcomponent-model-async")
                .arg(&self.wasm_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start wasmtime serve");

            // Storing for cleanup on drop.
            self.process = Some(child);

            let start = std::time::Instant::now();
            loop {
                match reqwest::get(format!("http://localhost:{port}{route}")).await {
                    Ok(r) => {
                        let actual = r.text().await.expect("Failed to read response");
                        assert_eq!(&actual, expected_response);
                        return Ok(());
                    }
                    Err(e) => {
                        if start.elapsed() > Duration::from_secs(5) {
                            return Err(anyhow!("Unable to reach the app: {e}"));
                        }
                    }
                }
            }
        }
    }

    impl Drop for App {
        fn drop(&mut self) {
            if let Some(child) = &mut self.process {
                _ = child.kill()
            }
        }
    }

    #[test]
    fn example_wasip1() {
        let app = App::new("../examples/wasip1", None, None, false);
        app.build_module().expect("failed to build app module");
        app.run_module().expect("failed to run app module");
    }

    #[tokio::test]
    async fn example_wasip2() {
        let unit_tests = vec![
            Test {
                should_fail: false,
                pkg_path: String::from("./unit_tests_should_pass"),
            },
            Test {
                should_fail: true,
                pkg_path: String::from("./unit_tests_should_fail"),
            },
        ];

        let mut app = App::new(
            "../examples/wasip2",
            Some("wasip2-example"),
            Some(unit_tests),
            true,
        );

        app.build_component().expect("failed to build app");

        app.run_component("/", "Hello, world!")
            .await
            .expect("app failed to run");

        app.build_test_modules()
            .expect("failed to build app unit tests");

        app.run_test_modules()
            .expect("tests succeeded/failed when they should not have");
    }

    #[tokio::test]
    async fn example_wasip3() {
        let mut app = App::new("../examples/wasip3", Some("wasip3-example"), None, true);
        app.build_component().expect("failed to build app");
        app.run_component("/hello", "Hello, world!")
            .await
            .expect("app failed to run");
    }
}
