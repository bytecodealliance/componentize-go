use crate::{bindings::generate_bindings, componentize};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{ffi::OsString, path::PathBuf};

/// A tool that creates Go WebAssembly components.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Options {
    #[command(flatten)]
    pub common: Common,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Args, Clone, Debug)]
pub struct Common {
    /// The location of the WIT document(s).
    ///
    /// This may be specified more than once, for example:
    /// `-d ./wit/deps -d ./wit/app`.
    ///
    /// These paths can be either directories containing `*.wit` files, `*.wit`
    /// files themselves, or `*.wasm` files which are wasm-encoded WIT packages.
    #[arg(long, short = 'd')]
    pub wit_path: Vec<PathBuf>,

    /// Name of world to target (or default world if `None`).
    #[arg(long, short = 'w')]
    pub world: Option<String>,

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
    /// Build a Go WebAssembly component.
    Componentize(Componentize),

    /// Generate Go bindings for the world.
    Bindings(Bindings),
}

#[derive(Parser)]
pub struct Componentize {
    /// The path to the Go binary (or look for binary in PATH if `None`).
    #[arg(long)]
    pub go: Option<PathBuf>,

    /// Final output path for the component (or `./main.wasm` if `None`).
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// The directory containing the "go.mod" file (or current directory if `None`).
    #[arg(long = "mod")]
    pub mod_path: Option<PathBuf>,
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

    /// Whether or not `gofmt` should be used (if present) to format generated code.
    #[arg(long)]
    pub format: bool,

    /// The name of the Go module housing the generated bindings (or "wit_component" if `None`).
    ///
    /// This option is used if the generated bindings will be used as a library.
    #[arg(long)]
    pub mod_name: Option<String>,
}

pub fn run<T: Into<OsString> + Clone, I: IntoIterator<Item = T>>(args: I) -> Result<()> {
    let options = Options::parse_from(args);
    match options.command {
        Command::Componentize(opts) => componentize(options.common, opts),
        Command::Bindings(opts) => bindings(options.common, opts),
    }
}

fn componentize(common: Common, componentize: Componentize) -> Result<()> {
    // Step 1: Build a WebAssembly core module using Go.
    let core_module = componentize::build_wasm_core_module(
        componentize.mod_path,
        componentize.output,
        componentize.go,
    )?;

    // Step 2: Embed the WIT documents in the core module.
    componentize::embed_wit(
        &core_module,
        &common.wit_path,
        common.world.as_deref(),
        &common.features,
        common.all_features,
    )?;

    // Step 3: Update the core module to use the component model ABI.
    componentize::core_module_to_component(&core_module)?;
    Ok(())
}

fn bindings(common: Common, bindings: Bindings) -> Result<()> {
    generate_bindings(
        common.wit_path.as_ref(),
        common.world.as_deref(),
        &common.features,
        common.all_features,
        bindings.generate_stubs,
        bindings.format,
        bindings.output.as_deref(),
        bindings.mod_name,
    )
}
