use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::log;
use std::path::{Path, PathBuf};

impl Pipeline {
    pub(crate) fn run_aggregate_phase(&self, review_dir: &Path) -> Result<PathBuf> {
        log::log("=== PHASE 2: AGGREGATE ===");

        // Guard (Task 4): in hermes mode the review phase only prints
        // instructions; if the agent never ran them, the review dir holds no
        // reports. Fail fast with an actionable message instead of letting the
        // aggregator silently emit an empty plan. Reports may live directly in
        // the dir or in its per-run `<ts>` subdirectory — search recursively.
        let has_reports = walkdir::WalkDir::new(review_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_type().is_file()
                    && e.file_name()
                        .to_str()
                        .is_some_and(|n| n.ends_with("-review.md"))
            });
        if !has_reports {
            anyhow::bail!(
                "no review reports found in {} — review instructions were not executed?",
                review_dir.display()
            );
        }

        let plan_path = {
            // project_name is validated (allowlist) at Pipeline::new time, so
            // joining it onto the dev-notes root is safe.
            let project_name = self
                .project_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let plans_dir = self.dev_notes_root.join(&project_name).join("plans");
            std::fs::create_dir_all(&plans_dir)?;
            plans_dir.join(format!("{}-plan.md", self.timestamp))
        };

        let req = auto_dev_pipeline::bin_contract::AggregateRequest {
            input_dir: review_dir.to_path_buf(),
            output: plan_path.clone(),
            project: self.project_name.clone(),
            dev_notes_root: Some(self.dev_notes_root.clone()),
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
            // Fatal (plan finding: aggregate phase swallowed aggregator
            // failure and returned a plan path anyway): a failed aggregation
            // must stop Plan/Full here — otherwise execute later dies with a
            // generic "Failed to read plan file" (or worse, proceeds on a
            // stale plan) and the root cause is masked.
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "review-aggregator exited with code {:?}: {}",
                output.status.code(),
                stderr.trim()
            );
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
