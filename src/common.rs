use anyhow::Result;
use std::path::{Path, PathBuf};
use wit_parser::{PackageId, Resolve, WorldId};

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
