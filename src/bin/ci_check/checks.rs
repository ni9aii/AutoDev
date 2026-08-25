//! GitHub Actions CI status check and local test runner checks.

use super::CiChecker;
use anyhow::{Context, Result};
use auto_dev_pipeline::log;

impl CiChecker {
    pub(crate) fn check_ci_status(&self, repo: &str) -> Result<bool> {
        let token = auto_dev_pipeline::github::resolve_token(self.runner.as_ref())?;

        if token.is_none() {
            log::warn("No token found (GITHUB_PAT/GITHUB_TOKEN/gh auth) — trying without auth (public repos only)");
        }

        log::log(&format!("Checking CI status for: {}", repo));

        // Prefer runs for the checked-out HEAD SHA: stale red runs from
        // earlier commits on the same branch must not mask a green HEAD
        // (and vice versa). Fall back to branch filtering when the SHA
        // cannot be determined.
        let head_sha = self
            .runner
            .run("git", &["rev-parse", "HEAD"], Some(&self.project_path))
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty());

        // Branch fallback (used only when the HEAD SHA is unavailable):
        // restrict the verdict to runs of the checked-out branch.
        let current_branch = if head_sha.is_none() {
            self.current_branch().ok()
        } else {
            None
        };

        let data: serde_json::Value = if let Some(ref sha) = head_sha {
            log::log(&format!(
                "Filtering workflow runs by HEAD commit: {}",
                &sha[..sha.len().min(12)]
            ));
            auto_dev_pipeline::github::get_workflow_runs_for_sha(repo, sha, token.as_deref())?
        } else {
            // Current branch, so the verdict only considers runs for what is
            // checked out here (plan finding: the 3 most recent runs across ALL
            // branches were deciding the verdict).
            if let Some(ref branch) = current_branch {
                log::log(&format!("Filtering workflow runs by branch: {}", branch));
            } else {
                log::warn(
                    "Could not determine current branch — considering runs from all branches",
                );
            }
            auto_dev_pipeline::github::get_workflow_runs(repo, token.as_deref())?
        };

        let total_count = data
            .get("total_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if total_count == 0 {
            log::warn("No CI workflows found");
            return Ok(false);
        }

        log::log(&format!("Found {} recent workflow runs", total_count));

        let runs = data
            .get("workflow_runs")
            .and_then(|v| v.as_array())
            .context("No workflow_runs in response")?;

        let mut all_passed = true;
        let mut considered = 0usize;

        for run in runs {
            let branch = run
                .get("head_branch")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // SHA-filtered runs are already scoped to our commit; in the
            // branch-filter fallback, only the checked-out branch decides
            // the verdict (unrelated feature-branch failures must not fail
            // us — and vice versa mask our own red run).
            if head_sha.is_none() {
                if let Some(ref current) = current_branch {
                    if branch != current.as_str() {
                        continue;
                    }
                }
            }

            let name = run
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let status = run
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let conclusion = run
                .get("conclusion")
                .and_then(|v| v.as_str())
                .unwrap_or("N/A");
            let url = run.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

            considered += 1;

            let icon = match (status, conclusion) {
                ("completed", "success") => "✅",
                ("completed", "failure")
                | ("completed", "timed_out")
                | ("completed", "startup_failure") => {
                    all_passed = false;
                    "❌"
                }
                ("completed", _) => {
                    // cancelled / neutral / skipped etc. — not a pass.
                    all_passed = false;
                    "⏭️"
                }
                _ => {
                    // in_progress / queued / requested: undecided, and an
                    // unfinished run is NOT a passing signal.
                    all_passed = false;
                    "🔄"
                }
            };

            // stderr: this is human status output; stdout of ci-check must
            // stay clean for the --json piping contract.
            eprintln!(
                "  {} {}: {} ({}) on {}",
                icon, name, status, conclusion, branch
            );
            if !url.is_empty() {
                eprintln!("     URL: {}", url);
            }

            if considered >= 3 {
                break;
            }
        }

        if considered == 0 {
            log::warn("No workflow runs found for the current branch");
            return Ok(false);
        }

        if !all_passed {
            anyhow::bail!("Some recent workflow runs failed!");
        }

        log::success("All recent CI runs passed");
        Ok(true)
    }

    pub(crate) fn check_local_tests(&self) -> Result<()> {
        log::log("Checking local test status...");
        match auto_dev_pipeline::test_runner::run_local_tests(
            &self.project_path,
            self.runner.as_ref(),
        ) {
            Ok(Some(result)) => {
                log::log(&format!("Running: {}", result.runner.name()));
                if result.success {
                    log::success(&format!("Local tests passed ({})", result.runner.name()));
                    Ok(())
                } else {
                    let stderr_preview =
                        auto_dev_pipeline::markdown::safe_truncate(&result.stderr, 200);
                    anyhow::bail!(
                        "Local tests failed ({}):\nstdout: {}\nstderr: {}...",
                        result.runner.name(),
                        result.stdout,
                        stderr_preview
                    )
                }
            }
            Ok(None) => {
                // No runner configured or command unavailable (e.g. `make`
                // absent on Windows). Non-fatal: skip, don't fail the check.
                log::warn("No local test runner available — skipping local test step");
                Ok(())
            }
            Err(e) => {
                log::warn(&format!("Local test runner error (skipped): {}", e));
                Ok(())
            }
        }
    }

    /// Current git branch of the project (empty/detached HEAD → error, caller
    /// falls back to considering all branches).
    fn current_branch(&self) -> Result<String> {
        let output = self
            .runner
            .run(
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
                Some(&self.project_path),
            )
            .context("Failed to run 'git rev-parse --abbrev-ref HEAD'")?;
        if !output.status.success() {
            anyhow::bail!("git rev-parse failed");
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if branch.is_empty() || branch == "HEAD" || branch == "undefined" {
            anyhow::bail!("detached or unnamed HEAD");
        }
        Ok(branch)
    }
}
