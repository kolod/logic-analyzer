//! Proves the panic-safety lint policy is enforced (spec FR-009, FR-011, SC-008).
//!
//! Runs `cargo clippy` on the `la-lint-fixture` crate: clean code and test code must
//! pass, and each deliberate violation (enabled by a cfg) must fail with the expected
//! lint. Uses its own target directory so it does not disturb the main build cache.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn run_clippy(target_dir: &Path, cfg: Option<&str>) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace_root())
        .args([
            "clippy",
            "-p",
            "la-lint-fixture",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .env("CARGO_TARGET_DIR", target_dir)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    if let Some(cfg) = cfg {
        cmd.env("RUSTFLAGS", format!("--cfg {cfg}"));
    }
    cmd.output().expect("failed to run cargo clippy")
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn policy_rejects_violations_and_accepts_clean_and_test_code() {
    let target_dir = std::env::temp_dir().join(format!("la-lint-policy-{}", std::process::id()));

    let clean = run_clippy(&target_dir, None);
    assert!(
        clean.status.success(),
        "clean fixture (including relaxed test code) must pass:\n{}",
        text(&clean)
    );

    let cases = [
        ("lint_fixture_unwrap", "unwrap_used"),
        ("lint_fixture_expect", "expect_used"),
        ("lint_fixture_index", "indexing_slicing"),
        ("lint_fixture_arith", "arithmetic_side_effects"),
        ("lint_fixture_panic", "clippy::panic"),
        ("lint_fixture_unsafe", "unsafe"),
    ];
    for (cfg, expected) in cases {
        let out = run_clippy(&target_dir, Some(cfg));
        let output = text(&out);
        assert!(
            !out.status.success(),
            "violation `{cfg}` must be rejected:\n{output}"
        );
        assert!(
            output.contains(expected),
            "violation `{cfg}` must name `{expected}`:\n{output}"
        );
    }

    let _ = std::fs::remove_dir_all(&target_dir);
}
