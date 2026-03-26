use crate::utils::make_path_absolute;
use anyhow::Result;
use std::path::{Path, PathBuf};
use wit_parser::{Resolve, WorldId};

#[allow(clippy::too_many_arguments)]
pub fn generate_bindings(
    resolve: &mut Resolve,
    world: WorldId,
    generate_stubs: bool,
    should_format: bool,
    output: Option<&Path>,
    pkg_name: Option<String>,
    export_pkg_name: Option<String>,
) -> Result<()> {
    let mut files = Default::default();

    let format = if should_format {
        wit_bindgen_go::Format::True
    } else {
        wit_bindgen_go::Format::False
    };

    // If the user wants to create a package rather than a standalone binary, provide them with the
    // go.bytecodealliance.org/pkg version that needs to be placed in their go.mod file
    let mut message: Option<String> = None;
    if pkg_name.is_some() {
        message = Some(format!(
            "Success! Please add the following line to your 'go.mod' file:\n\nrequire {}",
            wit_bindgen_go::remote_pkg_version()
        ));
    }

    wit_bindgen_go::Opts {
        generate_stubs,
        format,
        pkg_name,
        export_pkg_name,
        ..Default::default()
    }
    .build()
    .generate(resolve, world, &mut files)?;

    let output_path = match output {
        Some(p) => make_path_absolute(p)?,
        None => PathBuf::from("."),
    };

    for (name, contents) in files.iter() {
        let file_path = output_path.join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&file_path, contents)?;
    }

    if let Some(msg) = message {
        println!("{msg}");
    }

    Ok(())
}
