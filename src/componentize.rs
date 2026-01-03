use crate::common::parse_wit;
use anyhow::{Context, Result, anyhow};
use std::{path::PathBuf, process::Command};

/// Ensure that the Go version is compatible with the embedded Wasm tooling.
fn check_go_version(go_path: &PathBuf) -> Result<()> {
    let output = Command::new(go_path).arg("version").output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "'go version' command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let version_string = String::from_utf8(output.stdout)?;
    let version_regex = regex::Regex::new(r"go(\d+)\.(\d+)\.(\d+)").unwrap();
    let semver = version_regex.captures(&version_string).map(|caps| {
        (
            caps[1].parse::<u32>().unwrap(), // Major
            caps[2].parse::<u32>().unwrap(), // Minor
            caps[3].parse::<u32>().unwrap(), // Patch
        )
    });

    if let Some((major, minor, patch)) = semver {
        // TODO: there might be a patch number correlated with wasip3.
        if major == 1 && minor >= 25 {
            Ok(())
        } else {
            Err(anyhow!(
                "Go version is not valid. Expected '^1.25.0', found '{}.{}.{}'",
                major,
                minor,
                patch
            ))
        }
    } else {
        Err(anyhow!(
            "Failed to parse Go version from: {}",
            version_string
        ))
    }
}

/// Update the WebAssembly core module to use the component model ABI.
pub fn core_module_to_component(wasm_file: &PathBuf) -> Result<()> {
    // In the rare case the snapshot needs to be updated, the latest version
    // can be found here: https://github.com/bytecodealliance/wasmtime/releases
    const WASIP1_SNAPSHOT: &[u8] = include_bytes!("wasi_snapshot_preview1.reactor.wasm");
    let wasm: Vec<u8> = wat::Parser::new().parse_file(wasm_file)?;

    let mut encoder = wit_component::ComponentEncoder::default().validate(true);
    encoder = encoder.module(&wasm)?;
    encoder = encoder.adapter("wasi_snapshot_preview1", WASIP1_SNAPSHOT)?;

    let bytes = encoder
        .encode()
        .context("failed to encode component from module")?;

    std::fs::write(wasm_file, bytes)
        .context(format!("failed to write `{}`", wasm_file.display()))?;

    Ok(())
}

pub fn embed_wit(
    wasm_file: &PathBuf,
    wit_path: &[PathBuf],
    world: Option<&str>,
    features: &[String],
    all_features: bool,
) -> Result<()> {
    let mut wasm = wat::Parser::new().parse_file(wasm_file)?;
    let (resolve, world_id) = parse_wit(wit_path, world, features, all_features)?;
    wit_component::embed_component_metadata(
        &mut wasm,
        &resolve,
        world_id,
        wit_component::StringEncoding::UTF8,
    )?;
    std::fs::write(wasm_file, wasm)
        .context(format!("failed to write `{}`", wasm_file.display()))?;
    Ok(())
}

/// Compiles a Go application to WebAssembly core.
pub fn build_wasm_core_module(
    go_module: Option<PathBuf>,
    out: Option<PathBuf>,
    go_path: Option<PathBuf>,
) -> Result<PathBuf> {
    let go = match &go_path {
        Some(p) => {
            if p.is_relative() {
                std::env::current_dir()?.join(p)
            } else {
                p.to_path_buf()
            }
        }
        None => PathBuf::from("go"),
    };

    check_go_version(&go)?;

    let out_path_buf = match &out {
        Some(p) => {
            if p.is_relative() {
                std::env::current_dir()?.join(p)
            } else {
                p.to_path_buf()
            }
        }
        None => std::env::current_dir()?.join("main.wasm"),
    };

    // The `go build` command doesn't overwrite the output file, which causes
    // issues if the `componentize-go componentize` command is run multiple times.
    if out_path_buf.exists() {
        std::fs::remove_file(&out_path_buf)?;
    }

    let out_path = out_path_buf
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("Output path is not valid unicode"))?;

    let module_path = match &go_module {
        Some(p) => {
            if !p.is_dir() {
                return Err(anyhow!("Module path '{}' is not a directory", p.display()));
            }
            p.to_str()
                .ok_or_else(|| anyhow!("Module path is not valid unicode"))?
        }
        None => ".",
    };

    let output = Command::new(&go)
        .args([
            "build",
            "-C",
            module_path,
            "-buildmode=c-shared",
            "-ldflags=-checklinkname=0",
            "-o",
            &out_path,
        ])
        .env("GOOS", "wasip1")
        .env("GOARCH", "wasm")
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "'go build' command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(PathBuf::from(out_path))
}
