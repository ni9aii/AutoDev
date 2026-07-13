use anyhow::{Context, Result};
use clap::Parser;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

/// Review Aggregator for Auto-Dev Pipeline
/// Aggregates findings from reviewers and generates prioritized fix plan
#[derive(Parser, Debug)]
#[command(name = "review-aggregator", version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Directory with review reports (optional if --dev-notes is set)
    #[arg(long, required = false)]
    input_dir: Option<PathBuf>,

    /// Output plan file path (optional if --dev-notes is set)
    #[arg(long, required = false)]
    output: Option<PathBuf>,

    /// Project name (used for dev-notes path construction)
    #[arg(long)]
    project: Option<String>,

    /// Auto-construct dev-notes paths: read from <root>/<project>/reviews/<timestamp>/
    /// and write to <root>/<project>/plans/<timestamp>-plan.md
    #[arg(long, default_value = "false")]
    dev_notes: bool,

    /// Root directory for dev-notes (overrides $DEV_NOTES_ROOT and ~/obsidian-vault/dev-notes default)
    #[arg(long)]
    dev_notes_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Finding {
    role: String,
    severity: String,
    title: String,
    description: String,
    file: Option<String>,
    line: Option<usize>,
    classification: String, // "do_now" or "defer"
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

fn parse_review_file(filepath: &Path) -> Result<Vec<Finding>> {
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

fn classify_finding(severity: &str, file: &Option<String>, body: &str) -> String {
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
        "do_now".to_string()
    } else {
        "defer".to_string()
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
fn dedup_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut seen = std::collections::HashSet::new();
    findings
        .into_iter()
        .filter(|f| seen.insert(dedup_key(f)))
        .collect()
}

fn prioritize_findings(findings: &[Finding]) -> Vec<Finding> {
    use auto_dev_pipeline::severity::Severity;
    // Rank by typed severity (Critical=0, Important=1, Minor=2); unknown sorts last.
    let severity_rank = |s: &str| s.parse::<Severity>().map(|sv| sv as u8).unwrap_or(u8::MAX);

    let mut sorted = findings.to_vec();
    sorted.sort_by_key(|f| (severity_rank(&f.severity), f.role.clone(), f.title.clone()));
    sorted
}

fn generate_plan(findings: &[Finding], output_path: &Path) -> Result<()> {
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

fn main() -> Result<()> {
    let args = Args::parse();

    // Resolve dev-notes paths if --dev-notes flag is set
    let (input_dir, output_path) = if args.dev_notes {
        let project = args
            .project
            .as_ref()
            .context("--project is required when --dev-notes is enabled")?;
        let root =
            auto_dev_pipeline::git::paths::resolve_dev_notes_root(args.dev_notes_root.as_ref())?;
        let reviews_dir = {
            auto_dev_pipeline::validation::validate_project_name(project)
                .map_err(|e| anyhow::anyhow!(e))?;
            root.join(project).join("reviews")
        };

        // Find the most recent timestamp directory
        let latest_dir = fs::read_dir(&reviews_dir)
            .with_context(|| format!("Failed to read reviews dir: {}", reviews_dir.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .max();

        let (input_dir, timestamp) = match latest_dir {
            Some(dir) => (
                dir.clone(),
                dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            ),
            None => {
                eprintln!(
                    "[auto-dev] WARNING: No review directories found in {} — generating empty plan",
                    reviews_dir.display()
                );
                (reviews_dir.clone(), "empty".to_string())
            }
        };
        let plans_dir = root.join(project).join("plans");
        fs::create_dir_all(&plans_dir)?;
        let output_path = plans_dir.join(format!("{}-plan.md", timestamp));

        println!("[auto-dev] dev-notes mode enabled");
        println!("[auto-dev] Input:  {}", input_dir.display());
        println!("[auto-dev] Output: {}", output_path.display());

        (input_dir, output_path)
    } else {
        let input_dir = args
            .input_dir
            .clone()
            .context("--input-dir is required when --dev-notes is not set")?;
        let output_path = args
            .output
            .clone()
            .context("--output is required when --dev-notes is not set")?;
        (input_dir, output_path)
    };

    if !input_dir.exists() {
        anyhow::bail!("Input directory not found: {}", input_dir.display());
    }

    // Parse all review files
    let mut all_findings = Vec::new();
    for entry in WalkDir::new(&input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let findings = parse_review_file(entry.path())?;
        eprintln!(
            "Parsed {} findings from {}",
            findings.len(),
            entry.path().display()
        );
        all_findings.extend(findings);
    }

    let before_dedup = all_findings.len();
    all_findings = dedup_findings(all_findings);
    let deduped = before_dedup - all_findings.len();
    if deduped > 0 {
        eprintln!("Removed {} duplicate finding(s)", deduped);
    }

    if all_findings.is_empty() {
        eprintln!("No findings found. Generating empty plan.");
    }

    // Generate plan
    generate_plan(&all_findings, &output_path)?;
    println!("Plan generated: {}", output_path.display());
    println!("Total findings: {}", all_findings.len());

    // Summary
    let mut severity_counts: HashMap<String, usize> = HashMap::new();
    for f in &all_findings {
        *severity_counts.entry(f.severity.clone()).or_insert(0) += 1;
    }

    println!("\nSeverity breakdown:");
    for sev in &["CRITICAL", "IMPORTANT", "MINOR"] {
        println!("  {}: {}", sev, severity_counts.get(*sev).unwrap_or(&0));
    }

    Ok(())
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
            classification: "do_now".to_string(),
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
