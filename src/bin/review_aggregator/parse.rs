//! Parsing of review report markdown files into structured findings.

use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::Path;

use super::findings::{classify_finding, Finding};

// Pre-compiled regex patterns (compiled once, used many times)
static HEADER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^###\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+?)$")
        .expect("Invalid HEADER_RE pattern")
});

static TABLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)\|\s*(CRITICAL|IMPORTANT|MINOR)\s*\|\s*([^|]+?)\s*\|")
        .expect("Invalid TABLE_RE pattern")
});

static BULLET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^\s*[-*]\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+)$")
        .expect("Invalid BULLET_RE pattern")
});

static FILE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)[Ff]ile:\s*`?([^`\n]+)`?").expect("Invalid FILE_RE pattern"));

static LINE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)[Ll]ine:\s*(\d+)").expect("Invalid LINE_RE pattern"));

/// Matches `File:` / `Line:` / `Source:` lead-in lines. These carry a single
/// fact that the parser already extracted into structured fields, so the whole
/// line is dropped from the description body.
static META_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^\s*(File|Line|Source)\s*:\s*\S.*$").expect("Invalid META_RE pattern")
});

/// Matches the `Description:` lead-in. The description is multi-line, so only
/// the prefix is stripped and the following text is kept.
static DESC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?im)^\s*Description\s*:\s*").expect("Invalid DESC_RE pattern"));

/// Strips parser-metadata from a finding body so the generated plan does not
/// duplicate it. `File:`/`Line:`/`Source:` lines are dropped entirely (their
/// value is already in structured fields); `Description:` keeps its text with
/// only the prefix removed.
fn clean_body(body: &str) -> String {
    body.lines()
        .map(|l| {
            DESC_RE
                .replace(META_RE.replace(l, "").as_ref(), "")
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Self-correction markers that indicate a finding should be skipped
const SKIP_MARKERS: &[&str] = &[
    "removing this entry",
    "downgrading",
    "false alarm",
    "not present",
    "not a bug",
    "not an issue",
    "no critical here",
    "no issue here",
    "re-checking",
];

pub(crate) fn parse_review_file(filepath: &Path) -> Result<Vec<Finding>> {
    let content = fs::read_to_string(filepath)?;
    let mut findings = Vec::new();

    // Extract reviewer role from filename (e.g., "code-review.md" -> "code")
    let role = filepath
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end_matches("-review"))
        .unwrap_or("unknown")
        .to_string();

    // Parse headers with body manually (Rust regex doesn't support look-ahead)
    let mut matches: Vec<(String, String, String)> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    let mut in_code = false;
    while i < lines.len() {
        let line = lines[i];
        // Toggle fenced code-block state on ``` lines (with optional language).
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        }
        // Headers inside code blocks are prose, not findings.
        if !in_code {
            if let Some(cap) = HEADER_RE.captures(line) {
                let severity = cap[1].to_uppercase();
                let title = cap[2].trim().to_string();
                // Collect body until next real heading, respecting code fences.
                let mut body_lines = Vec::new();
                i += 1;
                let mut body_in_code = false;
                while i < lines.len() {
                    let next = lines[i];
                    if next.trim_start().starts_with("```") {
                        body_in_code = !body_in_code;
                        body_lines.push(next);
                        i += 1;
                        continue;
                    }
                    // A markdown heading only ends the body outside code blocks.
                    if !body_in_code
                        && (next.starts_with("### ")
                            || next.starts_with("## ")
                            || next.starts_with("# "))
                    {
                        break;
                    }
                    body_lines.push(next);
                    i += 1;
                }
                let body = body_lines.join("\n").trim().to_string();
                matches.push((severity, title, body));
                continue;
            }
        }
        i += 1;
    }

    for cap in TABLE_RE.captures_iter(&content) {
        let severity = cap[1].to_uppercase();
        let title = cap[2].trim().to_string();
        // Skip summary-count rows like `| CRITICAL | 1 |` where the "title"
        // cell is just a number — these are severity tallies, not findings.
        if title.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        matches.push((severity, title, String::new()));
    }

    for cap in BULLET_RE.captures_iter(&content) {
        let severity = cap[1].to_uppercase();
        let title = cap[2].trim().to_string();
        matches.push((severity, title, String::new()));
    }

    for (severity, title, body) in matches {
        // Skip self-corrected / false-alarm findings
        let body_lower = body.to_lowercase();
        if SKIP_MARKERS.iter().any(|m| body_lower.contains(m)) {
            continue;
        }

        // Extract file path
        let file = FILE_RE.captures(&body).map(|cap| cap[1].trim().to_string());

        // Extract line number
        let line = LINE_RE
            .captures(&body)
            .and_then(|cap| cap[1].parse::<usize>().ok());

        let classification = classify_finding(&severity, &file, &body);

        findings.push(Finding {
            role: role.clone(),
            severity,
            title,
            description: clean_body(&body),
            file,
            line,
            classification,
        });
    }

    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_review_ignores_headers_inside_code_blocks() {
        // Regression for Fix 14: a `### [SEVERITY]` heading (or any `#` line)
        // inside a fenced code block is prose, not a finding. Otherwise review
        // reports that show example findings in ``` blocks spawn phantom
        // findings, and `#`-lines inside code falsely truncate a finding body.
        let dir = std::env::temp_dir().join(format!("autodev-parse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("code-review.md");
        std::fs::write(
            &p,
            "## 🔴 Do Now (Quick Wins)\n\n\
### [IMPORTANT] Real finding\n\
Body line one\n\
```\n\
### [CRITICAL] Phantom inside code\n\
## not a heading\n\
```\n\
trailing body after code block\n",
        )
        .unwrap();

        let findings = parse_review_file(&p).unwrap();
        // Only the one real finding; the header inside ``` must be ignored.
        assert_eq!(findings.len(), 1, "code-block header leaked as finding");
        assert_eq!(findings[0].title, "Real finding");
        // The `#`-line inside the fence must NOT have truncated the body.
        assert!(
            findings[0]
                .description
                .contains("trailing body after code block"),
            "body truncated at in-code '#': {}",
            findings[0].description
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_review_ignores_summary_count_table() {
        // Regression: a severity-tally table in the report header
        //   | CRITICAL | 1 |
        //   | IMPORTANT | 7 |
        // must NOT be parsed as findings (title cell is a bare number).
        let dir = std::env::temp_dir().join(format!("autodev-tbl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("architecture-review.md");
        std::fs::write(
            &p,
            "## Summary\n\
| Severity | Count |\n\
| CRITICAL | 1 |\n\
| IMPORTANT | 7 |\n\
| MINOR | 6 |\n\n\
### [CRITICAL] Real architecture finding\n\
Some detail.\n",
        )
        .unwrap();

        let findings = parse_review_file(&p).unwrap();
        assert_eq!(
            findings.len(),
            1,
            "summary tally rows leaked as findings: {:?}",
            findings.iter().map(|f| f.title.clone()).collect::<Vec<_>>()
        );
        assert_eq!(findings[0].title, "Real architecture finding");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_review_extracts_file_line_and_role() {
        let dir = std::env::temp_dir().join(format!("autodev-b2a-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("security-review.md");
        std::fs::write(
            &p,
            "### [CRITICAL] SQL injection\nBad query.\nFile: `src/db.rs`\nLine: 42\n",
        )
        .unwrap();
        let f = parse_review_file(&p).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].role, "security");
        assert_eq!(f[0].severity, "CRITICAL");
        assert_eq!(f[0].file.as_deref(), Some("src/db.rs"));
        assert_eq!(f[0].line, Some(42));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_parse_review_skips_self_corrected_findings() {
        let dir = std::env::temp_dir().join(format!("autodev-b2b-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("code-review.md");
        std::fs::write(
            &p,
            "### [CRITICAL] Off by one\nOn re-checking this is a false alarm, not a bug.\n",
        )
        .unwrap();
        let f = parse_review_file(&p).unwrap();
        assert_eq!(f.len(), 0, "self-corrected finding should be skipped");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
