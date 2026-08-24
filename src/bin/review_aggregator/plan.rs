//! Generation of the prioritized fix plan markdown file.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::findings::{prioritize_findings, Finding};

/// One deferred item extracted from a PREVIOUS plan for carry-over.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CarriedDefer {
    pub(crate) title: String,
    pub(crate) role: String,
    pub(crate) severity: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) description: String,
    /// Timestamp of the plan the item was last carried from.
    pub(crate) origin_ts: String,
    /// How many times this item has been carried over (1 on first carry).
    pub(crate) attempt: u32,
}

/// Marker line rendered under each carried item; parsed back on the next run.
const CARRIED_MARKER_PREFIX: &str = "**Carried over:** from ";
/// Items carried this many times or more are flagged for human decision.
const WONTFIX_ATTEMPT_THRESHOLD: u32 = 3;

/// Parse the "## 🟡 Defer to Next Phase" section of a previous plan into
/// [`CarriedDefer`] items. Items that already carry a
/// `**Carried over:** from <ts>, attempt N` marker get attempt = N + 1 and keep
/// their original origin timestamp; fresh items get attempt = 1 with
/// `now_ts` as their origin. Returns an empty vec when there is no defer
/// section.
pub(crate) fn parse_carry_over(prev_plan: &str, now_ts: &str) -> Vec<CarriedDefer> {
    let mut items = Vec::new();
    let Some(section) = extract_defer_section(prev_plan) else {
        return items;
    };

    let marker_re = regex::Regex::new(r"\*\*Carried over:\*\* from (.+), attempt (\d+)").ok();

    for block in split_defer_blocks(section) {
        let mut title = String::new();
        let mut role = "Unknown".to_string();
        let mut severity = "UNKNOWN".to_string();
        let mut file = None;
        let mut line_no = None;
        let mut description = String::new();
        let mut in_description = false;
        let mut origin_ts = now_ts.to_string();
        let mut attempt: u32 = 1;

        for raw in block.lines() {
            let line = raw.trim_end();
            if let Some(rest) = line.strip_prefix("### ") {
                // "Deferred N: <title>" or "Carried N: <title>"
                if let Some((_, t)) = rest.split_once(": ") {
                    title = t.trim().to_string();
                }
                in_description = false;
            } else if let Some(rest) = line.strip_prefix("**Source:** ") {
                role = rest
                    .strip_suffix(" Reviewer")
                    .unwrap_or(rest)
                    .trim()
                    .to_string();
                in_description = false;
            } else if let Some(rest) = line.strip_prefix("**Severity:** ") {
                severity = rest.trim().to_string();
                in_description = false;
            } else if let Some(rest) = line.strip_prefix("**File:** ") {
                file = Some(rest.trim_matches('`').to_string());
                in_description = false;
            } else if let Some(rest) = line.strip_prefix("**Line:** ") {
                line_no = rest.trim().parse::<usize>().ok();
                in_description = false;
            } else if line == "**Description:**" || line.starts_with("**Description:**") {
                description = line
                    .strip_prefix("**Description:**")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                in_description = true;
            } else if line.starts_with(CARRIED_MARKER_PREFIX) {
                if let Some(caps) = marker_re.as_ref().and_then(|re| re.captures(line)) {
                    origin_ts = caps[1].to_string();
                    attempt = caps[2].parse::<u32>().unwrap_or(1).saturating_add(1);
                }
                in_description = false;
            } else if in_description && !line.is_empty() && !line.starts_with('#') {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(line);
            }
        }

        if title.is_empty() {
            continue;
        }
        items.push(CarriedDefer {
            title,
            role,
            severity,
            file,
            line: line_no,
            description,
            origin_ts,
            attempt,
        });
    }
    items
}

/// Return the text of the defer section (header excluded, next `## ` header or
/// EOF terminating it), or None when absent.
fn extract_defer_section(plan: &str) -> Option<&str> {
    let start = plan.find("## 🟡 Defer to Next Phase")?;
    let body = &plan[start..];
    let after_header = match body.find('\n') {
        Some(idx) => &body[idx + 1..],
        None => "",
    };
    let end = after_header
        .find("\n## ")
        .map(|idx| idx + 1)
        .unwrap_or(after_header.len());
    Some(&after_header[..end])
}

/// Split a defer section into per-item blocks at `### ` headings.
fn split_defer_blocks(section: &str) -> Vec<&str> {
    let mut starts: Vec<usize> = Vec::new();
    let mut offset = 0usize;
    for l in section.split_inclusive('\n') {
        if l.starts_with("### ") {
            starts.push(offset);
        }
        offset += l.len();
    }
    if starts.is_empty() {
        return Vec::new();
    }
    starts.push(section.len());
    starts
        .windows(2)
        .map(|w| section[w[0]..w[1]].trim_end())
        .collect()
}

/// Extract the previous plan's own generation timestamp from its
/// `> Generated: <ts>` line, if present.
fn extract_plan_timestamp(plan: &str) -> Option<String> {
    plan.lines().find_map(|l| {
        l.trim()
            .strip_prefix("> Generated: ")
            .map(|ts| ts.trim().to_string())
    })
}

/// Read and parse the previous plan at `path` for carry-over. Any problem
/// (missing file, unreadable, no defer section) is logged as a warning and
/// yields no carried items — never a hard failure.
pub(crate) fn read_carry_over(path: &Path, now_ts: &str) -> Vec<CarriedDefer> {
    match fs::read_to_string(path) {
        Ok(content) => {
            // Fresh items are stamped with the PREVIOUS plan's generation
            // timestamp so provenance points at the run that deferred them.
            let default_ts = extract_plan_timestamp(&content).unwrap_or_else(|| now_ts.to_string());
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
        .filter(|f| f.classification == "do_now")
        .collect();
    if !do_now.is_empty() {
        lines.push("## 🔴 Do Now (Quick Wins)".to_string());
        lines.push(String::new());
        for (i, finding) in do_now.iter().enumerate() {
            lines.push(format!("### Fix {}: {}", i + 1, finding.title));
            lines.push(format!("\n**Source:** {} Reviewer", finding.role));
            lines.push(format!("**Severity:** {}", finding.severity));
            if let Some(ref file) = finding.file {
                lines.push(format!("**File:** `{}`", file));
            }
            if let Some(line) = finding.line {
                lines.push(format!("**Line:** {}", line));
            }
            lines.push("\n**Description:**".to_string());
            lines.push(finding.description.clone());
            lines.push(String::new());
            lines.push("**Action:** _To be filled by implementer_".to_string());
            lines.push(String::new());
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
        .filter(|f| f.classification == "defer")
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
                    .map(|i| i.origin_ts.clone())
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
            lines.push(format!("### Deferred {}: {}", i + 1, finding.title));
            lines.push(format!("\n**Source:** {} Reviewer", finding.role));
            lines.push(format!("**Severity:** {}", finding.severity));
            if let Some(ref file) = finding.file {
                lines.push(format!("**File:** `{}`", file));
            }
            lines.push("\n**Description:**".to_string());
            lines.push(finding.description.clone());
            lines.push(String::new());
        }
    }

    // Write output
    fs::write(output_path, lines.join("\n"))?;
    Ok(())
}

/// Render one carried-over deferred item inside the Defer section.
fn render_carried_item(lines: &mut Vec<String>, index: usize, item: &CarriedDefer) {
    let mut title = format!("### Carried {}: {}", index, item.title);
    if item.attempt >= WONTFIX_ATTEMPT_THRESHOLD {
        title.push_str(" ⚠️ WONTFIX candidate — requires human decision");
    }
    lines.push(title);
    lines.push(format!("\n**Source:** {} Reviewer", item.role));
    lines.push(format!("**Severity:** {}", item.severity));
    if let Some(ref file) = item.file {
        lines.push(format!("**File:** `{}`", file));
    }
    if let Some(line) = item.line {
        lines.push(format!("**Line:** {}", line));
    }
    lines.push("\n**Description:**".to_string());
    lines.push(item.description.clone());
    lines.push(String::new());
    lines.push(format!(
        "{}{}, attempt {}",
        CARRIED_MARKER_PREFIX, item.origin_ts, item.attempt
    ));
    lines.push(String::new());
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Fresh items get the current run timestamp and attempt 1.
        assert_eq!(items[0].origin_ts, "2026-08-24T00:00:00");
        assert_eq!(items[0].attempt, 1);
    }

    #[test]
    fn no_defer_section_yields_no_items() {
        assert!(parse_carry_over("# Plan\n\n## Summary\n\ntext", "t").is_empty());
    }

    #[test]
    fn attempt_increments_on_recarry() {
        let plan_with_marker = "## 🟡 Defer to Next Phase\n\n### Deferred 1: Old issue\n\n**Source:** Code Reviewer\n**Severity:** MINOR\n\n**Description:**\nSomething deferred.\n\n**Carried over:** from 2026-01-01T09:00:00, attempt 2\n";
        let items = parse_carry_over(plan_with_marker, "2026-08-24T00:00:00");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempt, 3);
        assert_eq!(items[0].origin_ts, "2026-01-01T09:00:00");
    }

    #[test]
    fn wontfix_marked_at_threshold() {
        let item = CarriedDefer {
            title: "Old".into(),
            role: "Code".into(),
            severity: "MINOR".into(),
            file: None,
            line: None,
            description: "d".into(),
            origin_ts: "2026-01-01T09:00:00".into(),
            attempt: 3,
        };
        let mut lines = Vec::new();
        render_carried_item(&mut lines, 1, &item);
        let text = lines.join("\n");
        assert!(text.contains("WONTFIX candidate"));
        assert!(text.contains("attempt 3"));

        let fresh = CarriedDefer { attempt: 1, ..item };
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
