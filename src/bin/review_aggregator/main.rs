//! Review Aggregator binary for the Auto-Dev Pipeline.
//! Aggregates findings from reviewers and generates a prioritized fix plan.

mod findings;
mod parse;
mod plan;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use findings::dedup_findings;
use parse::parse_review_file;
use plan::generate_plan;

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

        // Find the most recent timestamp directory. A missing reviews/ dir
        // (fresh project) is not an error: create it and fall through to the
        // empty-plan path below.
        if !reviews_dir.exists() {
            fs::create_dir_all(&reviews_dir).with_context(|| {
                format!("Failed to create reviews dir: {}", reviews_dir.display())
            })?;
        }
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
                // Keep the empty-plan promise: no review directories under an
                // existing reviews/ means a fresh project — emit the empty
                // plan instead of erroring.
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
