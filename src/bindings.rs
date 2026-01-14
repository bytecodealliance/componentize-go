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
    mod_name: Option<String>,
) -> Result<()> {
    let (resolve, world) = parse_wit(wit_path, world, features, all_features)?;
    let mut files = Default::default();

    let format = if should_format {
        wit_bindgen_go::Format::True
    } else {
        wit_bindgen_go::Format::False
    };

    wit_bindgen_go::Opts {
        generate_stubs,
        format,
        mod_name,
        ..Default::default()
    }
    .build()
    .generate(&resolve, world, &mut files)?;

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

    Ok(())
}
