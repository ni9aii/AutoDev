//! CI Status Checker binary for the Auto-Dev Pipeline.
//! Checks GitHub Actions status via API and runs local tests.

mod checks;
mod report;

use anyhow::Result;
use auto_dev_pipeline::process::ProcessRunner;
use auto_dev_pipeline::{git, log};
use clap::Parser;
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
    pub(crate) project_path: PathBuf,
    pub(crate) runner: Box<dyn ProcessRunner>,
}

impl CiChecker {
    fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            runner: Box::new(auto_dev_pipeline::process::SystemRunner),
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
