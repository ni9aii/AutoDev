//! Integration test guarding the destructive `release` path of `run-pipeline`.

mod common;

use common::*;
use std::process::Command;

/// `run-pipeline <repo> release` without `--release-version` must fail fast at
/// argument validation — BEFORE building, tagging, or pushing anything. Guards
/// the destructive release path: a misinvocation must never create a git tag or
/// hit the network.
#[test]
fn integration_run_pipeline_release_requires_version() {
    let td = TempDir::new("run-release-noversion");

    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&td.path)
        .status()
        .expect("git init");
    assert!(init.success(), "git init failed");

    let out = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "release",
            // Point dev-notes at the temp dir: Pipeline::new resolves the
            // dev-notes root and creates the output dir BEFORE release-phase
            // argument validation, so without this flag every test run would
            // leak an empty directory tree into the real ~/obsidian-vault.
            "--dev-notes-root",
            td.path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn run-pipeline");

    assert!(
        !out.status.success(),
        "release without --release-version must fail, but exited 0"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("release-version"),
        "error message should point to the --release-version flag, got: {}",
        combined
    );

    // Critical: no tag may have been created by a failed/misinvoked release.
    let tags = Command::new("git")
        .args(["tag"])
        .current_dir(&td.path)
        .output()
        .expect("git tag");
    assert!(
        tags.stdout.is_empty(),
        "release must not create a git tag when it fails validation"
    );
}
