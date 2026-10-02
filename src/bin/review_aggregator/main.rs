//! Review Aggregator binary for the Auto-Dev Pipeline.
//! Aggregates findings from reviewers and generates a prioritized fix plan.

mod findings;
mod jev;
mod parse;
mod plan;

use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use auto_dev_pipeline::log;
use findings::dedup_findings;
use parse::parse_review_file;
use plan::generate_plan_with_provenance;

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

    /// Root directory for dev-notes (overrides $DEV_NOTES_ROOT and ~/Notes/dev-notes default)
    #[arg(long)]
    dev_notes_root: Option<PathBuf>,

    /// Previous plan file whose unresolved "Defer to Next Phase" items are
    /// carried into the new plan (with an attempt counter). Missing or
    /// unparseable files are skipped with a warning, not an error.
    #[arg(long)]
    carry_over_from: Option<PathBuf>,

    /// Re-classify findings with Jev (TypeSafe System One) after the
    /// heuristic pass. Requires TYPESAFE_API_KEY; any Jev failure degrades
    /// to the heuristic verdict (marked heuristic_fallback in the plan).
    /// Off by default: without this flag the output is identical to the
    /// heuristic-only run.
    #[arg(long, default_value = "false")]
    jev: bool,
}

fn main() -> Result<()> {
    auto_dev_pipeline::log::auto_detect_no_color();
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
            auto_dev_pipeline::devnotes::paths(&root, project).reviews
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
                log::warn(&format!(
                    "No review directories found in {} — generating empty plan",
                    reviews_dir.display()
                ));
                (reviews_dir.clone(), "empty".to_string())
            }
        };
        let plans_dir = auto_dev_pipeline::devnotes::paths(&root, project).plans;
        fs::create_dir_all(&plans_dir)?;
        let output_path = plans_dir.join(format!("{}-plan.md", timestamp));

        log::log("dev-notes mode enabled");
        log::log(&format!("Input:  {}", input_dir.display()));
        log::log(&format!("Output: {}", output_path.display()));

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
        log::log(&format!(
            "Parsed {} findings from {}",
            findings.len(),
            entry.path().display()
        ));
        all_findings.extend(findings);
    }

    let before_dedup = all_findings.len();
    all_findings = dedup_findings(all_findings);
    let deduped = before_dedup - all_findings.len();
    if deduped > 0 {
        log::log(&format!("Removed {} duplicate finding(s)", deduped));
    }

    if all_findings.is_empty() {
        log::warn("No findings found. Generating empty plan.");
    }

    // Optional Jev re-classification (phase b): one batch request for the
    // whole run, after dedup. Errors degrade to the heuristic verdict —
    // the plan is still generated. Provenance lines appear only when the
    // Jev pass actually ran: a failed client (no key) leaves the plan
    // identical to a heuristic-only run.
    let mut jev_ran = false;
    if args.jev {
        match jev::reclassify_with_jev(&mut all_findings) {
            Ok(changed) => {
                jev_ran = true;
                log::log(&format!(
                    "Jev reclassified {} finding(s) vs heuristic",
                    changed
                ));
            }
            Err(e) => {
                log::warn(&format!("{e}; using heuristic classifications"));
            }
        }
    }

    // Generate plan
    generate_plan_with_provenance(
        &all_findings,
        &output_path,
        args.carry_over_from.as_deref(),
        jev_ran,
    )?;

    // Human-readable summary lives entirely on stderr (json-output contract:
    // stdout stays clean for piping/parsing). The trailing DONE line is the
    // fixed anchor for both humans and wrapper agents.
    let severity_counts: HashMap<String, usize> =
        all_findings
            .iter()
            .fold(HashMap::new(), |mut acc: HashMap<String, usize>, f| {
                *acc.entry(f.severity.clone()).or_insert(0) += 1;
                acc
            });
    let do_now_count = all_findings
        .iter()
        .filter(|f| f.classification == findings::Classification::DoNow)
        .count();
    log::success(&format!("Plan generated: {}", output_path.display()));
    log::done(&format!(
        "findings={} do_now={} critical={} important={} minor={} plan={}",
        all_findings.len(),
        do_now_count,
        severity_counts.get("CRITICAL").unwrap_or(&0),
        severity_counts.get("IMPORTANT").unwrap_or(&0),
        severity_counts.get("MINOR").unwrap_or(&0),
        output_path.display()
    ));

    Ok(())
}
