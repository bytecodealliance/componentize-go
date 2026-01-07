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

    // TODO: Once the patch is merged in Big Go, this needs to be removed.
    async fn patched_go_path() -> PathBuf {
        let test_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root_manifest = test_manifest.parent().unwrap();

        // Determine OS and architecture
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            "windows" => "windows",
            bad_os => panic!("OS not supported: {bad_os}"),
        };

        // Map to Go's naming conventions
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "amd64",
            bad_arch => panic!("ARCH not supported: {bad_arch}"),
        };

        let go_dir = format!("go-{os}-{arch}-bootstrap");
        let go_path = root_manifest.join(&go_dir);
        let go_bin = go_path.join("bin").join("go");

        // Skip if already installed
        if go_bin.exists() {
            return go_bin;
        }

        // Download the patched Go toolchain
        let archive_name = format!("{go_dir}.tbz");
        let archive_path = root_manifest.join(&archive_name);
        let download_url = format!(
            "https://github.com/dicej/go/releases/download/go1.25.5-wasi-on-idle/{archive_name}"
        );

        println!("Downloading patched Go from {download_url}");
        let response = reqwest::get(&download_url)
            .await
            .expect("Failed to download patched Go");

        std::fs::write(
            &archive_path,
            response.bytes().await.expect("Failed to read download"),
        )
        .expect("Failed to write archive");

        // Extract the archive
        println!("Extracting {} to {}", archive_name, root_manifest.display());
        let tar_file = std::fs::File::open(&archive_path).expect("Failed to open archive");
        let tar_decoder = bzip2::read::BzDecoder::new(tar_file);
        let mut archive = tar::Archive::new(tar_decoder);
        archive
            .unpack(root_manifest)
            .expect("Failed to extract archive");

        // Clean up archive
        std::fs::remove_file(&archive_path).ok();

        go_bin
    }

    struct App<'a> {
        path: &'a str,
        world: &'a str,
        wasm_path: PathBuf,
        process: Option<Child>,
    }

    impl<'a> App<'a> {
        /// Create a new app runner.
        fn new(path: &'a str, world: &'a str) -> Self {
            let wasm_path = PathBuf::from(path).join("main.wasm");
            App {
                path,
                world,
                wasm_path,
                process: None,
            }
        }

        /// Build the app with componentize-go.
        fn build(&self, go_path: Option<PathBuf>) -> Result<()> {
            let app_path = PathBuf::from(self.path);
            let wit_path = app_path.join("wit");

            // Generate bindings
            let bindings_output = Command::new(COMPONENTIZE_GO_PATH.as_path())
                .args(["-w", self.world, "-d", wit_path.to_str().unwrap()])
                .arg("bindings")
                .args(["-o", self.path])
                .output()
                .expect(&format!(
                    "failed to generate bindings for application \"{}\"",
                    self.path
                ));
            if !bindings_output.status.success() {
                return Err(anyhow!(
                    "{}",
                    String::from_utf8_lossy(&bindings_output.stderr)
                ));
            }

            // Build component
            let mut build_cmd = Command::new(COMPONENTIZE_GO_PATH.as_path());
            build_cmd
                .args(["-w", self.world, "-d", wit_path.to_str().unwrap()])
                .arg("componentize")
                .args(["--mod", self.path])
                .args(["-o", self.wasm_path.to_str().unwrap()]);

            if let Some(go) = go_path {
                build_cmd.args(["--go", go.to_str().unwrap()]);
            }

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

        /// Run the app and check the output.
        async fn run(&mut self, route: &str, expected_response: &str) -> Result<()> {
            let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to a free port");
            let addr = listener.local_addr().expect("Failed to get local address");
            let port = addr.port();
            drop(listener);

            let child = Command::new("wasmtime")
                .arg("serve")
                .args(["--addr", &format!("0.0.0.0:{port}")])
                .args(["-Sp3,cli", self.wasm_path.to_str().unwrap()])
                .arg("-Wcomponent-model-async")
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

    impl<'a> Drop for App<'a> {
        fn drop(&mut self) {
            if let Some(child) = &mut self.process {
                _ = child.kill()
            }
        }
    }

    #[tokio::test]
    async fn example_wasip2() {
        let mut app = App::new("../examples/wasip2", "wasip2-example");
        app.build(None).expect("failed to build app");
        app.run("/", "Hello, world!")
            .await
            .expect("app failed to run");
    }

    #[tokio::test]
    async fn example_wasip3() {
        let mut app = App::new("../examples/wasip3", "wasip3-example");
        app.build(Some(patched_go_path().await))
            .expect("failed to build app");
        app.run("/hello", "Hello, world!")
            .await
            .expect("app failed to run");
    }
}
