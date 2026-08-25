//! Generation of the prioritized fix plan markdown file.
//!
//! Rendering and parsing both go through the shared typed model
//! (`auto_dev_pipeline::plan`) — one producer, one consumer (Task 1).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use auto_dev_pipeline::markdown;
use auto_dev_pipeline::plan::{self, PlanItem};

use super::findings::{prioritize_findings, Classification, Finding};

/// Convert a parsed finding into a plan item.
fn finding_to_item(f: &Finding) -> PlanItem {
    PlanItem {
        role: f.role.clone(),
        severity: f.severity.clone(),
        title: f.title.clone(),
        description: f.description.clone(),
        file: f.file.clone(),
        line: f.line,
        carried_from: None,
        attempt: 0,
        do_now: f.classification == Classification::DoNow,
    }
}

/// Parse the "## 🟡 Defer to Next Phase" section of a previous plan into items.
/// Items that already carry a provenance marker keep their original origin
/// timestamp and incremented attempt; fresh items get attempt = 1 with
/// `now_ts` as their origin. Returns an empty vec when there is no defer
/// section.
pub(crate) fn parse_carry_over(prev_plan: &str, now_ts: &str) -> Vec<PlanItem> {
    let section = markdown::extract_section(prev_plan, "Defer to Next Phase");
    if section.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();
    for mut item in plan::parse_items(&section) {
        if item.carried_from.is_none() {
            // Fresh item deferred this run: provenance starts now, attempt 1.
            item.carried_from = Some(now_ts.to_string());
            item.attempt = 1;
        } else {
            // Already carried before: increment once per generation (the
            // marker records the attempt at last render).
            item.attempt = item.attempt.saturating_add(1);
        }
        items.push(item);
    }
    items
}

/// Read and parse the previous plan at `path` for carry-over. Any problem
/// (missing file, unreadable, no defer section) is logged as a warning and
/// yields no carried items — never a hard failure. Fresh items are stamped
/// with the PREVIOUS plan's generation timestamp so provenance points at the
/// run that deferred them.
pub(crate) fn read_carry_over(path: &Path, now_ts: &str) -> Vec<PlanItem> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let default_ts = content
                .lines()
                .find_map(|l| l.trim().strip_prefix("> Generated: "))
                .map(|ts| ts.trim().to_string())
                .unwrap_or_else(|| now_ts.to_string());
            let items = parse_carry_over(&content, &default_ts);
            if items.is_empty() {
                auto_dev_pipeline::log::log(&format!(
                    "WARNING: --carry-over-from {} has no 'Defer to Next Phase' items — skipping carry-over",
                    path.display()
                ));
            }
            items
        }
        Err(e) => {
            auto_dev_pipeline::log::log(&format!(
                "WARNING: could not read --carry-over-from {}: {} — skipping carry-over",
                path.display(),
                e
            ));
            Vec::new()
        }
    }
}

pub(crate) fn generate_plan(
    findings: &[Finding],
    output_path: &Path,
    carry_over_from: Option<&Path>,
) -> Result<()> {
    let prioritized = prioritize_findings(findings);
    let mut lines: Vec<String> = Vec::new();

    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");

    lines.push("# Auto-Dev Fix Plan".to_string());
    lines.push(format!("\n> Generated: {}", now));
    lines.push(format!("> Total findings: {}", findings.len()));

    let critical_count = findings.iter().filter(|f| f.severity == "CRITICAL").count();
    let important_count = findings
        .iter()
        .filter(|f| f.severity == "IMPORTANT")
        .count();
    let minor_count = findings.iter().filter(|f| f.severity == "MINOR").count();

    lines.push(format!("> Critical: {}", critical_count));
    lines.push(format!("> Important: {}", important_count));
    lines.push(format!("> Minor: {}", minor_count));
    lines.push(String::new());

    // Summary by reviewer
    lines.push("## Summary by Reviewer".to_string());
    let mut role_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for f in findings {
        let entry = role_counts.entry(f.role.clone()).or_default();
        *entry.entry(f.severity.clone()).or_insert(0) += 1;
    }

    let mut roles: Vec<_> = role_counts.keys().collect();
    roles.sort();
    for role in roles {
        lines.push(format!("\n### {} Reviewer", role));
        let counts = &role_counts[role];
        for sev in &["CRITICAL", "IMPORTANT", "MINOR"] {
            if let Some(&count) = counts.get(*sev) {
                if count > 0 {
                    lines.push(format!("- {}: {}", sev, count));
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(String::new());

    // Do Now section
    let do_now: Vec<_> = prioritized
        .iter()
        .filter(|f| f.classification == Classification::DoNow)
        .collect();
    if !do_now.is_empty() {
        lines.push("## 🔴 Do Now (Quick Wins)".to_string());
        lines.push(String::new());
        for (i, finding) in do_now.iter().enumerate() {
            plan::render_item(&mut lines, i + 1, "Fix", &finding_to_item(finding), true);
        }
    }

    // Carried-over items from a previous plan (optional)
    let carried = carry_over_from.and_then(|p| {
        let items = read_carry_over(p, &now.to_string());
        (!items.is_empty()).then_some(items)
    });

    // Defer section
    let defer: Vec<_> = prioritized
        .iter()
        .filter(|f| f.classification == Classification::Defer)
        .collect();
    let has_carried = carried.as_ref().is_some_and(|c| !c.is_empty());
    if !defer.is_empty() || has_carried {
        lines.push("## 🟡 Defer to Next Phase".to_string());
        lines.push(String::new());

        // Carried items go first, under a small provenance header.
        if let Some(items) = &carried {
            if !items.is_empty() {
                let latest = items
                    .iter()
                    .filter_map(|i| i.carried_from.clone())
                    .max()
                    .unwrap_or_default();
                auto_dev_pipeline::log::log(&format!(
                    "Carrying over {} deferred item(s) from previous plan {}",
                    items.len(),
                    latest
                ));
                lines.push(format!(
                    "> Carried over from {} ({} items)",
                    latest,
                    items.len()
                ));
                lines.push(String::new());
                for (i, item) in items.iter().enumerate() {
                    render_carried_item(&mut lines, i + 1, item);
                }
            }
        }

        for (i, finding) in defer.iter().enumerate() {
            plan::render_item(
                &mut lines,
                i + 1,
                "Deferred",
                &finding_to_item(finding),
                false,
            );
        }
    }

    // Write output
    fs::write(output_path, lines.join("\n"))?;

    // JSON sidecar (Task 2): machine-readable mirror of the plan next to the
    // markdown. Best-effort for consumers; a serialization failure must not
    // invalidate the human-readable plan, but a write failure is fatal (the
    // sidecar is part of the published artifact set).
    //
    // SINGLE-WRITER INVARIANT: this function is the only code that writes
    // `<ts>-plan.md` and `<ts>-plan.json`, always together, with items in the
    // order [Do Now..., carried..., fresh Defer...]. Each item carries an
    // explicit `do_now` flag because position alone cannot recover the Do Now
    // slice — when no items are carried, fresh Defer items are also
    // non-carried and indistinguishable by ordering. Consumers (execute phase
    // divergence check) must select items by `do_now == true`, never by
    // "all non-carried items".
    let plan_doc = auto_dev_pipeline::plan::Plan {
        generated: now.to_string(),
        items: {
            let mut items: Vec<PlanItem> = do_now.iter().map(|f| finding_to_item(f)).collect();
            if let Some(carried_items) = &carried {
                items.extend(carried_items.iter().cloned());
            }
            items.extend(defer.iter().map(|f| finding_to_item(f)));
            items
        },
    };
    let sidecar_path = output_path.with_extension("json");
    fs::write(
        &sidecar_path,
        serde_json::to_string_pretty(&plan_doc).context("serialize plan JSON sidecar")?,
    )?;
    auto_dev_pipeline::log::log(&format!("Plan sidecar written: {}", sidecar_path.display()));
    Ok(())
}

/// Render one carried-over deferred item inside the Defer section.
fn render_carried_item(lines: &mut Vec<String>, index: usize, item: &PlanItem) {
    plan::render_item(lines, index, "Carried", item, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_dev_pipeline::plan::WONTFIX_ATTEMPT_THRESHOLD;

    const SAMPLE_PLAN: &str = r#"# Auto-Dev Fix Plan

> Generated: 2026-08-20T10:00:00

## Summary by Reviewer

---

## 🔴 Do Now (Quick Wins)

### Fix 1: Fix something

**Source:** Code Reviewer
**Severity:** CRITICAL

## 🟡 Defer to Next Phase

### Deferred 1: Refactor module layout

**Source:** Architecture Reviewer
**Severity:** MINOR
**File:** `src/lib.rs`
**Line:** 42

**Description:**
The module layout needs a cross-module redesign.

### Deferred 2: Add metrics endpoint

**Source:** Devops Reviewer
**Severity:** MINOR

**Description:**
Expose pipeline metrics.

"#;

    #[test]
    fn extracts_defer_items_from_sample_plan() {
        let items = parse_carry_over(SAMPLE_PLAN, "2026-08-24T00:00:00");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Refactor module layout");
        assert_eq!(items[0].role, "Architecture");
        assert_eq!(items[0].severity, "MINOR");
        assert_eq!(items[0].file.as_deref(), Some("src/lib.rs"));
        assert_eq!(items[0].line, Some(42));
        assert_eq!(
            items[0].description,
            "The module layout needs a cross-module redesign."
        );
        // Fresh items get the previous plan's timestamp and attempt 1.
        assert_eq!(
            items[0].carried_from.as_deref(),
            Some("2026-08-24T00:00:00")
        );
        assert_eq!(items[0].attempt, 1);
    }

    #[test]
    fn no_defer_section_yields_no_items() {
        assert!(parse_carry_over("# Plan\n\n## Summary\n\ntext", "t").is_empty());
    }

    #[test]
    fn attempt_increments_on_recarry() {
        let plan_with_marker = "## 🟡 Defer to Next Phase\n\n### Carried 1: Old issue\n\n**Source:** Code Reviewer\n**Severity:** MINOR\n\n**Description:**\nSomething deferred.\n\n**Carried over:** from 2026-01-01T09:00:00, attempt 2\n";
        let items = parse_carry_over(plan_with_marker, "2026-08-24T00:00:00");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempt, 3);
        assert_eq!(
            items[0].carried_from.as_deref(),
            Some("2026-01-01T09:00:00")
        );
    }

    #[test]
    fn wontfix_marked_at_threshold() {
        let mut item = PlanItem::new("Old");
        item.role = "Code".into();
        item.severity = "MINOR".into();
        item.description = "d".into();
        item.carried_from = Some("2026-01-01T09:00:00".into());
        item.attempt = WONTFIX_ATTEMPT_THRESHOLD;
        let mut lines = Vec::new();
        render_carried_item(&mut lines, 1, &item);
        let text = lines.join("\n");
        assert!(text.contains("WONTFIX candidate"));
        assert!(text.contains("attempt 3"));

        let mut fresh = item.clone();
        fresh.attempt = 1;
        let mut lines = Vec::new();
        render_carried_item(&mut lines, 1, &fresh);
        assert!(!lines.join("\n").contains("WONTFIX candidate"));
    }

    #[test]
    fn roundtrip_preserves_title_and_severity() {
        let first = parse_carry_over(SAMPLE_PLAN, "2026-08-24T00:00:00");
        // Render items back into a plan-shaped string and re-parse.
        let mut rendered = String::from("## 🟡 Defer to Next Phase\n\n");
        for (i, item) in first.iter().enumerate() {
            let mut lines = Vec::new();
            render_carried_item(&mut lines, i + 1, item);
            rendered.push_str(&lines.join("\n"));
            rendered.push('\n');
        }
        let second = parse_carry_over(&rendered, "2026-08-25T00:00:00");
        assert_eq!(second.len(), 2);
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.title, b.title);
            assert_eq!(a.severity, b.severity);
            assert_eq!(b.attempt, a.attempt + 1);
        }
    }
}
