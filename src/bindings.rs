use crate::common::{make_path_absolute, parse_wit};
use anyhow::Result;
use std::path::{Path, PathBuf};

#[allow(clippy::too_many_arguments)]
pub fn generate_bindings(
    wit_path: &[impl AsRef<Path>],
    world: Option<&str>,
    features: &[String],
    all_features: bool,
    generate_stubs: bool,
    should_format: bool,
    output: Option<&Path>,
    pkg_name: Option<String>,
) -> Result<()> {
    let (mut resolve, world) = parse_wit(wit_path, world, features, all_features)?;
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
        ..Default::default()
    }
    .build()
    .generate(&mut resolve, world, &mut files)?;

    let output_path = match output {
        Some(p) => make_path_absolute(&p.to_path_buf())?,
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
