//! `docs/decoder-api.md` and the code agree (spec 007, task T011): the version line is
//! `DECODER_API_VERSION` and the document has the sections the contract names.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

fn document() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join("docs/decoder-api.md")).unwrap()
}

#[test]
fn the_version_line_is_the_version_constant() {
    let doc = document();
    let line = doc
        .lines()
        .find(|l| l.starts_with("Version:"))
        .expect("a `Version:` line");
    assert_eq!(
        line.trim_start_matches("Version:").trim(),
        la_decoders::DECODER_API_VERSION
    );
    // It is the second paragraph, right after the title.
    let paragraphs: Vec<&str> = doc.split("\n\n").collect();
    assert_eq!(paragraphs.get(1).map(|p| p.trim()), Some(line));
}

#[test]
fn the_document_has_the_sections_of_the_contract() {
    let doc = document();
    for section in [
        "## Rules",
        "## What a decoder provides",
        "## What the framework guarantees a decoder",
        "## What a decoder must not do",
        "## Descriptor and message keys",
        "## Generators",
        "## Registering a decoder",
        "## Versioning",
    ] {
        assert!(
            doc.lines().any(|l| l == section),
            "section `{section}` is missing"
        );
    }
}

#[test]
fn every_error_code_a_decoder_can_cause_is_named_where_it_matters() {
    let doc = document();
    for code in [
        la_decoders::error::INVALID_OUTPUT,
        la_decoders::error::INVALID_SETUP,
        la_decoders::error::FAILED,
        la_decoders::error::STACK_CYCLE,
        la_decoders::error::STACK_INCOMPATIBLE,
        la_decoders::error::STACK_TOO_DEEP,
        la_decoders::error::TOO_MANY_INSTANCES,
    ] {
        assert!(doc.contains(code), "{code} is not in the document");
    }
}

#[test]
fn the_block_shapes_and_the_rows_of_a_lane_are_in_the_document() {
    let doc = document();
    for name in ["Part", "Whole", "lane_rows"] {
        assert!(doc.contains(name), "`{name}` is not in the document");
    }
    assert_eq!(la_decoders::DECODER_API_VERSION, "1.2");
    assert!(
        doc.contains("1.2 added"),
        "the versioning paragraph does not say what 1.2 added"
    );
}
