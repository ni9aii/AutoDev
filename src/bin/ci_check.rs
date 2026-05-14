use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;

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

    fn log(msg: &str) {
        println!("{} {}", "[ci-check]".blue(), msg);
    }

    fn warn(msg: &str) {
        println!("{} {}", "[ci-check]".yellow(), msg);
    }

    fn error(msg: &str) {
        println!("{} {}", "[ci-check]".red(), msg);
    }

    fn success(msg: &str) {
        println!("{} {}", "[ci-check]".green(), msg);
    }

    fn get_repo_info(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to get git remote")?;

        if !output.status.success() {
            anyhow::bail!("No git remote 'origin' found");
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Parse GitHub URL
        // Group 1: owner, Group 2: repo name (optional .git suffix)
        let re = Regex::new(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?$")?;
        if let Some(caps) = re.captures(&remote_url) {
            let owner = &caps[1];
            let repo = &caps[2];
            Ok(format!("{}/{}", owner, repo))
        } else {
            anyhow::bail!("Not a GitHub repository: {}", remote_url)
        }
    }

    fn check_ci_status(&self, repo: &str) -> Result<bool> {
        let token = std::env::var("GITHUB_PAT").ok();

        if token.is_none() {
            Self::warn("GITHUB_PAT not set, trying without auth (public repos only)");
        }

        Self::log(&format!("Checking CI status for: {}", repo));

        let api_url = format!(
            "https://api.github.com/repos/{}/actions/runs?per_page=5",
            repo
        );

        let client = reqwest::blocking::Client::new();
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
            Self::warn(&format!("GitHub API error ({}): {}", status, msg));

            if status.as_u16() == 403 {
                Self::warn("Rate limit exceeded. Set GITHUB_PAT for higher limits.");
            }
            return Ok(false);
        }

        let data: serde_json::Value = response.json().context("Failed to parse GitHub API response")?;

        let total_count = data
            .get("total_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        if total_count == 0 {
            Self::warn("No CI workflows found");
            return Ok(false);
        }

        Self::log(&format!("Found {} recent workflow runs", total_count));

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
            Self::error("Some recent workflow runs failed!");
            return Ok(false);
        }

        Self::success("All recent CI runs passed");
        Ok(true)
    }

    fn check_local_tests(&self) -> Result<()> {
        Self::log("Checking local test status...");

        if self.project_path.join("Makefile").exists() {
            let output = Command::new("make")
                .arg("test")
                .current_dir(&self.project_path)
                .output()?;
            if output.status.success() {
                Self::success("Local tests passed (make test)");
            } else {
                Self::warn("Local tests failed (make test)");
            }
        } else if self.project_path.join("package.json").exists() {
            let output = Command::new("npm")
                .arg("test")
                .current_dir(&self.project_path)
                .output()?;
            if output.status.success() {
                Self::success("Local tests passed (npm test)");
            } else {
                Self::warn("Local tests failed (npm test)");
            }
        } else if self.project_path.join("pyproject.toml").exists()
            || self.project_path.join("setup.py").exists()
        {
            let output = Command::new("pytest")
                .current_dir(&self.project_path)
                .output()?;
            if output.status.success() {
                Self::success("Local tests passed (pytest)");
            } else {
                Self::warn("Local tests failed (pytest)");
            }
        }

        Ok(())
    }

    fn run(&self) -> Result<()> {
        Self::log("CI Status Checker v1.0.0 (Rust)");
        Self::log(&format!("Project: {}", self.project_path.display()));

        // Get repo info
        match self.get_repo_info() {
            Ok(repo) => {
                Self::log(&format!("Repository: {}", repo));

                if let Err(e) = self.check_ci_status(&repo) {
                    Self::warn(&format!("CI check failed: {}", e));
                }
            }
            Err(e) => {
                Self::warn(&format!("Could not determine GitHub repo: {}", e}));
            }
        }

        self.check_local_tests()?;

        Self::success("All checks complete!");
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let checker = CiChecker::new(args.project_path);
    checker.run()?;
    Ok(())
}
