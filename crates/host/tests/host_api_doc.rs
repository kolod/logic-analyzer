//! `docs/host-api.md` and the code agree (spec 006, task T087).
//!
//! The document's version line must be `HOST_API_VERSION`; every command in its table must exist
//! in `Command`; and every `Command` variant a person can trigger must be in the table.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::BTreeSet;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn document() -> String {
    std::fs::read_to_string(root().join("docs/host-api.md")).unwrap()
}

/// The variants of `pub enum Command`, read from its source.
fn variants() -> BTreeSet<String> {
    let source = std::fs::read_to_string(root().join("crates/host/src/command.rs")).unwrap();
    let start = source.find("pub enum Command {").unwrap();
    let body = &source[start..];
    let end = body.find("\n}\n").unwrap();
    body[..end]
        .lines()
        .filter(|l| l.starts_with("    ") && !l.starts_with("     "))
        .filter_map(|l| {
            let name: String = l
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            name.chars()
                .next()
                .filter(char::is_ascii_uppercase)
                .map(|_| name)
        })
        .collect()
}

/// The command names of the table in the "Commands" section: the leading word of every code span
/// in the second column.
fn documented() -> BTreeSet<String> {
    let text = document();
    let start = text.find("\n## Commands\n").expect("a Commands section");
    let section = &text[start + 1..];
    let end = section[3..].find("\n## ").map_or(section.len(), |e| e + 3);
    let mut names = BTreeSet::new();
    for row in section[..end].lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<&str> = row.split('|').collect();
        let Some(cell) = cells.get(2) else { continue };
        let mut rest = *cell;
        while let Some(open) = rest.find('`') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('`') else { break };
            let span = &after[..close];
            let name: String = span
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                names.insert(name);
            }
            rest = &after[close + 1..];
        }
    }
    names
}

#[test]
fn the_version_line_is_the_version_of_the_code() {
    let text = document();
    let line = text
        .lines()
        .find(|l| l.starts_with("Version: "))
        .expect("a `Version:` line");
    assert_eq!(line, format!("Version: {}", la_host::HOST_API_VERSION));
}

#[test]
fn every_documented_command_exists_and_every_command_is_documented() {
    let code = variants();
    let doc = documented();
    assert!(
        doc.len() > 40,
        "only {} commands parsed from the table",
        doc.len()
    );
    for name in &doc {
        assert!(
            code.contains(name),
            "the document names {name}, which is not a command"
        );
    }
    // The commands that only exist to provoke failures in tests are not documented.
    let missing: Vec<&String> = code
        .iter()
        .filter(|v| !v.starts_with("Test") && !doc.contains(*v))
        .collect();
    assert!(
        missing.is_empty(),
        "commands missing from the document: {missing:?}"
    );
}

#[test]
fn the_design_contract_points_to_the_final_document() {
    let design = std::fs::read_to_string(
        root().join("specs/006-ui-shell-signal-view/contracts/host-api.md"),
    )
    .unwrap();
    assert!(
        design.contains("docs/host-api.md"),
        "the design contract must say where the authoritative text is"
    );
    assert!(
        design.contains("authoritative"),
        "and that it is authoritative"
    );
}
