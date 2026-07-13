use anyhow::Context;
use auto_dev_pipeline::log;
use serde::Serialize;

use crate::pipeline::build::Pipeline;
use crate::Phase;

/// Machine-readable result of a pipeline run (emitted with `--json`).
#[derive(Debug, Serialize)]
struct PipelineSummary<'a> {
    status: &'a str,
    version: &'a str,
    project: String,
    phase: String,
    mode: &'a str,
    timestamp: &'a str,
    output_dir: String,
}

/// Dispatch a built pipeline to its phase sequence.
pub(crate) fn run(pipeline: &Pipeline) -> anyhow::Result<()> {
    log::log(&format!(
        "Auto-Dev Pipeline v{} (Rust)",
        env!("CARGO_PKG_VERSION")
    ));
    log::log(&format!("Project: {}", pipeline.project_path.display()));
    log::log(&format!("Phase: {}", pipeline.phase));
    log::log(&format!(
        "Mode: {}",
        if pipeline.hermes_mode {
            "Hermes (delegate_task)"
        } else {
            "Legacy (Claude CLI)"
        }
    ));
    log::log(&format!("Output: {}", pipeline.output_dir.display()));

    pipeline.check_prerequisites()?;

    match pipeline.phase {
        Phase::Review => {
            pipeline.run_review_phase()?;
        }
        Phase::Plan => {
            let review_dir = pipeline.run_review_phase()?;
            pipeline.run_aggregate_phase(&review_dir)?;
        }
        Phase::Full => {
            let review_dir = pipeline.run_review_phase()?;
            let plan_path = pipeline.run_aggregate_phase(&review_dir)?;
            pipeline.run_execute_phase(&plan_path)?;
            pipeline.run_verify_phase()?;
        }
        Phase::Release => {
            let version = pipeline.version.as_ref().context(
                "Release phase requires --release-version argument (e.g., --release-version v0.5.0)",
            )?;
            pipeline.run_verify_phase()?;
            pipeline.run_release_phase(version)?;
        }
    }

    if pipeline.json {
        let summary = PipelineSummary {
            status: "success",
            version: env!("CARGO_PKG_VERSION"),
            project: pipeline.project_path.display().to_string(),
            phase: pipeline.phase.to_string(),
            mode: if pipeline.hermes_mode {
                "hermes"
            } else {
                "legacy"
            },
            timestamp: &pipeline.timestamp,
            output_dir: pipeline.output_dir.display().to_string(),
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&summary).context("serialize JSON summary")?
        );
    } else {
        log::success("Pipeline complete!");
        log::log(&format!("Reports: {}", pipeline.output_dir.display()));
    }

    Ok(())
}
