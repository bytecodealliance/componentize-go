use crate::utils::{check_go_version, go_tags_arg, make_path_absolute};
use anyhow::{Result, anyhow, bail};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Compiles a Go application to a wasm module with `go build`.
///
/// If the module is not going to be adapted to the component model,
/// set the `only_wasip1` arg to true.
///
/// `tags` contains any build tags derived from the target world (see
/// [`crate::utils::world_build_tags`]); these are merged with any `-tags`
/// specified via `GOFLAGS` and passed to `go build`.
pub fn build_module(
    out: Option<&PathBuf>,
    go: &Path,
    only_wasip1: bool,
    tags: &[String],
) -> Result<PathBuf> {
    check_go_version(go)?;

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

    let mut args = vec!["build".to_string(), "-C".to_string(), ".".to_string()];
    // The -buildmode flag mutes the module's output, so it is ommitted when
    // building a plain wasip1 module
    if !only_wasip1 {
        args.push("-buildmode=c-shared".to_string());
    }
    args.push("-ldflags=-checklinkname=0".to_string());
    if let Some(tags_arg) = go_tags_arg(tags) {
        args.push(tags_arg);
    }
    args.push("-o".to_string());
    args.push(out_path.to_string());

    let output = Command::new(go)
        .args(&args)
        .env("GOOS", "wasip1")
        .env("GOARCH", "wasm")
        .output()?;

    if !output.status.success() {
        bail!(
            "'go build' command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(PathBuf::from(out_path))
}
