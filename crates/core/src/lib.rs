//! The filesystem- and subprocess-free core of componentize-go.
//!
//! Every operation here is a pure byte transform: WIT sources in, generated Go
//! or encoded wasm out. Nothing in this crate touches the filesystem, spawns a
//! process, or talks to the network, which is what lets it compile to
//! `wasm32-unknown-unknown` and run as a component with zero WASI imports.
//!
//! The native driver wraps these functions with filesystem I/O; the component
//! wrapper in `crates/component` exposes them over WIT. Both go through this
//! one implementation so the two paths cannot drift.

use anyhow::{Context, Result, anyhow, bail};
use wit_parser::{
    CloneMaps, Package, PackageId, PackageName, Resolve, Stability, UnresolvedPackageGroup, World,
    WorldId,
};

pub use wit_bindgen_go::remote_pkg_version;
pub use wit_parser;

/// The `wasi_snapshot_preview1` reactor adapter.
///
/// Sourced from wasmtime's `wasi-preview1-component-adapter-provider` crate
/// rather than checked in, so updating it is a version bump. That crate is
/// versioned in lockstep with wasmtime, so keep it aligned with the wasmtime
/// used in `tests/`.
pub use wasi_preview1_component_adapter_provider::WASI_SNAPSHOT_PREVIEW1_REACTOR_ADAPTER as WASIP1_SNAPSHOT_ADAPT;

/// A single file, identified by a path relative to the root of its
/// [`WitSource`] (for inputs) or to the output directory (for generated code).
#[derive(Clone, Debug)]
pub struct File {
    pub path: String,
    pub contents: Vec<u8>,
}

/// One WIT input, corresponding to a single `-d`/`--wit-path` argument.
///
/// Paths are relative to the root of that argument, and their structure carries
/// the same meaning the filesystem layout does for `wit-parser`'s `push_path`:
///
/// * `*.wit` at the root — parsed together as this source's package.
/// * `deps/<name>/*.wit` — a dependency package.
/// * `deps/<name>.wit` — a single-file dependency package.
/// * `deps/<name>.{wasm,wat}` — a wasm-encoded dependency package.
///
/// A source whose root holds a single `.wasm`/`.wat` and no `.wit` is itself a
/// wasm-encoded package.
#[derive(Clone, Debug, Default)]
pub struct WitSource {
    /// Where this source came from — the `--wit-path` argument, natively. Used
    /// only so that errors name the offending input, which matters when several
    /// sources are in play and `componentize-go.toml` discovery can add more.
    pub name: String,
    pub files: Vec<File>,
}

impl WitSource {
    pub fn new(name: impl Into<String>, files: Vec<File>) -> Self {
        Self {
            name: name.into(),
            files,
        }
    }
}

/// Options for Go bindings generation.
///
/// Deliberately has no `format` field: `gofmt` is a subprocess and so is
/// unavailable in wasm. Formatting is the caller's job, applied to the returned
/// files, which keeps the two drivers producing identical bytes here.
#[derive(Clone, Debug, Default)]
pub struct BindingsOptions {
    pub generate_stubs: bool,
    pub pkg_name: Option<String>,
    pub export_pkg_name: Option<String>,
    pub include_versions: bool,
}

fn is_wasm_encoded(path: &str) -> bool {
    path.ends_with(".wasm") || path.ends_with(".wat")
}

fn decode_package(resolve: &mut Resolve, name: &str, bytes: &[u8]) -> Result<PackageId> {
    // `.wat` needs converting to binary first; `wit-component` only decodes wasm.
    let owned;
    let bytes = if name.ends_with(".wat") {
        owned = wat::parse_bytes(bytes)
            .with_context(|| format!("failed to parse `{name}` as WAT"))?
            .into_owned();
        &owned
    } else {
        bytes
    };

    let decoded = wit_component::decode(bytes)
        .with_context(|| format!("failed to decode `{name}` as a WIT package"))?;
    let pkg = decoded.package();
    let remap = resolve.merge(decoded.resolve().clone())?;
    Ok(remap.packages[pkg.index()])
}

/// Split a source's files into its main package and its `deps/` entries, then
/// push all of them into `resolve`, returning the id of the main package.
fn push_source(resolve: &mut Resolve, source: &WitSource) -> Result<PackageId> {
    let mut root_wit: Vec<&File> = Vec::new();
    let mut root_encoded: Vec<&File> = Vec::new();
    // dep name -> files, preserving the `deps/<name>/...` grouping.
    let mut dep_dirs: std::collections::BTreeMap<String, Vec<&File>> = Default::default();
    let mut dep_files: Vec<&File> = Vec::new();

    for file in &source.files {
        let path = file.path.replace('\\', "/");
        let rest = path.strip_prefix("deps/");
        match rest {
            None => {
                if path.contains('/') {
                    // Not at the root and not under `deps/`; wit-parser ignores
                    // these, so do the same rather than guessing.
                    continue;
                }
                if is_wasm_encoded(&path) {
                    root_encoded.push(file);
                } else if path.ends_with(".wit") {
                    root_wit.push(file);
                }
            }
            Some(rest) => match rest.split_once('/') {
                // `deps/<name>/...` — a directory package.
                Some((name, _)) => dep_dirs.entry(name.to_string()).or_default().push(file),
                // `deps/<name>.{wit,wasm,wat}` — a single-file package.
                None => dep_files.push(file),
            },
        }
    }

    // Wasm-encoded packages must land first: text packages may reference them,
    // and `push_groups` only resolves against what is already in `resolve`.
    for file in dep_files.iter().filter(|f| is_wasm_encoded(&f.path)) {
        decode_package(resolve, &file.path, &file.contents)?;
    }
    for files in dep_dirs.values() {
        for file in files.iter().filter(|f| is_wasm_encoded(&f.path)) {
            decode_package(resolve, &file.path, &file.contents)?;
        }
    }

    if root_wit.is_empty() {
        // A source that is itself a wasm-encoded package.
        return match root_encoded.as_slice() {
            [file] => decode_package(resolve, &file.path, &file.contents),
            [] => bail!("no WIT files found in source"),
            _ => bail!("source root contains multiple wasm-encoded packages"),
        };
    }

    let main = parse_group(&root_wit)?;
    let mut deps = Vec::new();
    for files in dep_dirs.values() {
        let text: Vec<&File> = files
            .iter()
            .copied()
            .filter(|f| f.path.ends_with(".wit"))
            .collect();
        if !text.is_empty() {
            deps.push(parse_group(&text)?);
        }
    }
    for file in dep_files.iter().filter(|f| f.path.ends_with(".wit")) {
        deps.push(parse_group(&[file])?);
    }

    // `push_groups` topologically sorts main against its dependencies, which is
    // why the deps cannot simply be pushed one at a time.
    Ok(resolve.push_groups(main, deps)?)
}

fn parse_group(files: &[&File]) -> Result<UnresolvedPackageGroup> {
    let mut map = wit_parser::SourceMap::default();
    for file in files {
        let text = std::str::from_utf8(&file.contents)
            .with_context(|| format!("`{}` is not valid UTF-8", file.path))?;
        map.push(std::path::Path::new(&file.path), text);
    }
    map.parse().map_err(|(_, e)| e.into())
}

/// Resolve WIT sources into a [`Resolve`] and the world to target.
///
/// Mirrors the CLI's semantics: each source is resolved independently and then
/// merged, so the same package appearing in several sources is consolidated
/// rather than duplicated. Naming more than one world merges them into a
/// synthetic `componentize-go:union/union` world.
pub fn resolve_wit(
    sources: &[WitSource],
    worlds: &[String],
    features: &[String],
    all_features: bool,
) -> Result<(Resolve, WorldId)> {
    if sources.is_empty() {
        bail!("no WIT sources provided");
    }

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

    let packages = sources
        .iter()
        .map(|source| {
            // Resolve into a scratch `Resolve` and merge, which consolidates a
            // package referenced by more than one source.
            let mut tmp = Resolve {
                all_features,
                features: resolve.features.clone(),
                ..Default::default()
            };
            let pkg = push_source(&mut tmp, source)
                .with_context(|| format!("failed to parse WIT for path [{}]", source.name))?;
            let consolidated = resolve.merge(tmp)?;
            Ok(consolidated.packages[pkg.index()])
        })
        .collect::<Result<Vec<_>>>()?;

    let worlds = worlds
        .iter()
        .map(|world| {
            packages
                .iter()
                .find_map(|&pkg| resolve.select_world(&[pkg], Some(world)).ok())
                .ok_or_else(|| {
                    anyhow!("no world named `{world}` found in any of the loaded WIT packages")
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let world = match &worlds[..] {
        [] => packages
            .iter()
            .find_map(|&pkg| resolve.select_world(&[pkg], None).ok())
            .ok_or_else(|| anyhow!("no default world found in any of the loaded WIT packages"))?,
        &[world] => world,
        worlds => {
            let union_package = resolve.packages.alloc(Package {
                name: PackageName {
                    namespace: "componentize-go".into(),
                    name: "union".into(),
                    version: None,
                },
                docs: Default::default(),
                interfaces: Default::default(),
                worlds: Default::default(),
            });

            let union_world = resolve.worlds.alloc(World {
                name: "union".into(),
                imports: Default::default(),
                exports: Default::default(),
                package: Some(union_package),
                docs: Default::default(),
                stability: Stability::Unknown,
                includes: Default::default(),
                span: Default::default(),
            });

            resolve.packages[union_package]
                .worlds
                .insert("union".into(), union_world);

            for &world in worlds {
                resolve.merge_worlds(world, union_world, &mut CloneMaps::default())?;
            }

            union_world
        }
    };

    Ok((resolve, world))
}

/// Generate Go bindings for `world`, returning the files to write.
pub fn generate_bindings(
    resolve: &mut Resolve,
    world: WorldId,
    opts: &BindingsOptions,
) -> Result<Vec<File>> {
    let mut files = Default::default();

    wit_bindgen_go::Opts {
        generate_stubs: opts.generate_stubs,
        // Always off: `gofmt` is a subprocess. Callers format the result.
        format: wit_bindgen_go::Format::False,
        pkg_name: opts.pkg_name.clone(),
        export_pkg_name: opts.export_pkg_name.clone(),
        include_versions: opts.include_versions,
        ..Default::default()
    }
    .build()
    .generate(resolve, world, &mut files)?;

    Ok(files
        .iter()
        .map(|(path, contents)| File {
            path: path.to_string(),
            contents: contents.to_vec(),
        })
        .collect())
}

/// Embed the component-type custom section describing `world` into `module`.
pub fn embed_wit(module: &[u8], resolve: &Resolve, world: WorldId) -> Result<Vec<u8>> {
    let mut wasm = module.to_vec();
    wit_component::embed_component_metadata(
        &mut wasm,
        resolve,
        world,
        wit_component::StringEncoding::UTF8,
    )?;
    Ok(wasm)
}

/// Encode a core module into a component.
///
/// `adapter` overrides the bundled `wasi_snapshot_preview1` reactor adapter,
/// which is used when `None`.
pub fn module_to_component(module: &[u8], adapter: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut encoder = wit_component::ComponentEncoder::default()
        .validate(true)
        .module(module)?
        .adapter(
            wasi_preview1_component_adapter_provider::WASI_SNAPSHOT_PREVIEW1_ADAPTER_NAME,
            adapter.unwrap_or(WASIP1_SNAPSHOT_ADAPT),
        )?;

    encoder
        .encode()
        .context("failed to encode component from module")
}
