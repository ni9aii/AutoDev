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

/// JSON sidecar (architecture plan Task 2): alongside `<ts>-plan.md` the
/// aggregator must write `<ts>-plan.json`, a machine-readable mirror whose
/// items match what the markdown parser extracts from the md plan.
#[test]
fn integration_review_aggregator_writes_json_sidecar_matching_markdown() {
    let td = TempDir::new("aggregator-sidecar");
    let project = "sidecarproj";
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

    let plans_dir = td.path.join(project).join("plans");
    let sidecar_path = plans_dir.join(format!("{}-plan.json", timestamp));
    assert!(
        sidecar_path.exists(),
        "JSON sidecar not created at {:?}",
        sidecar_path
    );

    let sidecar_text = fs::read_to_string(&sidecar_path).unwrap();
    let plan_doc: serde_json::Value =
        serde_json::from_str(&sidecar_text).expect("sidecar is not valid JSON");

    // Structure: generated + items array.
    assert!(plan_doc["generated"].is_string(), "missing generated ts");
    let items = plan_doc["items"].as_array().expect("items is not an array");
    assert!(
        !items.is_empty(),
        "sidecar has no items despite FAKE_REVIEW"
    );

    // Item shape and content agreement with the markdown.
    for item in items {
        assert!(
            item["title"].as_str().unwrap_or("").len() > 0,
            "empty title"
        );
        assert!(item["severity"].is_string(), "severity missing");
    }
    let titles: Vec<&str> = items.iter().filter_map(|i| i["title"].as_str()).collect();
    assert!(
        titles.iter().any(|t| t.contains("SQL injection")),
        "sidecar missing the aggregated finding; got {:?}",
        titles
    );

    // Every item title in the JSON appears in the markdown plan too.
    let md = fs::read_to_string(plans_dir.join(format!("{}-plan.md", timestamp))).unwrap();
    for t in &titles {
        assert!(md.contains(t), "sidecar title {t:?} absent from markdown");
    }
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

/// A review whose only finding classifies as "defer" (MINOR severity).
const DEFER_REVIEW: &str = r#"# Code Review Report

### [MINOR] Consider splitting the parser module
File: `src/parse.rs`
Description: The parse module mixes tokenizing and AST construction.
"#;

/// Carry-over end-to-end: run 1 produces plan P1 with a defer item; run 2 with
/// --carry-over-from P1 must embed that item in P2's defer section marked
/// attempt 1, under a "Carried over from" provenance header.
#[test]
fn integration_review_aggregator_carries_over_deferred_items() {
    let td = TempDir::new("aggregator-carryover");
    let project = "carryproj";
    let plans_dir = td.path.join(project).join("plans");

    // --- Run 1: produce P1 with one deferred finding ---
    let ts1 = "20260101_000000";
    let reviews1 = td.path.join(project).join("reviews").join(ts1);
    fs::create_dir_all(&reviews1).unwrap();
    fs::write(reviews1.join("code-review.md"), DEFER_REVIEW).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_review-aggregator"))
        .args([
            "--dev-notes",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
        ])
        .status()
        .expect("spawn review-aggregator (run 1)");
    assert!(status.success(), "run 1 exited non-zero");

    let p1_path = plans_dir.join(format!("{}-plan.md", ts1));
    let p1 = fs::read_to_string(&p1_path).unwrap();
    assert!(
        p1.contains("## 🟡 Defer to Next Phase"),
        "P1 missing defer section"
    );
    assert!(
        p1.contains("Consider splitting the parser module"),
        "P1 missing deferred finding"
    );

    // --- Run 2: a fresh reviews dir + --carry-over-from P1 ---
    let ts2 = "20260102_000000";
    let reviews2 = td.path.join(project).join("reviews").join(ts2);
    fs::create_dir_all(&reviews2).unwrap();
    fs::write(reviews2.join("code-review.md"), DEFER_REVIEW).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_review-aggregator"))
        .args([
            "--dev-notes",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
            "--carry-over-from",
        ])
        .arg(&p1_path)
        .status()
        .expect("spawn review-aggregator (run 2)");
    assert!(status.success(), "run 2 exited non-zero");

    let p2_path = plans_dir.join(format!("{}-plan.md", ts2));
    let p2 = fs::read_to_string(&p2_path).unwrap();
    assert!(
        p2.contains("> Carried over from "),
        "P2 missing carried-over header"
    );
    assert!(
        p2.contains("**Carried over:** from "),
        "P2 carried item missing provenance marker"
    );
    assert!(
        p2.contains(", attempt 1"),
        "P2 carried item not marked attempt 1"
    );
    // Original title/severity/file info is preserved.
    assert!(
        p2.contains("Consider splitting the parser module"),
        "carried item title lost"
    );
    assert!(p2.contains("**Severity:** MINOR"));
    assert!(p2.contains("`src/parse.rs`"));
    // Carried items come before freshly deferred ones.
    let carried_pos = p2.find("### Carried 1:").expect("no carried heading");
    let fresh_pos = p2.find("### Deferred 1:").expect("no fresh defer heading");
    assert!(
        carried_pos < fresh_pos,
        "carried item not at top of defer section"
    );

    // --- Missing carry-over file: warning, not failure ---
    let ts3 = "20260103_000000";
    let reviews3 = td.path.join(project).join("reviews").join(ts3);
    fs::create_dir_all(&reviews3).unwrap();
    fs::write(reviews3.join("code-review.md"), DEFER_REVIEW).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_review-aggregator"))
        .args([
            "--dev-notes",
            "--dev-notes-root",
            td.path.to_str().unwrap(),
            "--project",
            project,
            "--carry-over-from",
            "/nonexistent/plan.md",
        ])
        .status()
        .expect("spawn review-aggregator (run 3)");
    assert!(
        status.success(),
        "missing --carry-over-from file must not fail"
    );
}
