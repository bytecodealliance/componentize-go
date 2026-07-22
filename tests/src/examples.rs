use crate::harness::{App, COMPONENTIZE_GO_PATH, Test};
use anyhow::{Result, anyhow};
use std::{env, path::Path, process::Command, time::Duration};

#[test]
fn example_wasip1() {
    let app = App::new(Path::new("../examples/wasip1"), &[], &[], None, false);
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

    let cwd = env::current_dir().unwrap();
    let app_dir = cwd.parent().unwrap().join("examples").join("wasip2");
    let mut app = App::new(
        &app_dir,
        &[&app_dir.join("wit")],
        &["wasip2-example"],
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
    let cwd = env::current_dir().unwrap();
    let app_dir = cwd.parent().unwrap().join("examples").join("wasip3");
    let mut app = App::new(
        &app_dir,
        &[&app_dir.join("wit")],
        &["wasip3-example"],
        None,
        true,
    );
    app.build_component().expect("failed to build app");
    app.run_component("/hello", "Hello, world!")
        .await
        .expect("app failed to run");
}

// This test is more verbose because it uses unique componentize-go and wasmtime
// options that didn't make sense to abstract.
#[tokio::test]
async fn example_sdk() -> Result<()> {
    let cwd = env::current_dir()?;
    let app_dir = cwd
        .parent()
        .unwrap()
        .join("examples")
        .join("sdk")
        .join("component");

    // Build component
    let build_output = Command::new(COMPONENTIZE_GO_PATH.as_path())
        .arg("build")
        .current_dir(&app_dir)
        .output()?;
    if !build_output.status.success() {
        return Err(anyhow!(
            "failed to build application \"{}\": {}",
            app_dir.display(),
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    // Run component
    let mut child = Command::new("wasmtime")
        .arg("run")
        .args(["-S", "p3,inherit-network"])
        .args(["-W", "component-model-async"])
        .arg(app_dir.join("main.wasm"))
        .spawn()?;

    // Send HTTP request to component
    let start = std::time::Instant::now();
    loop {
        match reqwest::get(format!("http://localhost:6767")).await {
            Ok(r) => {
                let actual = r.text().await.expect("Failed to read response");
                assert_eq!(&actual, "Hello from Go + wasi:sockets!");
                break;
            }
            Err(e) => {
                if start.elapsed() > Duration::from_secs(30) {
                    return Err(anyhow!("Unable to reach the app: {e}"));
                }
            }
        }
    }

    // Kill running component
    child.kill()?;
    Ok(())
}
