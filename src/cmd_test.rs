use crate::utils::{check_go_version, make_path_absolute};
use anyhow::{Result, anyhow};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Compiles a Go application to a wasm module with `go test -c`.
///
/// If the module is not going to be adapted to the component model,
/// set the `only_wasip1` arg to true.
pub fn build_test_module(
    path: &Path,
    output_dir: Option<&PathBuf>,
    go_path: Option<&PathBuf>,
    only_wasip1: bool,
) -> Result<PathBuf> {
    let go = match &go_path {
        Some(p) => make_path_absolute(p)?,
        None => PathBuf::from("go"),
    };

    check_go_version(&go)?;

    let test_wasm_path = {
        // The directory in which the test component will be placed
        let test_dir = match output_dir {
            Some(p) => make_path_absolute(p)?,
            None => std::env::current_dir()?,
        };

        test_dir.join(get_test_filename(path))
    };

    // Ensuring the newly compiled wasm file overwrites any previously-existing wasm file
    if test_wasm_path.exists() {
        std::fs::remove_file(&test_wasm_path)?;
    }

    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
    }

    // The -buildmode flag mutes the unit test output, so it is ommitted
    let module_args = [
        "test",
        "-c",
        "-ldflags=-checklinkname=0",
        "-o",
        test_wasm_path
            .to_str()
            .expect("the combined paths of 'output-dir' and 'pkg' are not valid unicode"),
        path.to_str().expect("pkg path is not valid unicode"),
    ];

    // TODO: for when we figure out how wasip2 tests are to be run
    #[allow(unused_variables)]
    let component_args = [
        "test",
        "-c",
        "-buildmode=c-shared",
        "-ldflags=-checklinkname=0",
        "-o",
        test_wasm_path
            .to_str()
            .expect("the combined paths of 'output-dir' and 'pkg' are not valid unicode"),
        path.to_str().expect("pkg path is not valid unicode"),
    ];

    let output = if only_wasip1 {
        Command::new(&go)
            .args(module_args)
            .env("GOOS", "wasip1")
            .env("GOARCH", "wasm")
            .output()?
    } else {
        unimplemented!("Please use the --wasip1 flag when building unit tests");

        // TODO: for when we figure out how wasip2 tests are to be run
        #[allow(unreachable_code)]
        Command::new(&go)
            .args(component_args)
            .env("GOOS", "wasip1")
            .env("GOARCH", "wasm")
            .output()?
    };

    if !output.status.success() {
        return Err(anyhow!(
            "'go test -c' command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(test_wasm_path)
}

// Format the test filename based on the package path (see unit tests for more details).
pub fn get_test_filename(path: &Path) -> String {
    let components: Vec<&str> = path
        .components()
        .filter_map(|c| match c {
            // Filter out the `/` and `.`
            std::path::Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect();

    let tail = if components.len() >= 2 {
        &components[components.len() - 2..]
    } else {
        &components[..]
    };

    format!("test_{}.wasm", tail.join("_"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_test_filename() {
        let tests = [
            ("./foo/bar/baz", "test_bar_baz.wasm"),
            ("./foo/bar", "test_foo_bar.wasm"),
            ("./bar", "test_bar.wasm"),
            ("/usr/bin/foo/bar/baz", "test_bar_baz.wasm"),
        ];

        for (input, expected) in tests.iter() {
            let input_string = input.to_string();
            let actual = get_test_filename(&PathBuf::from(input_string));
            assert_eq!(actual, expected.to_string());
        }
    }
}
