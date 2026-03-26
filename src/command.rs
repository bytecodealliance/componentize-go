use crate::{
    cmd_bindings::generate_bindings,
    cmd_build::build_module,
    cmd_test::build_test_module,
    utils::{embed_wit, module_to_component},
};
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::{ffi::OsString, path::PathBuf};

/// A tool that creates Go WebAssembly components.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Options {
    #[command(flatten)]
    pub wit_opts: WitOpts,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Args, Clone, Debug)]
pub struct WitOpts {
    /// The location of the WIT document(s).
    ///
    /// This may be specified more than once, for example:
    /// `-d ./wit/deps -d ./wit/app`.
    ///
    /// These paths can be either directories containing `*.wit` files, `*.wit`
    /// files themselves, or `*.wasm` files which are wasm-encoded WIT packages.
    ///
    /// Note that, unless `--ignore-toml-files` is specified, `componentize-go`
    /// will also use `go list` to scan the current Go module and its
    /// dependencies to find any `componentize-go.toml` files.  The WIT
    /// documents referenced by any such files will be added to this list
    /// automatically.
    #[arg(long, short = 'd')]
    pub wit_path: Vec<PathBuf>,

    /// Name of world to target (or default world if not specified).
    ///
    /// This may be specified more than once, in which case the worlds will be
    /// merged.
    ///
    /// Note that, unless `--ignore-toml-files` _or_ at least one `--world`
    /// option is specified, `componentize-go` will use `go list` to scan the
    /// current Go module and its dependencies to find any
    /// `componentize-go.toml` files, and the WIT worlds referenced by any such
    /// files will be used.
    #[arg(long, short = 'w')]
    pub world: Vec<String>,

    #[arg(long)]
    pub ignore_toml_files: bool,

    /// Whether or not to activate all WIT features when processing WIT files.
    ///
    /// This enables using `@unstable` annotations in WIT files.
    #[arg(long)]
    pub all_features: bool,

    /// Comma-separated list of features that should be enabled when processing
    /// WIT files.
    ///
    /// This enables using `@unstable` annotations in WIT files.
    #[arg(long)]
    pub features: Vec<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Build a Go WebAssembly binary.
    Build(Build),

    /// Build Go test WebAssembly binary.
    Test(Test),

    /// Generate Go bindings for a WIT world.
    Bindings(Bindings),
}

#[derive(Parser)]
pub struct Build {
    /// Whether or not to build a WebAssembly module.
    ///
    /// If ommitted, this will build a component.
    #[arg(long)]
    pub wasip1: bool,

    /// Final output path for the component (or `./main.wasm` if `None`).
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// The path to the Go binary (or look for binary in PATH if `None`).
    #[arg(long)]
    pub go: Option<PathBuf>,
}

#[derive(Parser)]
pub struct Test {
    /// Whether or not to build a WebAssembly module.
    ///
    /// If ommitted, this will build a component.
    #[arg(long)]
    pub wasip1: bool,

    /// A package containing Go test files.
    ///
    /// This may be specified more than once, for example:
    /// `--pkg ./cmd/foo --pkg ./cmd/bar`.
    ///
    /// The test components will be named using the last segment of the provided path, for example:
    /// `--pkg ./foo/bar/baz` will result in a file named `test_bar_baz.wasm`
    #[arg(long)]
    pub pkg: Vec<PathBuf>,

    /// Output directory for test components (or current directory if `None`).
    ///
    /// This will be created if it does not already exist.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// The path to the Go binary (or look for binary in PATH if `None`).
    #[arg(long)]
    pub go: Option<PathBuf>,
}

#[derive(Parser)]
pub struct Bindings {
    /// Output directory for bindings (or current directory if `None`).
    ///
    /// This will be created if it does not already exist.
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// If true, generate stub functions for any exported functions and/or resources.
    #[arg(long)]
    pub generate_stubs: bool,

    /// Whether or not `gofmt` should be used (if present in PATH) to format generated code.
    #[arg(long)]
    pub format: bool,

    /// If specified, organize the bindings into a package for use as a library;
    /// otherwise (if None), the bindings will be organized for use as a standalone executable.
    #[arg(long)]
    pub pkg_name: Option<String>,
}

pub fn run<T: Into<OsString> + Clone, I: IntoIterator<Item = T>>(args: I) -> Result<()> {
    let options = Options::parse_from(args);
    match options.command {
        Command::Build(opts) => build(options.wit_opts, opts),
        Command::Bindings(opts) => bindings(options.wit_opts, opts),
        Command::Test(opts) => test(options.wit_opts, opts),
    }
}

fn build(wit_opts: WitOpts, build: Build) -> Result<()> {
    // Build a wasm module using `go build`.
    let module = build_module(build.output.as_ref(), build.go.as_ref(), build.wasip1)?;

    if !build.wasip1 {
        // Embed the WIT documents in the wasip1 component.
        embed_wit(
            &module,
            &wit_opts.wit_path,
            &wit_opts.world,
            wit_opts.ignore_toml_files,
            &wit_opts.features,
            wit_opts.all_features,
        )?;

        // Update the wasm module to use the current component model ABI.
        module_to_component(&module)?;
    }

    Ok(())
}

fn test(wit_opts: WitOpts, test: Test) -> Result<()> {
    if test.pkg.is_empty() {
        return Err(anyhow!("Path to a package containing Go tests is required"));
    }

    for pkg in test.pkg.iter() {
        // Build a wasm module using `go test -c`.
        let module = build_test_module(pkg, test.output.as_ref(), test.go.as_ref(), test.wasip1)?;

        if !test.wasip1 {
            // Embed the WIT documents in the wasm module.
            embed_wit(
                &module,
                &wit_opts.wit_path,
                &wit_opts.world,
                wit_opts.ignore_toml_files,
                &wit_opts.features,
                wit_opts.all_features,
            )?;

            // Update the wasm module to use the current component model ABI.
            module_to_component(&module)?;
        }
    }

    Ok(())
}

fn bindings(wit_opts: WitOpts, bindings: Bindings) -> Result<()> {
    generate_bindings(
        &wit_opts.wit_path,
        &wit_opts.world,
        wit_opts.ignore_toml_files,
        &wit_opts.features,
        wit_opts.all_features,
        bindings.generate_stubs,
        bindings.format,
        bindings.output.as_deref(),
        bindings.pkg_name,
    )
}
