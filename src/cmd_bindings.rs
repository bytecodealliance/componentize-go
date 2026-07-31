use crate::utils::make_path_absolute;
use anyhow::{Context, Result};
use componentize_go_core::BindingsOptions;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use wit_parser::{Resolve, WorldId};

/// Format Go source with `gofmt`, if it is on `PATH`.
///
/// Generation itself happens in `componentize-go-core`, which cannot spawn a
/// process, so formatting is applied here to the emitted bytes instead. That
/// keeps the native and component code paths producing identical output.
fn gofmt(path: &Path) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let output = match Command::new("gofmt")
        .stdin(Stdio::from(file))
        .stderr(Stdio::inherit())
        .output()
    {
        Ok(output) => output,
        // Not installed: generated code is still valid, just unformatted.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).context("failed to run `gofmt`"),
    };

    if output.status.success() {
        std::fs::write(path, output.stdout)
            .with_context(|| format!("failed to write '{}'", path.display()))?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn generate_bindings(
    resolve: &mut Resolve,
    world: WorldId,
    generate_stubs: bool,
    should_format: bool,
    output: Option<&Path>,
    pkg_name: Option<String>,
    export_pkg_name: Option<String>,
    include_versions: bool,
) -> Result<()> {
    // If the user wants to create a package rather than a standalone binary, provide them with the
    // go.bytecodealliance.org/pkg version that needs to be placed in their go.mod file
    let message = pkg_name.as_ref().map(|_| {
        format!(
            "Success! Please add the following line to your 'go.mod' file:\n\nrequire {}",
            componentize_go_core::remote_pkg_version()
        )
    });

    let files = componentize_go_core::generate_bindings(
        resolve,
        world,
        &BindingsOptions {
            generate_stubs,
            pkg_name,
            export_pkg_name,
            include_versions,
        },
    )?;

    let output_path = match output {
        Some(p) => make_path_absolute(p)?,
        None => PathBuf::from("."),
    };

    for file in &files {
        let file_path = output_path.join(&file.path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&file_path, &file.contents)
            .with_context(|| format!("failed to write '{}'", file_path.display()))?;

        if should_format && file.path.ends_with(".go") {
            gofmt(&file_path)?;
        }
    }

    if let Some(msg) = message {
        println!("{msg}");
    }

    Ok(())
}
