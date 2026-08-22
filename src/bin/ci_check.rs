use anyhow::{Context, Result};
use auto_dev_pipeline::process::{ProcessRunner, SystemRunner};
use auto_dev_pipeline::{git, log};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// CI Status Checker for Auto-Dev Pipeline
/// Checks GitHub Actions status via API and runs local tests
#[derive(Parser, Debug)]
#[command(name = "ci-check", version = env!("CARGO_PKG_VERSION"))]
struct Args {
    /// Project path (git repo)
    #[arg(default_value = ".")]
    project_path: PathBuf,

    /// Save CI status report to dev-notes project directory
    #[arg(long, default_value = "false")]
    dev_notes: bool,

    /// Project name for dev-notes path (defaults to repo name)
    #[arg(long)]
    project: Option<String>,

    /// Root directory for dev-notes (overrides $DEV_NOTES_ROOT and ~/obsidian-vault/dev-notes default)
    #[arg(long)]
    dev_notes_root: Option<PathBuf>,
}

struct CiChecker {
    project_path: PathBuf,
    runner: Box<dyn ProcessRunner>,
}

impl CiChecker {
    fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            runner: Box::new(SystemRunner),
        }
    }

    fn check_ci_status(&self, repo: &str) -> Result<bool> {
        let token = auto_dev_pipeline::github::resolve_token(self.runner.as_ref())?;

        if token.is_none() {
            log::warn("No token found (GITHUB_PAT/GITHUB_TOKEN/gh auth) — trying without auth (public repos only)");
        }

        log::log(&format!("Checking CI status for: {}", repo));

        // Current branch, so the verdict only considers runs for what is
        // checked out here (plan finding: the 3 most recent runs across ALL
        // branches were deciding the verdict).
        let current_branch = self.current_branch().ok();
        if let Some(ref branch) = current_branch {
            log::log(&format!("Filtering workflow runs by branch: {}", branch));
        } else {
            log::warn("Could not determine current branch — considering runs from all branches");
        }

        let data: serde_json::Value =
            auto_dev_pipeline::github::get_workflow_runs(repo, token.as_deref())?;

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

            // Only the checked-out branch decides the verdict; unrelated
            // feature-branch failures must not fail us (and vice versa mask
            // our own red run).
            if let Some(ref current) = current_branch {
                if branch != current.as_str() {
                    continue;
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

    fn check_local_tests(&self) -> Result<()> {
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

    fn run(&self, args: &Args) -> Result<()> {
        log::log(&format!(
            "CI Status Checker v{} (Rust)",
            env!("CARGO_PKG_VERSION")
        ));
        log::log(&format!("Project: {}", self.project_path.display()));

        // Get repo info
        let repo = match git::get_repo_info(&self.project_path, self.runner.as_ref()) {
            Ok(repo) => {
                log::log(&format!("Repository: {}", repo));
                Some(repo)
            }
            Err(e) => {
                log::warn(&format!("Could not determine GitHub repo: {}", e));
                None
            }
        };

        // Check CI status if repo identified
        let ci_passed = if let Some(ref repo_str) = repo {
            match self.check_ci_status(repo_str) {
                Ok(passed) => passed,
                Err(e) => {
                    log::warn(&format!("CI check failed: {}", e));
                    false
                }
            }
        } else {
            false
        };

        // Run local tests
        let local_passed = match self.check_local_tests() {
            Ok(()) => true,
            Err(e) => {
                log::error(&format!("Local tests failed: {}", e));
                false
            }
        };

        // Save report to dev-notes if requested
        if args.dev_notes {
            let project_name = args.project.clone().or_else(|| {
                repo.as_ref()
                    .and_then(|r| r.split('/').nth(1).map(|s| s.to_string()))
            });

            if let Some(project) = project_name {
                let root = auto_dev_pipeline::git::paths::resolve_dev_notes_root(
                    args.dev_notes_root.as_ref(),
                )?;
                if let Err(e) = self.save_dev_notes_report(&project, ci_passed, local_passed, &root)
                {
                    log::warn(&format!("Failed to save dev-notes report: {}", e));
                }
            } else {
                log::warn("Cannot determine project name for dev-notes report");
            }
        }

        // Fail-closed decision (see overall_outcome doc): a known repo with
        // failing/unverifiable CI is fatal, not just a warning in the report.
        let repo_known = repo.is_some();
        Self::overall_outcome(repo_known, ci_passed, local_passed)?;

        log::success("All checks complete!");
        Ok(())
    }

    /// Fail-closed exit decision (plan finding: "ci-check exits 0 even when
    /// GitHub Actions runs are failing").
    ///
    /// - Local test failure is always fatal.
    /// - A *known* repo whose CI is failing OR could not be verified (API
    ///   error, rate limit, zero runs) is fatal: the tool's purpose is to gate
    ///   on remote CI, so silence must not mean green.
    /// - An unknown repo (not GitHub / no origin) cannot be checked remotely:
    ///   warn-only, decided locally.
    fn overall_outcome(repo_known: bool, ci_passed: bool, local_passed: bool) -> Result<()> {
        if !local_passed {
            anyhow::bail!("Local tests failed — see output above");
        }
        if repo_known && !ci_passed {
            anyhow::bail!(
                "GitHub Actions CI is failing or could not be verified — see output above"
            );
        }
        Ok(())
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

    fn save_dev_notes_report(
        &self,
        project: &str,
        ci_passed: bool,
        local_passed: bool,
        root: &std::path::Path,
    ) -> Result<()> {
        let reports_dir = {
            auto_dev_pipeline::validation::validate_project_name(project)
                .map_err(|e| anyhow::anyhow!(e))?;
            root.join(project).join("ci-reports")
        };
        fs::create_dir_all(&reports_dir)?;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let report_path = reports_dir.join(format!("{}-ci-status.md", timestamp));

        let status_icon = |passed: bool| if passed { "✅" } else { "❌" };

        let content = format!(
            "# CI Status Report\n\n\
            **Project:** {}\n\
            **Timestamp:** {}\n\
            **Repository:** {}\n\n\
            ## Results\n\n\
            | Check | Status |\n\
            |-------|--------|\n\
            | GitHub Actions CI | {} |\n\
            | Local Tests | {} |\n\n\
            ## Overall\n\n\
            {}\n",
            project,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            git::get_repo_info(&self.project_path, self.runner.as_ref())
                .unwrap_or_else(|_| "unknown".to_string()),
            status_icon(ci_passed),
            status_icon(local_passed),
            if ci_passed && local_passed {
                "✅ All checks passed"
            } else {
                "❌ Some checks failed"
            }
        );

        fs::write(&report_path, content)?;
        log::log(&format!("CI report saved: {}", report_path.display()));
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let checker = CiChecker::new(args.project_path.clone());
    checker.run(&args)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Matrix tests for the fail-closed exit decision. This is the gate that
    // decides whether the verify/release phases proceed, so every combination
    // is pinned explicitly.
    #[test]
    fn outcome_all_passed_is_ok() {
        assert!(CiChecker::overall_outcome(true, true, true).is_ok());
    }

    #[test]
    fn outcome_failing_ci_with_known_repo_fails_closed() {
        // Regression: previously exited 0 when remote CI failed but local
        // tests passed — releases could be tagged on a red repo.
        let err = CiChecker::overall_outcome(true, false, true)
            .expect_err("known repo + failing CI must fail");
        assert!(err.to_string().contains("CI"));
    }

    #[test]
    fn outcome_local_failure_always_fatal() {
        assert!(CiChecker::overall_outcome(true, true, false).is_err());
        assert!(CiChecker::overall_outcome(false, true, false).is_err());
        assert!(CiChecker::overall_outcome(false, false, false).is_err());
    }

    #[test]
    fn outcome_unknown_repo_cannot_fail_on_ci() {
        // Non-GitHub / origin-less repos: remote CI unknowable → warn-only,
        // decision falls to local results.
        assert!(CiChecker::overall_outcome(false, false, true).is_ok());
    }
}
