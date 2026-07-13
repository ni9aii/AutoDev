//! Integration tests for the Auto-Dev Pipeline binaries.
//!
//! These exercise the binaries end-to-end (real subprocess via CARGO_BIN_EXE_*)
//! or through the `Pipeline` struct with a `MockRunner`, without requiring a
//! live Claude Code CLI or network access.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A self-cleaning temporary directory under the system temp dir.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("autodev-it-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Fake review report in the format the aggregator's parser understands
/// (### [SEVERITY] Title + File:/Description: body). The body deliberately
/// repeats "File:" / "Description:" lead-ins to verify the aggregator strips
/// them from the description (no duplication in the generated plan), and uses a
/// multi-line description so we also catch the clean_body prefix-strip bug
/// (the first line after "Description:" must be preserved, not dropped).
const FAKE_REVIEW: &str = r#"# Code Review Report

### [CRITICAL] SQL injection in db.rs
File: `src/db.rs`
Description: User input concatenated into a query string without parameterization.
This second line must also survive aggregation.
File: `src/db.rs`
Description: This is a duplicate metadata line that must be stripped.

### [IMPORTANT] Missing error handling in main.rs
File: `src/main.rs`
Description: `unwrap()` on a fallible call can panic at runtime.
"#;

/// Run `review-aggregator` against a temp dev-notes tree and assert a plan is
/// produced with the expected sections.
#[test]
fn integration_review_aggregator_produces_plan() {
    let td = TempDir::new("aggregator");
    let project = "testproj";
    let timestamp = "20260101_000000";
    let reviews_dir = td.path.join(project).join("reviews").join(timestamp);
    fs::create_dir_all(&reviews_dir).unwrap();
    fs::write(reviews_dir.join("code-review.md"), FAKE_REVIEW).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_review-aggregator"))
        .args([
            "--dev-notes",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .status()
        .expect("spawn review-aggregator");

    assert!(status.success(), "review-aggregator exited non-zero");

    let plan_path = td
        .path
        .join(project)
        .join("plans")
        .join(format!("{}-plan.md", timestamp));
    assert!(
        plan_path.exists(),
        "plan file not created at {:?}",
        plan_path
    );

    let plan = fs::read_to_string(&plan_path).unwrap();
    assert!(plan.contains("Do Now"), "plan missing 'Do Now' section");
    assert!(
        plan.contains("SQL injection"),
        "plan missing aggregated finding"
    );
    assert!(plan.contains("CRITICAL"), "plan missing severity label");
    // The aggregator must strip parser-metadata lines (File:/Description:) from
    // the description body, so the generated plan must not duplicate them.
    // FAKE_REVIEW has 2 findings, both with a File:, so expect exactly 2 (one per
    // finding), not 4 (which would mean the body metadata leaked through).
    let file_count = plan.matches("**File:**").count();
    let desc_count = plan.matches("**Description:**").count();
    assert_eq!(file_count, 2, "File: metadata count wrong (leak/dup?)");
    assert_eq!(
        desc_count, 2,
        "Description: metadata count wrong (leak/dup?)"
    );
    assert!(
        !plan.contains("**Description:** This is a duplicate metadata line"),
        "duplicate metadata line leaked into plan body"
    );
    // clean_body must strip the "Description:" prefix but KEEP the text that
    // follows it — including the first line. Regression guard for the
    // prefix-strip bug where the whole first line was dropped.
    assert!(
        plan.contains("User input concatenated into a query string"),
        "first line of description was dropped by clean_body"
    );
    assert!(
        plan.contains("This second line must also survive aggregation"),
        "multi-line description body was not preserved"
    );
}

/// Run `review-aggregator` when there are no reviews: it should still succeed
/// and create an empty/placeholder plan rather than panic.
#[test]
fn integration_review_aggregator_no_reviews_is_ok() {
    let td = TempDir::new("aggregator-empty");
    let project = "empty";
    fs::create_dir_all(td.path.join(project).join("reviews")).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_review-aggregator"))
        .args([
            "--dev-notes",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .status()
        .expect("spawn review-aggregator");

    assert!(
        status.success(),
        "review-aggregator should handle empty input"
    );
}

/// Run `run-pipeline` with `--json` and assert stdout is valid JSON with the
/// expected top-level fields (logs go to stderr, so stdout is JSON-only).
#[test]
fn integration_run_pipeline_json_is_valid() {
    let td = TempDir::new("run-json");
    // Point dev-notes at a temp dir so the run doesn't touch real notes.
    let status = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            ".",
            "review",
            "--hermes-mode",
            "--json",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn run-pipeline");

    assert!(status.status.success(), "run-pipeline exited non-zero");

    let stdout = String::from_utf8_lossy(&status.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("stdout is not valid JSON");
    assert_eq!(value["status"], "success");
    assert_eq!(value["phase"], "review");
    assert_eq!(value["mode"], "hermes");
    assert!(value["version"].is_string());
    assert!(value["output_dir"].is_string());
}

/// End-to-end: `run-pipeline <git-repo> plan` must pass the git prerequisite
/// check and produce a plan file (empty when there are no reviews). Exercises
/// the full binary path: arg parse → prerequisites → aggregate phase.
#[test]
fn integration_run_pipeline_plan_end_to_end() {
    let td = TempDir::new("run-plan");
    let project = "e2e";
    // run-pipeline requires the target to be a git repository.
    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&td.path)
        .status()
        .expect("git init");
    assert!(init.success(), "git init failed");

    fs::create_dir_all(td.path.join(project).join("reviews")).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "plan",
            "--hermes-mode",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .output()
        .expect("spawn run-pipeline");

    assert!(status.status.success(), "run-pipeline plan exited non-zero");

    // A plan file should have been written under <root>/<project>/plans/.
    let plans_dir = td.path.join(project).join("plans");
    assert!(plans_dir.exists(), "plans dir not created");
    let mut plans: Vec<_> = fs::read_dir(&plans_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    plans.sort_by_key(|e| e.file_name());
    assert!(!plans.is_empty(), "no plan file written");
    let plan = fs::read_to_string(plans[0].path()).unwrap();
    assert!(
        plan.contains("Auto-Dev Fix Plan"),
        "plan content unexpected"
    );
}

/// End-to-end: `run-pipeline <git-repo> full` must drive all four phases
/// (review → aggregate → execute → verify) to completion. This is the test
/// that exercises BOTH companion binaries — `review-aggregator` (aggregate) and
/// `ci-check` (verify) — via the sibling-resolution path, so it guards against
/// the "works locally, fails when companions aren't on $PATH" regression.
///
/// Hermetic: the temp repo has no `origin` remote, so `ci-check` can't reach
/// the GitHub API and falls back to local tests only. A tiny `Makefile` with a
/// no-op `test` target satisfies the verify phase's local-test requirement
/// without a network or a real toolchain invocation.
#[test]
fn integration_run_pipeline_full_end_to_end() {
    let td = TempDir::new("run-full");
    let project = "e2e-full";

    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&td.path)
        .status()
        .expect("git init");
    assert!(init.success(), "git init failed");

    // A Makefile makes detect_test_runner pick `make test`; the target is a
    // no-op so the verify phase's local-test check passes cheaply.
    fs::write(td.path.join("Makefile"), "test:\n\t@echo ok\n").unwrap();
    fs::create_dir_all(td.path.join(project).join("reviews")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "full",
            "--hermes-mode",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .output()
        .expect("spawn run-pipeline");

    assert!(
        out.status.success(),
        "run-pipeline full exited non-zero.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Aggregate phase must have written a plan (proves review-aggregator ran).
    let plans_dir = td.path.join(project).join("plans");
    assert!(
        plans_dir.exists(),
        "plans dir not created by aggregate phase"
    );
    assert!(
        fs::read_dir(&plans_dir).unwrap().next().is_some(),
        "no plan file written by aggregate phase"
    );

    // Verify phase must have reached completion (proves ci-check ran).
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("Verification complete"),
        "verify phase did not complete (ci-check may not have run)"
    );
    assert!(
        combined.contains("Pipeline complete"),
        "pipeline did not reach completion"
    );
}

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
        .args([td.path.to_str().unwrap(), "release", "--hermes-mode"])
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
