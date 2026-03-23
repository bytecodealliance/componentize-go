use anyhow::{Context, Result, anyhow};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use wit_parser::{PackageId, Resolve, WorldId};

// In the rare case the snapshot needs to be updated, the latest version
// can be found here: https://github.com/bytecodealliance/wasmtime/releases
const WASIP1_SNAPSHOT_ADAPT: &[u8] = include_bytes!("wasi_snapshot_preview1.reactor.wasm");

pub fn parse_wit(
    paths: &[impl AsRef<Path>],
    world: Option<&str>,
    features: &[String],
    all_features: bool,
) -> Result<(Resolve, WorldId)> {
    // If no WIT directory was provided as a parameter and none were referenced
    // by Go packages, use ./wit by default.
    if paths.is_empty() {
        let paths = &[Path::new("wit")];
        return parse_wit(paths, world, features, all_features);
    }
    debug_assert!(!paths.is_empty(), "The paths should not be empty");

    let mut resolve = Resolve {
        all_features,
        ..Default::default()
    };
    for features in features {
        for feature in features
            .split(',')
            .flat_map(|s| s.split_whitespace())
            .filter(|f| !f.is_empty())
        {
            resolve.features.insert(feature.to_string());
        }
    }

    let mut main_packages: Vec<PackageId> = vec![];
    for path in paths.iter().map(AsRef::as_ref) {
        let (pkg, _files) = resolve.push_path(path)?;
        main_packages.push(pkg);
    }

    let world = resolve.select_world(&main_packages, world)?;
    Ok((resolve, world))
}

// Converts a relative path to an absolute path.
pub fn make_path_absolute(p: &PathBuf) -> Result<PathBuf> {
    if p.is_relative() {
        Ok(std::env::current_dir()?.join(p))
    } else {
        Ok(p.to_owned())
    }
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
        .context(format!("failed to write '{}'", wasm_file.display()))?;
    Ok(())
}

/// Update the wasm module to use the current component model ABI.
pub fn module_to_component(wasm_file: &PathBuf, adapt_file: &Option<PathBuf>) -> Result<()> {
    let wasm: Vec<u8> = wat::Parser::new().parse_file(wasm_file)?;

    let mut encoder = wit_component::ComponentEncoder::default().validate(true);
    encoder = encoder.module(&wasm)?;
    let adapt_bytes = if let Some(adapt) = adapt_file {
        std::fs::read(adapt)
            .context(format!("failed to read adapt file '{}'", adapt.display()))?
    } else {
        WASIP1_SNAPSHOT_ADAPT.to_vec()
    };
    encoder = encoder.adapter("wasi_snapshot_preview1", &adapt_bytes)?;


    let bytes = encoder
        .encode()
        .context("failed to encode component from module")?;

    std::fs::write(wasm_file, bytes)
        .context(format!("failed to write `{}`", wasm_file.display()))?;

    Ok(())
}

/// Ensure that the Go version is compatible with the embedded Wasm tooling.
pub fn check_go_version(go_path: &PathBuf) -> Result<()> {
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
