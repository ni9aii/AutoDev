use anyhow::{Context, Result};
use auto_dev_pipeline::{git, log};
use clap::Parser;
use std::path::PathBuf;

/// CI Status Checker for Auto-Dev Pipeline
/// Checks GitHub Actions status via API
#[derive(Parser, Debug)]
#[command(name = "ci-check", version = "1.0.0")]
struct Args {
    /// Project path
    #[arg(default_value = ".")]
    project_path: PathBuf,
}

struct CiChecker {
    project_path: PathBuf,
}

impl CiChecker {
    fn new(project_path: PathBuf) -> Self {
        Self { project_path }
    }

    fn check_ci_status(&self, repo: &str) -> Result<bool> {
        let token = std::env::var("GITHUB_PAT").ok();

        if token.is_none() {
            log::warn("GITHUB_PAT not set, trying without auth (public repos only)");
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
            .header("User-Agent", "auto-dev-pipeline/1.0");

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
            log::warn(&format!("GitHub API error ({}): {}", status, msg));

            if status.as_u16() == 403 {
                log::warn("Rate limit exceeded. Set GITHUB_PAT for higher limits.");
            }
            return Ok(false);
        }

        let data: serde_json::Value = response.json().context("Failed to parse GitHub API response")?;

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
            let name = run.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
            let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("N/A");
            let branch = run.get("head_branch").and_then(|v| v.as_str()).unwrap_or("unknown");
            let url = run.get("html_url").and_then(|v| v.as_str()).unwrap_or("");

            let icon = match conclusion {
                "success" => "✅",
                "failure" => {
                    all_passed = false;
                    "❌"
                }
                _ => "🔄",
            };

            println!("  {} {}: {} ({}) on {}", icon, name, status, conclusion, branch);
            if !url.is_empty() {
                println!("     URL: {}", url);
            }
        }

        if !all_passed {
            log::error("Some recent workflow runs failed!");
            return Ok(false);
        }

        log::success("All recent CI runs passed");
        Ok(true)
    }

    fn check_local_tests(&self) -> Result<()> {
        log::log("Checking local test status...");
        let result = auto_dev_pipeline::test_runner::run_local_tests(&self.project_path)?;
        log::log(&format!("Running: {}", result.runner.name()));
        if result.success {
            log::success(&format!("Local tests passed ({})", result.runner.name()));
            Ok(())
        } else {
            let stderr_preview = if result.stderr.len() > 200 {
                format!("{}...", &result.stderr[..200])
            } else {
                result.stderr.clone()
            };
            anyhow::bail!(
                "Local tests failed ({}):\nstdout: {}\nstderr: {}",
                result.runner.name(),
                result.stdout,
                stderr_preview
            )
        }
    }

    fn run(&self) -> Result<()> {
        log::log("CI Status Checker v1.0.0 (Rust)");
        log::log(&format!("Project: {}", self.project_path.display()));

        // Get repo info
        match git::get_repo_info(&self.project_path) {
            Ok(repo) => {
                log::log(&format!("Repository: {}", repo));

                if let Err(e) = self.check_ci_status(&repo) {
                    log::warn(&format!("CI check failed: {}", e));
                }
            }
            Err(e) => {
                log::warn(&format!("Could not determine GitHub repo: {}", e));
            }
        }

        self.check_local_tests()?;

        log::success("All checks complete!");
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let checker = CiChecker::new(args.project_path);
    checker.run()?;
    Ok(())
}
