//! Integration tests for the `run-pipeline` binary (review / plan / full).
//!
//! These exercise the binary end-to-end via real subprocesses, without
//! requiring a live Claude Code CLI or network access.

mod common;

use common::*;
use std::fs;
use std::process::Command;

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
    // Aggregate phase requires at least one review report (guard). In hermes
    // mode the review phase targets <root>/<project>/reviews/<ts>/ — pinned
    // via AUTO_DEV_TIMESTAMP so the fixture lands in the right directory.
    let ts = "20260101_000000";
    let review_dir = td.path.join(project).join("reviews").join(ts);
    fs::create_dir_all(&review_dir).unwrap();
    fs::write(
        review_dir.join("code-review.md"),
        "### [MINOR] sample\nDescription\nFile: `src/lib.rs`\nLine: 1\n",
    )
    .unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "plan",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .env("AUTO_DEV_TIMESTAMP", "20260101_000000")
        .output()
        .expect("spawn run-pipeline");

    assert!(status.status.success(), "run-pipeline plan exited non-zero");

    // A plan file should have been written under <root>/<project>/plans/.
    let plans_dir = td.path.join(project).join("plans");
    assert!(plans_dir.exists(), "plans dir not created");
    let mut plans: Vec<_> = fs::read_dir(&plans_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        // Only markdown plans; the aggregator also writes a JSON sidecar
        // (<ts>-plan.json) which must not be picked up here.
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.ends_with("-plan.md"))
        })
        .collect();
    plans.sort_by_key(|e| e.file_name());
    assert!(!plans.is_empty(), "no plan file written");
    let plan = fs::read_to_string(plans[0].path()).unwrap();
    assert!(
        plan.contains("Auto-Dev Fix Plan"),
        "plan content unexpected"
    );
}

/// Guard (architecture plan Task 4): hermes-mode review phase only PRINTS
/// instructions — if the agent never ran them, the review dir contains no
/// reports. Aggregate must fail fast with an actionable message instead of
/// silently producing an empty plan.
#[test]
fn integration_run_pipeline_plan_fails_on_missing_reviews() {
    let td = TempDir::new("run-plan-guard");
    let project = "e2e-guard";

    let init = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&td.path)
        .status()
        .expect("git init");
    assert!(init.success(), "git init failed");

    // Review dir exists but contains NO *-review.md files.
    fs::create_dir_all(td.path.join(project).join("reviews")).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "plan",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .output()
        .expect("spawn run-pipeline");

    assert!(
        !out.status.success(),
        "aggregate must fail when no review reports exist"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("no review reports found"),
        "error should name the missing-reviews cause, got:\n{}",
        combined
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
    // Aggregate phase requires at least one review report (guard); hermes
    // review dir is <root>/<project>/reviews/<ts>/ with the pinned timestamp.
    let ts = "20260101_000000";
    let review_dir = td.path.join(project).join("reviews").join(ts);
    fs::create_dir_all(&review_dir).unwrap();
    fs::write(
        review_dir.join("code-review.md"),
        "### [MINOR] sample\nDescription\nFile: `src/lib.rs`\nLine: 1\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_run-pipeline"))
        .args([
            td.path.to_str().unwrap(),
            "full",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .env("AUTO_DEV_TIMESTAMP", "20260101_000000")
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
