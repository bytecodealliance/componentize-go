use crate::utils::make_path_absolute;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use wit_parser::{Resolve, WorldId};

/// Module path of the Go runtime package that generated bindings depend on.
const PKG_MODULE: &str = "go.bytecodealliance.org/pkg";

#[allow(clippy::too_many_arguments)]
pub fn generate_bindings(
    resolve: &mut Resolve,
    world: WorldId,
    generate_stubs: bool,
    should_format: bool,
    output: Option<&Path>,
    pkg_name: Option<String>,
    include_versions: bool,
    quiet: bool,
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
    if pkg_name.is_some() && !quiet {
        message = Some(format!(
            "Success! Please make sure your 'go.mod' file requires at least:\n\nrequire {}",
            wit_bindgen_go::remote_pkg_version()
        ));
    }

    wit_bindgen_go::Opts {
        generate_stubs,
        format,
        pkg_name,
        include_versions,
        quiet,
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

        // Standalone bindings come with a fresh `go.mod`. Only write it if there isn't one yet, since an existing
        // `go.mod` belongs to the application. Only change what the generated code depends on.
        if name == "go.mod" && file_path.exists() {
            update_go_mod(&file_path, &String::from_utf8_lossy(contents), quiet)?;
            continue;
        }

        std::fs::write(&file_path, contents)?;
    }

    if let Some(msg) = message {
        eprintln!("{msg}");
    }

    Ok(())
}

/// The go.bytecodealliance.org/pkg version the generated bindings need.
fn required_pkg_version() -> Result<String> {
    let line = wit_bindgen_go::remote_pkg_version();
    let mut parts = line.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some(PKG_MODULE), Some(version)) => Ok(version.to_string()),
        _ => Err(anyhow!("unexpected remote package version `{line}`")),
    }
}

// Bring an existing `go.mod` in line with what the generated bindings need,
// leaving everything else as is.
fn update_go_mod(path: &Path, generated: &str, quiet: bool) -> Result<()> {
    let required = required_pkg_version()?;
    let module = module_path(generated)
        .ok_or_else(|| anyhow!("generated go.mod has no module directive"))?;
    let existing = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut notes = Vec::new();
    let mut contents = existing;
    if let Some(updated) = set_module_path(&contents, module) {
        contents = updated;
        notes.push(format!("module path set to `{module}`"));
    }
    if let Some(updated) = require_at_least(&contents, PKG_MODULE, &required)? {
        contents = updated;
        notes.push(format!("now requires {PKG_MODULE} {required}"));
    }
    if notes.is_empty() {
        return Ok(());
    }

    std::fs::write(path, contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    if !quiet {
        eprintln!("Updated {}: {}", path.display(), notes.join("; "));
    }
    Ok(())
}

/// The module path declared by a `go.mod`, if any.
fn module_path(go_mod: &str) -> Option<&str> {
    go_mod.lines().find_map(|line| {
        let mut fields = line.split("//").next()?.split_whitespace();
        match (fields.next(), fields.next()) {
            (Some("module"), Some(path)) => Some(path.trim_matches('"')),
            _ => None,
        }
    })
}

/// Replace the module path in `go_mod` with `module`. Returns `None` when it
/// already matches.
fn set_module_path(go_mod: &str, module: &str) -> Option<String> {
    let current = module_path(go_mod)?;
    if current == module {
        return None;
    }
    let mut lines: Vec<String> = go_mod.lines().map(str::to_string).collect();
    let line = lines
        .iter_mut()
        .find(|l| module_path(l).is_some())
        .expect("module directive found above");
    *line = line.replacen(current, module, 1);
    let mut updated = lines.join("\n");
    updated.push('\n');
    Some(updated)
}

/// Ensure `go_mod` requires `module` at `version` or newer.
///
/// Returns `Ok(None)` when the existing requirement is already new enough,
/// otherwise the updated file contents. An existing requirement is never
/// lowered, and nothing else in the file is touched.
fn require_at_least(go_mod: &str, module: &str, version: &str) -> Result<Option<String>> {
    let wanted = parse_go_version(version)?;
    let mut in_require_block = false;
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    let mut changed = false;

    for line in go_mod.lines() {
        let code = line.split("//").next().unwrap_or("");
        let mut fields: Vec<&str> = code.split_whitespace().collect();

        if in_require_block {
            if fields.first() == Some(&")") {
                in_require_block = false;
            }
        } else if fields.first() == Some(&"require") {
            if fields.get(1) == Some(&"(") {
                in_require_block = true;
                lines.push(line.to_string());
                continue;
            }
            fields.remove(0);
        } else {
            lines.push(line.to_string());
            continue;
        }

        if fields.first() == Some(&module)
            && let Some(current) = fields.get(1)
        {
            found = true;
            if parse_go_version(current)? < wanted {
                lines.push(line.replacen(current, version, 1));
                changed = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if !found {
        lines.push(String::new());
        lines.push(format!("require {module} {version}"));
        changed = true;
    }

    if !changed {
        return Ok(None);
    }

    let mut updated = lines.join("\n");
    updated.push('\n');
    Ok(Some(updated))
}

/// Parse a Go module version (`v1.2.3`, `v1.2.4-0.20260101000000-abcdef123456`,
/// `v2.0.0+incompatible`). Go module versions are semver with a `v` prefix, and
/// pseudo-versions are prereleases, so they order correctly.
fn parse_go_version(version: &str) -> Result<semver::Version> {
    let trimmed = version
        .strip_prefix('v')
        .ok_or_else(|| anyhow!("invalid Go module version `{version}`"))?;
    semver::Version::parse(trimmed)
        .with_context(|| format!("invalid Go module version `{version}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = PKG_MODULE;

    #[test]
    fn raises_lower_requirement_in_block() {
        let go_mod = "module example.com/app\n\ngo 1.27.1\n\nrequire (\n\tgo.bytecodealliance.org/pkg v0.2.3\n\tgo.wasmcloud.dev/component v0.2.0 // indirect\n)\n";
        let updated = require_at_least(go_mod, PKG, "v0.4.1").unwrap().unwrap();
        assert_eq!(
            updated,
            "module example.com/app\n\ngo 1.27.1\n\nrequire (\n\tgo.bytecodealliance.org/pkg v0.4.1\n\tgo.wasmcloud.dev/component v0.2.0 // indirect\n)\n"
        );
    }

    #[test]
    fn raises_lower_single_line_requirement() {
        let go_mod =
            "module wit_component\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/pkg v0.4.0\n";
        let updated = require_at_least(go_mod, PKG, "v0.4.1").unwrap().unwrap();
        assert_eq!(
            updated,
            "module wit_component\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/pkg v0.4.1\n"
        );
    }

    #[test]
    fn raises_pseudo_version_below_release() {
        let go_mod = "module m\n\nrequire go.bytecodealliance.org/pkg v0.2.4-0.20260911130647-2495ff7eca86\n";
        let updated = require_at_least(go_mod, PKG, "v0.4.1").unwrap().unwrap();
        assert!(updated.contains("go.bytecodealliance.org/pkg v0.4.1\n"));
    }

    #[test]
    fn never_lowers_newer_requirement() {
        let go_mod = "module m\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/pkg v0.5.0\n";
        assert_eq!(require_at_least(go_mod, PKG, "v0.4.1").unwrap(), None);
    }

    #[test]
    fn leaves_equal_requirement_alone() {
        let go_mod = "module m\n\nrequire (\n\tgo.bytecodealliance.org/pkg v0.4.1 // indirect\n)\n";
        assert_eq!(require_at_least(go_mod, PKG, "v0.4.1").unwrap(), None);
    }

    #[test]
    fn adds_missing_requirement_and_keeps_the_rest() {
        let go_mod = "module example.com/app\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/wasi v0.1.1\n\ntool github.com/bytecodealliance/componentize-go\n";
        let updated = require_at_least(go_mod, PKG, "v0.4.1").unwrap().unwrap();
        assert_eq!(
            updated,
            "module example.com/app\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/wasi v0.1.1\n\ntool github.com/bytecodealliance/componentize-go\n\nrequire go.bytecodealliance.org/pkg v0.4.1\n"
        );
    }

    #[test]
    fn ignores_replace_and_similar_module_paths() {
        let go_mod = "module m\n\nrequire go.bytecodealliance.org/pkg/extra v0.1.0\n\nreplace go.bytecodealliance.org/pkg v0.2.3 => ../pkg\n";
        let updated = require_at_least(go_mod, PKG, "v0.4.1").unwrap().unwrap();
        assert!(updated.contains("replace go.bytecodealliance.org/pkg v0.2.3 => ../pkg\n"));
        assert!(updated.ends_with("\nrequire go.bytecodealliance.org/pkg v0.4.1\n"));
    }

    #[test]
    fn module_path_is_set_to_generated_one() {
        let go_mod = "module example.com/worker // the worker\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/pkg v0.4.1\n";
        assert_eq!(module_path(go_mod), Some("example.com/worker"));
        assert_eq!(
            set_module_path(go_mod, "wit_component").unwrap(),
            "module wit_component // the worker\n\ngo 1.27.1\n\nrequire go.bytecodealliance.org/pkg v0.4.1\n"
        );
        assert_eq!(
            set_module_path("module wit_component\n", "wit_component"),
            None
        );
    }

    #[test]
    fn required_version_parses() {
        let version = required_pkg_version().unwrap();
        parse_go_version(&version).unwrap();
    }
}
