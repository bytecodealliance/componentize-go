use crate::utils::{check_go_version, make_path_absolute};
use anyhow::{Result, anyhow};
use std::{path::PathBuf, process::Command};

/// Compiles a Go application to a wasm module with `go build`.
///
/// If the module is not going to be adapted to the component model,
/// set the `only_wasip1` arg to true.
pub fn build_module(
    out: Option<&PathBuf>,
    go_path: Option<&PathBuf>,
    only_wasip1: bool,
) -> Result<PathBuf> {
    let go = match &go_path {
        Some(p) => make_path_absolute(p)?,
        None => PathBuf::from("go"),
    };

    check_go_version(&go)?;

    let out_path_buf = match &out {
        Some(p) => make_path_absolute(p)?,
        None => std::env::current_dir()?.join("main.wasm"),
    };

    // Ensuring the newly compiled wasm file overwrites any previously-existing wasm file
    if out_path_buf.exists() {
        std::fs::remove_file(&out_path_buf)?;
    }

    let out_path = out_path_buf
        .to_str()
        .ok_or_else(|| anyhow!("Output path is not valid unicode"))?;

    // The -buildmode flag mutes the module's output, so it is ommitted
    let module_args = [
        "build",
        "-C",
        ".",
        "-ldflags=-checklinkname=0",
        "-o",
        out_path,
    ];

    let component_args = [
        "build",
        "-C",
        ".",
        "-buildmode=c-shared",
        "-ldflags=-checklinkname=0",
        "-o",
        out_path,
    ];

    let output = if only_wasip1 {
        Command::new(&go)
            .args(module_args)
            .env("GOOS", "wasip1")
            .env("GOARCH", "wasm")
            .output()?
    } else {
        Command::new(&go)
            .args(component_args)
            .env("GOOS", "wasip1")
            .env("GOARCH", "wasm")
            .output()?
    };

    if !output.status.success() {
        return Err(anyhow!(
            "'go build' command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(PathBuf::from(out_path))
}
