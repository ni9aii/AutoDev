//! Generation of the prioritized fix plan markdown file.

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::findings::{prioritize_findings, Finding};

pub(crate) fn generate_plan(findings: &[Finding], output_path: &Path) -> Result<()> {
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

    // Defer section
    let defer: Vec<_> = prioritized
        .iter()
        .filter(|f| f.classification == "defer")
        .collect();
    if !defer.is_empty() {
        lines.push("## 🟡 Defer to Next Phase".to_string());
        lines.push(String::new());
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
