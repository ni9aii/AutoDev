//! Integration tests for the `review-aggregator` binary.
//!
//! These exercise the binary end-to-end (real subprocess via CARGO_BIN_EXE_*),
//! without requiring a live Claude Code CLI or network access.

mod common;

use common::*;
use std::fs;
use std::process::Command;

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

/// Regression (plan finding: empty-plan fallback promised "generating empty
/// plan" then bailed when reviews/ itself didn't exist). A fresh project with
/// NO reviews directory at all must still get an empty plan, not an error.
#[test]
fn integration_review_aggregator_missing_reviews_dir_gets_empty_plan() {
    let td = TempDir::new("aggregator-no-dir");
    let project = "fresh";
    // Deliberately do NOT create <project>/reviews/.

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
        "missing reviews/ dir must produce an empty plan, not an error"
    );

    let plan = td.path.join(project).join("plans").join("empty-plan.md");
    assert!(
        plan.exists(),
        "empty plan not written at {}",
        plan.display()
    );
}
