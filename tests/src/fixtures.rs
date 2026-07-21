use crate::harness::App;
use crate::wasmtime::{State, engine};
use anyhow::Result;
use std::{env, path::PathBuf};
use wasmtime::{Store, component::*};

/// Retrieves the tests/fixtures directory
fn app_dir(name: &str) -> Result<PathBuf> {
    let cwd = env::current_dir()?;
    let app_dir = cwd.parent().unwrap().join("tests/fixtures").join(name);
    Ok(app_dir)
}

#[tokio::test]
async fn fixture_multiple_worlds() -> Result<()> {
    let app_dir = app_dir("multiple-worlds")?;
    let mut app = App::new(
        &app_dir,
        &[&app_dir.join("wit1"), &app_dir.join("wit2")],
        &["world1", "world2"],
        None,
        true,
    );
    app.build_component().expect("failed to build app");
    app.run_component("/hello", "Hello, world!")
        .await
        .expect("app failed to run");
    Ok(())
}

#[test]
fn fixture_named_implements() -> Result<()> {
    // Build the app with componentize-go
    let app_dir = app_dir("named-implements")?;
    let app = App::new(
        &app_dir,
        &[&app_dir.join("wit")],
        &["named-implements"],
        None,
        true,
    );
    app.build_component().expect("failed to build app");

    // Link the same `store` interface under three import names, each backed
    // by a distinct host implementation, to verify that named `implements`
    // imports route to their own instances.
    let engine = engine()?;
    let component = Component::from_file(&engine, app_dir.join("main.wasm"))?;

    let mut linker = Linker::<State>::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    for (import, prefix) in [
        ("primary", "primary"),
        ("secondary", "secondary"),
        ("foo:baz/store@0.1.0", "bare"),
    ] {
        linker
            .instance(import)?
            .func_wrap("get", move |_store, (key,): (String,)| {
                Ok((format!("{prefix}:{key}"),))
            })?;
    }

    let mut store = Store::new(&engine, State::new());
    let instance = linker.instantiate(&mut store, &component)?;
    let run = instance.get_typed_func::<(), ()>(&mut store, "run")?;
    run.call(&mut store, ())?;

    assert_eq!(
        store.data().stdout(),
        b"primary:color\nsecondary:color\nbare:color\n"
    );

    Ok(())
}

#[test]
fn fixture_memory_pressure() -> Result<()> {
    // Build the app with componentize-go
    let app_dir = app_dir("memory-pressure")?;
    let app = App::new(
        &app_dir,
        &[&app_dir.join("wit")],
        &["memory-pressure"],
        None,
        true,
    );
    app.build_component().expect("failed to build app");

    // Setup and run with custom wasmtime impl
    bindgen!({
        path: "fixtures/memory-pressure/wit",
        world: "memory-pressure",
    });

    impl MemoryPressureImports for State {
        fn get_str(&mut self) -> String {
            String::from("Hello!")
        }
    }

    let engine = engine()?;
    let component = Component::from_file(&engine, app_dir.join("main.wasm"))?;

    let mut linker = Linker::<State>::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
    MemoryPressure::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)?;

    let mut store = Store::new(&engine, State::new());
    let tests = MemoryPressure::instantiate(&mut store, &component, &linker)?;

    // Run component a generous number of times to make sure nothing panics
    let num_invocations = 10_000;
    for _ in 0..num_invocations {
        tests.call_run(&mut store)?;
    }
    assert_eq!(
        store.data().stdout(),
        b"Hello!\n".repeat(num_invocations).as_slice()
    );

    Ok(())
}
