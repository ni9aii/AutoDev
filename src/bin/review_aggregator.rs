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

static FILE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[Ff]ile:\s*`?([^`\n]+)`?").expect("Invalid FILE_RE pattern")
});

static LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)[Ll]ine:\s*(\d+)").expect("Invalid LINE_RE pattern")
});

/// Review Aggregator for Auto-Dev Pipeline
/// Aggregates findings from reviewers and generates prioritized fix plan
#[derive(Parser, Debug)]
#[command(name = "review-aggregator", version = "1.1.0")]
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

    /// Auto-construct dev-notes paths: read from ~/dev-notes/<project>/reviews/<timestamp>/
    /// and write to ~/dev-notes/<project>/plans/<timestamp>-plan.md
    #[arg(long, default_value = "false")]
    dev_notes: bool,
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

    // Extract reviewer role from filename
    let role = filepath
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.split('-').nth_back(2))
        .unwrap_or("unknown")
        .to_string();

    // Parse headers with body manually (Rust regex doesn't support look-ahead)
    let mut matches: Vec<(String, String, String)> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(cap) = HEADER_RE.captures(line) {
            let severity = cap[1].to_uppercase();
            let title = cap[2].trim().to_string();
            // Collect body until next header or end
            let mut body_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                let next = lines[i];
                if next.starts_with("### ") || next.starts_with("## ") || next.starts_with("# ") {
                    break;
                }
                body_lines.push(next);
                i += 1;
            }
            let body = body_lines.join("\n").trim().to_string();
            matches.push((severity, title, body));
            continue;
        }
        i += 1;
    }

    for cap in TABLE_RE.captures_iter(&content) {
        let severity = cap[1].to_uppercase();
        let title = cap[2].trim().to_string();
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
        let file = FILE_RE
            .captures(&body)
            .map(|cap| cap[1].trim().to_string());

        // Extract line number
        let line = LINE_RE
            .captures(&body)
            .and_then(|cap| cap[1].parse::<usize>().ok());

        let classification = classify_finding(&severity, &file, &body);

        findings.push(Finding {
            role: role.clone(),
            severity,
            title,
            description: body,
            file,
            line,
            classification,
        });
    }

    Ok(findings)
}

fn classify_finding(severity: &str, file: &Option<String>, body: &str) -> String {
    let is_critical = severity == "CRITICAL" || severity == "IMPORTANT";
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

fn prioritize_findings(findings: &[Finding]) -> Vec<Finding> {
    let severity_order = |s: &str| match s {
        "CRITICAL" => 0,
        "IMPORTANT" => 1,
        "MINOR" => 2,
        _ => 3,
    };

    let mut sorted = findings.to_vec();
    sorted.sort_by_key(|f| (severity_order(&f.severity), f.role.clone(), f.title.clone()));
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
    let important_count = findings.iter().filter(|f| f.severity == "IMPORTANT").count();
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
    let do_now: Vec<_> = prioritized.iter().filter(|f| f.classification == "do_now").collect();
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
    let defer: Vec<_> = prioritized.iter().filter(|f| f.classification == "defer").collect();
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
        let project = args.project.as_ref().context(
            "--project is required when --dev-notes is enabled"
        )?;
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let reviews_dir = home.join("dev-notes").join(project).join("reviews");

        // Find the most recent timestamp directory
        let latest_dir = fs::read_dir(&reviews_dir)
            .with_context(|| format!("Failed to read reviews dir: {}", reviews_dir.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.path())
            .max();

        let input_dir = latest_dir.with_context(|| {
            format!("No review directories found in {}", reviews_dir.display())
        })?;

        let timestamp = input_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let plans_dir = home.join("dev-notes").join(project).join("plans");
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
