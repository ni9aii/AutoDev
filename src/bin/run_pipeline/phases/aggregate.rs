use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::log;
use std::path::{Path, PathBuf};

impl Pipeline {
    pub(crate) fn run_aggregate_phase(&self, review_dir: &Path) -> Result<PathBuf> {
        log::log("=== PHASE 2: AGGREGATE ===");

        let plan_path = if self.hermes_mode {
            let project_name = self
                .project_name
                .clone()
                .or_else(|| {
                    self.project_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            let plans_dir = self.dev_notes_root.join(&project_name).join("plans");
            std::fs::create_dir_all(&plans_dir)?;
            plans_dir.join(format!("{}-plan.md", self.timestamp))
        } else {
            self.output_dir.join(format!("{}-plan.md", self.timestamp))
        };

        let req = auto_dev_pipeline::bin_contract::AggregateRequest {
            input_dir: review_dir.to_path_buf(),
            output: plan_path.clone(),
            project: if self.hermes_mode {
                self.project_name.clone()
            } else {
                None
            },
            dev_notes_root: if self.hermes_mode {
                Some(self.dev_notes_root.clone())
            } else {
                None
            },
        };
        let args = req.to_args();
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let aggregator = auto_dev_pipeline::bin_contract::resolve_companion(
            auto_dev_pipeline::bin_contract::AGGREGATOR,
        );
        let output = self
            .runner
            .run(&aggregator, &arg_refs, None)
            .context("Failed to run review-aggregator")?;

        if !output.status.success() {
            log::warn(&format!(
                "review-aggregator exited with code: {:?}",
                output.status.code()
            ));
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                log::warn(&stderr);
            }
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        eprint!("{}", stdout);

        log::success(&format!(
            "Aggregation complete. Plan: {}",
            plan_path.display()
        ));
        Ok(plan_path)
    }
}
