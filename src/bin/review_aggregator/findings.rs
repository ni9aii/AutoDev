//! Finding data model, classification, deduplication and prioritization.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Triage decision for a finding (Task 5: typed, was a free-form String).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Classification {
    #[serde(rename = "do_now")]
    DoNow,
    #[serde(rename = "defer")]
    Defer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Finding {
    pub(crate) role: String,
    pub(crate) severity: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) file: Option<String>,
    pub(crate) line: Option<usize>,
    pub(crate) classification: Classification,
}

pub(crate) fn classify_finding(
    severity: &str,
    file: &Option<String>,
    body: &str,
) -> Classification {
    use auto_dev_pipeline::severity::Severity;
    let is_critical = matches!(
        severity.parse::<Severity>(),
        Ok(Severity::Critical) | Ok(Severity::Important)
    );
    let has_file = file.is_some();
    let is_simple = !body.contains("refactor")
        && !body.contains("architecture")
        && !body.contains("cross-module")
        && !body.contains("redesign");

    if is_critical && has_file && is_simple {
        Classification::DoNow
    } else {
        Classification::Defer
    }
}

/// Normalized key used to detect the same finding reported by multiple reviewers.
fn dedup_key(finding: &Finding) -> String {
    format!(
        "{}|{}|{}",
        finding.severity.trim().to_lowercase(),
        finding.title.trim().to_lowercase(),
        finding.file.as_deref().unwrap_or("").trim().to_lowercase()
    )
}

/// Removes findings that share a severity+title+file key, keeping the first occurrence.
pub(crate) fn dedup_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut seen = HashSet::new();
    findings
        .into_iter()
        .filter(|f| seen.insert(dedup_key(f)))
        .collect()
}

pub(crate) fn prioritize_findings(findings: &[Finding]) -> Vec<Finding> {
    use auto_dev_pipeline::severity::Severity;
    // Rank by typed severity (Critical=0, Important=1, Minor=2); unknown sorts last.
    let severity_rank = |s: &str| s.parse::<Severity>().map(|sv| sv as u8).unwrap_or(u8::MAX);

    let mut sorted = findings.to_vec();
    sorted.sort_by_key(|f| (severity_rank(&f.severity), f.role.clone(), f.title.clone()));
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(role: &str, severity: &str, title: &str, file: Option<&str>) -> Finding {
        Finding {
            role: role.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            description: String::new(),
            file: file.map(|f| f.to_string()),
            line: None,
            classification: Classification::DoNow,
        }
    }

    #[test]
    fn test_dedup_removes_same_finding_from_different_reviewers() {
        let findings = vec![
            finding("code", "CRITICAL", "SQL injection", Some("src/db.rs")),
            finding("security", "CRITICAL", "SQL injection", Some("src/db.rs")),
        ];
        let deduped = dedup_findings(findings);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].role, "code");
    }

    #[test]
    fn test_dedup_keeps_distinct_findings() {
        let findings = vec![
            finding("code", "CRITICAL", "SQL injection", Some("src/db.rs")),
            finding(
                "security",
                "IMPORTANT",
                "Missing auth check",
                Some("src/auth.rs"),
            ),
        ];
        let deduped = dedup_findings(findings);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_dedup_key_case_and_whitespace_insensitive() {
        let findings = vec![
            finding("code", "CRITICAL", "  SQL Injection ", Some("src/db.rs")),
            finding("security", "critical", "sql injection", Some("SRC/DB.RS")),
        ];
        let deduped = dedup_findings(findings);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_prioritize_orders_by_typed_severity() {
        let findings = vec![
            finding("code", "MINOR", "m", None),
            finding("code", "CRITICAL", "c", None),
            finding("code", "IMPORTANT", "i", None),
        ];
        let out = prioritize_findings(&findings);
        let sevs: Vec<_> = out.iter().map(|f| f.severity.clone()).collect();
        assert_eq!(sevs, vec!["CRITICAL", "IMPORTANT", "MINOR"]);
    }
}
