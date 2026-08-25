//! Unified typed model of fix-plan items, with ONE renderer and ONE parser.
//!
//! The plan markdown is produced by review-aggregator and consumed by two
//! independent phases (execute parses "Do Now", carry-over parses "Defer").
//! Historically each consumer hand-rolled its own string-matching parser;
//! this module replaces all of them with a single producer/consumer pair so
//! a render-format change cannot silently break downstream parsing.
//!
//! Roundtrip invariant: `render_item` output fed back through `parse_items`
//! preserves every field (title, role, severity, file, line, description,
//! carry provenance) — see the roundtrip tests below.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Marker rendered under each carried item; parsed back on the next run.
const CARRIED_MARKER_PREFIX: &str = "**Carried over:** from ";

static CARRIED_MARKER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\*\*Carried over:\*\* from (.+), attempt (\d+)").expect("valid marker regex")
});

/// Suffix the renderer appends to carried titles past the WONTFIX threshold.
/// Stripped during parsing so it does not leak into the title.
const WONTFIX_SUFFIX: &str = " ⚠️ WONTFIX candidate — requires human decision";

/// One actionable item of a fix plan (a "Fix N", "Deferred N" or "Carried N"
/// entry). Carry provenance is folded into the same type: fresh items have
/// `carried_from == None` / `attempt == 0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanItem {
    pub role: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub file: Option<String>,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub carried_from: Option<String>,
    #[serde(default)]
    pub attempt: u32,
}

/// Machine-readable plan document — the JSON sidecar written next to
/// `<ts>-plan.md`. Consumers (execute, carry-over) prefer it over parsing the
/// markdown; the markdown remains the human artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Plan generation timestamp (`> Generated:` / filename `<ts>`).
    pub generated: String,
    pub items: Vec<PlanItem>,
}

impl PlanItem {
    pub fn new(title: impl Into<String>) -> Self {
        PlanItem {
            role: "Unknown".to_string(),
            severity: "UNKNOWN".to_string(),
            title: title.into(),
            description: String::new(),
            file: None,
            line: None,
            carried_from: None,
            attempt: 0,
        }
    }

    pub fn is_carried(&self) -> bool {
        self.carried_from.is_some()
    }
}

/// Parse every `### ` item block out of a plan section (e.g. the extracted
/// "Do Now" or "Defer to Next Phase" section). Blocks are delimited by `### `
/// headings; within a block, `**Field:**` lines populate structured fields,
/// everything after `**Description:**` up to the next field/heading/marker is
/// the description. Returns an empty vec when there are no item blocks.
pub fn parse_items(section: &str) -> Vec<PlanItem> {
    let mut items = Vec::new();
    let mut current: Option<PlanItem> = None;

    for raw in section.lines() {
        let line = raw.trim_end();

        if let Some(rest) = line.strip_prefix("### ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(PlanItem::new(parse_title(rest)));
            continue;
        }

        let Some(item) = current.as_mut() else {
            continue;
        };

        if let Some(marker) = line.strip_prefix(CARRIED_MARKER_PREFIX) {
            if let Some(caps) = CARRIED_MARKER_RE.captures(line) {
                item.carried_from = Some(caps[1].to_string());
                item.attempt = caps[2].parse::<u32>().unwrap_or(1);
            } else {
                // Malformed marker: still record provenance text so it is not
                // swallowed into the description.
                item.carried_from = Some(marker.trim().to_string());
                item.attempt = 1;
            }
        } else if let Some(rest) = line.strip_prefix("**Source:** ") {
            item.role = rest
                .strip_suffix(" Reviewer")
                .unwrap_or(rest)
                .trim()
                .to_string();
        } else if let Some(rest) = line.strip_prefix("**Severity:** ") {
            item.severity = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("**File:** ") {
            item.file = Some(rest.trim_matches('`').trim().to_string());
        } else if let Some(rest) = line.strip_prefix("**Line:** ") {
            item.line = rest.trim().parse::<usize>().ok();
        } else if let Some(rest) = line.strip_prefix("**Description:**") {
            item.description = rest.trim().to_string();
        } else if line.starts_with("**") {
            // Any other bold field (e.g. the aggregator's **Action:** meta
            // line) is a field boundary; it carries no structured data here
            // and must not join the description.
        } else if !line.is_empty() && !line.starts_with('#') {
            // Bare prose lines belong to the description whether they follow
            // the **Description:** label directly or appear without one
            // (legacy parse_fixes behaviour).
            if !item.description.is_empty() {
                item.description.push(' ');
            }
            item.description.push_str(line.trim());
        }
    }

    if let Some(item) = current.take() {
        items.push(item);
    }
    items
}

/// Render one item as plan-markdown lines. `heading_kind` is the numbered
/// heading prefix ("Fix", "Deferred" or "Carried"). When `with_action` is
/// set (Do Now items), the aggregator's `_To be filled by implementer_`
/// action placeholder is emitted.
pub fn render_item(
    lines: &mut Vec<String>,
    index: usize,
    heading_kind: &str,
    item: &PlanItem,
    with_action: bool,
) {
    let mut title = format!("### {} {}: {}", heading_kind, index, item.title);
    if item.attempt >= wontfix_attempt_threshold() {
        title.push_str(WONTFIX_SUFFIX);
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
    if with_action {
        lines.push(String::new());
        lines.push("**Action:** _To be filled by implementer_".to_string());
    }
    if let Some(ref origin) = item.carried_from {
        lines.push(String::new());
        lines.push(format!(
            "{}{}, attempt {}",
            CARRIED_MARKER_PREFIX, origin, item.attempt
        ));
    }
    lines.push(String::new());
}

/// Items carried this many times or more are flagged for human decision
/// (WONTFIX candidates). Kept here next to the marker it decorates.
pub const WONTFIX_ATTEMPT_THRESHOLD: u32 = 3;

pub fn wontfix_attempt_threshold() -> u32 {
    WONTFIX_ATTEMPT_THRESHOLD
}

/// Extract the title from a heading body: strips an optional numeric prefix
/// (`Fix 3:` / `Deferred 12:` / `Carried 2:` — with or without the colon) and
/// the WONTFIX decoration suffix.
fn parse_title(rest: &str) -> String {
    let mut title = match rest.split_once(": ") {
        Some((_, t)) if !t.trim().is_empty() => t.trim().to_string(),
        _ => rest.trim().to_string(),
    };
    if let Some(stripped) = title.strip_suffix(WONTFIX_SUFFIX) {
        title = stripped.to_string();
    }
    title
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_do_now_style_fix_block() {
        let section = "### Fix 1: Improve error handling\n\
            \n**Source:** Code Reviewer\n\
            **Severity:** CRITICAL\n\
            **File:** `src/lib.rs`\n\
            **Line:** 42\n\
            \n**Description:**\nErrors are swallowed silently.\n";
        let items = parse_items(section);
        assert_eq!(items.len(), 1);
        let f = &items[0];
        assert_eq!(f.title, "Improve error handling");
        assert_eq!(f.role, "Code");
        assert_eq!(f.severity, "CRITICAL");
        assert_eq!(f.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(f.line, Some(42));
        assert_eq!(f.description, "Errors are swallowed silently.");
        assert!(!f.is_carried());
        assert_eq!(f.attempt, 0);
    }

    #[test]
    fn action_meta_line_excluded_from_description() {
        let section = "### Fix 1: Wire metrics\n\n**Source:** code Reviewer\n\
            **Severity:** IMPORTANT\n\n**Description:**\nAdd a counter.\n\n\
            **Action:** _To be filled by implementer_\n";
        let items = parse_items(section);
        let d = &items[0].description;
        assert_eq!(d, "Add a counter.");
        assert!(!d.contains("Action"));
    }

    #[test]
    fn parses_multiple_blocks_and_numbered_headings() {
        let section = "### Deferred 1: Refactor layout\n\n**Source:** Architecture Reviewer\n\
            **Severity:** MINOR\n\n**Description:**\nCross-module redesign needed.\n\n\
            ### Deferred 2: Add metrics endpoint\n\n**Source:** Devops Reviewer\n\
            **Severity:** MINOR\n\n**Description:**\nExpose pipeline metrics.\n";
        let items = parse_items(section);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Refactor layout");
        assert_eq!(items[0].role, "Architecture");
        assert_eq!(items[1].title, "Add metrics endpoint");
    }

    #[test]
    fn parses_carried_marker_provenance() {
        let section = "### Carried 1: Old issue\n\n**Source:** Code Reviewer\n\
            **Severity:** MINOR\n\n**Description:**\nSomething deferred.\n\n\
            **Carried over:** from 2026-01-01T09:00:00, attempt 2\n";
        let items = parse_items(section);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].carried_from.as_deref(),
            Some("2026-01-01T09:00:00")
        );
        assert_eq!(items[0].attempt, 2);
        assert!(items[0].is_carried());
        assert_eq!(items[0].description, "Something deferred.");
    }

    #[test]
    fn wontfix_suffix_stripped_from_title_on_parse() {
        let section = "### Carried 1: Old issue ⚠️ WONTFIX candidate — requires human decision\n\n\
            **Description:**\nstale\n\n**Carried over:** from 2026-01-01T09:00:00, attempt 4\n";
        let items = parse_items(section);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Old issue");
        assert_eq!(items[0].attempt, 4);
    }

    #[test]
    fn empty_section_yields_no_items() {
        assert!(parse_items("").is_empty());
        assert!(parse_items("## Summary\n\nplain prose\n").is_empty());
    }

    #[test]
    fn missing_fields_default_gracefully() {
        let section = "### Fix Title without colon or fields\nJust a description line.\n";
        let items = parse_items(section);
        assert_eq!(items.len(), 1);
        // No ": " separator → whole heading is the title; the bare prose line
        // still lands in the description (legacy parse_fixes behaviour).
        assert_eq!(items[0].title, "Fix Title without colon or fields");
        assert_eq!(items[0].description, "Just a description line.");
        assert_eq!(items[0].severity, "UNKNOWN");
        assert_eq!(items[0].file, None);
    }

    #[test]
    fn render_then_parse_roundtrips_do_now_item() {
        let mut item = PlanItem::new("Improve error handling");
        item.role = "Code".into();
        item.severity = "CRITICAL".into();
        item.file = Some("src/lib.rs".into());
        item.line = Some(42);
        item.description = "Errors are swallowed silently.".into();

        let mut lines = Vec::new();
        render_item(&mut lines, 1, "Fix", &item, true);
        let parsed = parse_items(&lines.join("\n"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0], item);
    }

    #[test]
    fn render_then_parse_roundtrips_carried_item_across_generations() {
        let mut item = PlanItem::new("Refactor module layout");
        item.role = "Architecture".into();
        item.severity = "MINOR".into();
        item.file = Some("src/lib.rs".into());
        item.line = Some(7);
        item.description = "The module layout needs a cross-module redesign.".into();
        item.carried_from = Some("2026-08-20T10:00:00".into());
        item.attempt = 1;

        // Generation 1: render as a carried item, re-parse.
        let mut lines = Vec::new();
        render_item(&mut lines, 1, "Carried", &item, false);
        let gen1 = parse_items(&lines.join("\n"));
        assert_eq!(gen1.len(), 1);
        assert_eq!(gen1[0], item);

        // Generation 2: bump attempt, re-render, re-parse — provenance holds.
        let mut next = gen1[0].clone();
        next.attempt += 1;
        let mut lines = Vec::new();
        render_item(&mut lines, 1, "Carried", &next, false);
        let gen2 = parse_items(&lines.join("\n"));
        assert_eq!(gen2.len(), 1);
        assert_eq!(gen2[0].attempt, 2);
        assert_eq!(gen2[0].carried_from, item.carried_from);
        assert_eq!(gen2[0].title, item.title);
        assert_eq!(gen2[0].description, item.description);
    }

    #[test]
    fn wontfix_candidate_rendered_at_threshold_only() {
        let mut stale = PlanItem::new("Stale finding");
        stale.attempt = wontfix_attempt_threshold();

        let mut lines = Vec::new();
        render_item(&mut lines, 1, "Carried", &stale, false);
        let text = lines.join("\n");
        assert!(text.contains(WONTFIX_SUFFIX));

        // And it survives re-parse with the clean title.
        let reparsed = parse_items(&text);
        assert_eq!(reparsed[0].title, "Stale finding");

        let mut fresh = PlanItem::new("Fresh finding");
        fresh.attempt = 1;
        let mut lines = Vec::new();
        render_item(&mut lines, 1, "Carried", &fresh, false);
        assert!(!lines.join("\n").contains("WONTFIX"));
    }
}
