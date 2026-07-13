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
        let token = std::env::var("GITHUB_PAT")
            .ok()
            .or_else(|| std::env::var("GITHUB_TOKEN").ok())
            .or_else(|| self.gh_auth_token().ok());

        if token.is_none() {
            log::warn("GITHUB_PAT not set and gh auth token failed, trying without auth (public repos only)");
        }

        log::log(&format!("Checking CI status for: {}", repo));

        let api_url = format!(
            "https://api.github.com/repos/{}/actions/runs?per_page=5",
            repo
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        let mut request = client
            .get(&api_url)
            .header("Accept", "application/vnd.github+json")
            .header(
                "User-Agent",
                format!("auto-dev-pipeline/{}", env!("CARGO_PKG_VERSION")),
            );

        if let Some(ref token) = token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().context("Failed to call GitHub API")?;
        let status = response.status();

        if !status.is_success() {
            let body: serde_json::Value = response.json().unwrap_or_default();
            let msg = body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");

            if status.as_u16() == 403 {
                log::warn("Rate limit exceeded. Set GITHUB_PAT for higher limits.");
            }
            anyhow::bail!("GitHub API error ({}): {}", status, msg);
        }

        let data: serde_json::Value = response
            .json()
            .context("Failed to parse GitHub API response")?;

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

        for run in runs.iter().take(3) {
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
            let branch = run
                .get("head_branch")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let url = run.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

            let icon = match conclusion {
                "success" => "✅",
                "failure" => {
                    all_passed = false;
                    "❌"
                }
                _ => "🔄",
            };

            println!(
                "  {} {}: {} ({}) on {}",
                icon, name, status, conclusion, branch
            );
            if !url.is_empty() {
                println!("     URL: {}", url);
            }
        }

        if !all_passed {
            anyhow::bail!("Some recent workflow runs failed!");
        }

        log::success("All recent CI runs passed");
        Ok(true)
    }

    fn check_local_tests(&self) -> Result<()> {
        log::log("Checking local test status...");
        let result = auto_dev_pipeline::test_runner::run_local_tests(
            &self.project_path,
            self.runner.as_ref(),
        )?;
        log::log(&format!("Running: {}", result.runner.name()));
        if result.success {
            log::success(&format!("Local tests passed ({})", result.runner.name()));
            Ok(())
        } else {
            let stderr_preview = auto_dev_pipeline::markdown::safe_truncate(&result.stderr, 200);
            anyhow::bail!(
                "Local tests failed ({}):\nstdout: {}\nstderr: {}...",
                result.runner.name(),
                result.stdout,
                stderr_preview
            )
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

        if !local_passed {
            anyhow::bail!("Local tests failed — see output above");
        }

        log::success("All checks complete!");
        Ok(())
    }

    /// Try to get GitHub token from `gh auth token` CLI
    fn gh_auth_token(&self) -> Result<String> {
        let output = self
            .runner
            .run("gh", &["auth", "token"], None)
            .context("Failed to run 'gh auth token'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh auth token failed: {}", stderr);
        }

        let token =
            String::from_utf8(output.stdout).context("gh auth token returned invalid UTF-8")?;
        Ok(token.trim().to_string())
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
