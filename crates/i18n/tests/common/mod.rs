//! Helpers shared by the integration tests (not a test target itself). No assertions here.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::path::{Path, PathBuf};

use la_i18n::{Key, Language, Localizer};

/// The repository root (two levels above this crate).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every stable error code defined by the product's crates: constants of the form
/// `pub const NAME: &str = "domain.code";` in `error.rs` files under `crates/*/src`.
pub fn error_codes() -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(workspace_root().join("crates"))
        .unwrap()
        .flatten()
    {
        rust_files(&entry.path().join("src"), &mut files);
    }
    let mut codes = Vec::new();
    for file in files {
        if file.file_name().is_none_or(|n| n != "error.rs") {
            continue;
        }
        for line in std::fs::read_to_string(&file).unwrap().lines() {
            let Some(rest) = line.trim().strip_prefix("pub const ") else {
                continue;
            };
            let Some((_, value)) = rest.split_once(": &str = \"") else {
                continue;
            };
            let Some(code) = value.strip_suffix("\";") else {
                continue;
            };
            let valid = code.split_once('.').is_some_and(|(domain, name)| {
                !domain.is_empty()
                    && !name.is_empty()
                    && domain.chars().all(|c| c.is_ascii_lowercase())
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
            if valid {
                codes.push((code.to_owned(), file.clone()));
            }
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

/// A localizer with the real embedded resources.
pub fn localizer(language: Language) -> Localizer {
    Localizer::new(language)
}

/// The key of an error code.
pub fn key_of(code: &str) -> Key {
    Key::for_error_code(code)
}
