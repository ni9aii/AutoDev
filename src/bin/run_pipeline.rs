use anyhow::{Context, Result};
use auto_dev_pipeline::{log, markdown, test_runner};
use clap::{Parser, ValueEnum};
use shlex::try_quote;
use std::path::PathBuf;
use std::process::Command;

/// Available pipeline phases
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Phase {
    /// Run full pipeline: review → aggregate → execute → verify
    Full,
    /// Run review phase only
    Review,
    /// Run review + aggregate phases
    Plan,
    /// Run release phase (create git tag)
    Release,
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Phase::Full => write!(f, "full"),
            Phase::Review => write!(f, "review"),
            Phase::Plan => write!(f, "plan"),
            Phase::Release => write!(f, "release"),
        }
    }
}

/// Auto-Dev Pipeline Entry Point
/// Orchestrates: review → aggregate → execute → verify
#[derive(Parser, Debug)]
#[command(name = "run-pipeline", version = "1.0.0", about = "Auto-Dev Pipeline")]
struct Args {
    /// Project path
    #[arg(default_value = ".")]
    project_path: PathBuf,

    /// Phase to run
    #[arg(value_enum, default_value = "full")]
    phase: Phase,

    /// Version tag for release (e.g., v0.2.0)
    #[arg(short, long)]
    version: Option<String>,
}

struct Pipeline {
    project_path: PathBuf,
    phase: Phase,
    version: Option<String>,
    timestamp: String,
    output_dir: PathBuf,
}

/// Individual fix parsed from Do Now section
struct Fix {
    title: String,
    severity: String,
    file: Option<String>,
    description: String,
}

impl Pipeline {
    fn new(args: Args) -> Result<Self> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let output_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hermes/plans/auto-dev");

        std::fs::create_dir_all(&output_dir)?;

            Ok(Self {
            project_path: args.project_path,
            phase: args.phase,
            version: args.version,
            timestamp,
            output_dir,
        })
    }





    fn check_prerequisites(&self) -> Result<()> {
        log::log("Checking prerequisites...");

        // Check git repo
        let git_dir = self.project_path.join(".git");
        if !git_dir.exists() {
            anyhow::bail!("Not a git repository: {}", self.project_path.display());
        }

        // Check Claude Code CLI
        match Command::new("claude").arg("--version").output() {
            Ok(_) => log::log("Claude Code CLI: found"),
            Err(_) => {
                log::warn("Claude Code CLI not found. Install: npm install -g @anthropic-ai/claude-code");
                log::warn("Falling back to manual execution mode.");
            }
        }

        log::success("Prerequisites OK");
        Ok(())
    }

    fn run_review_phase(&self) -> Result<PathBuf> {
        log::log("=== PHASE 1: REVIEW ===");
        log::log("Launching 4 reviewers via Claude Code...");

        let review_dir = self.output_dir.join(format!("{}-reviews", self.timestamp));
        std::fs::create_dir_all(&review_dir)?;

        let reviewers = [
            ("code", "Code Reviewer: check logic, style, idioms, performance"),
            ("security", "Security Reviewer: check vulnerabilities, unsafe code, secrets"),
            ("architecture", "Architecture Reviewer: check structure, coupling, patterns"),
            ("devops", "DevOps Reviewer: check CI/CD, dependencies, build, deploy"),
        ];

        for (name, prompt) in &reviewers {
            log::log(&format!("Starting {} review...", name));
            
            let project_path_str = self.project_path.display().to_string();
            let project_path_quoted = try_quote(&project_path_str)?;
            let review_prompt = format!(
                "You are a {}. Review the project at {}.\n\n\
                Read all source files, then produce a markdown report with:\n\
                - ### [CRITICAL] / [IMPORTANT] / [MINOR] sections\n\
                - Each finding must have: title, description, file path, line number\n\n\
                Output format:\n\
                ### [SEVERITY] Title\n\
                Description...\n\
                File: `path/to/file.rs`\n\
                Line: 42\n\n\
                Save the report to: {}/{}-review.md",
                prompt,
                project_path_quoted,
                review_dir.display(),
                name
            );

            let output = Command::new("claude")
                .args([
                    "-p",
                    &review_prompt,
                    "--allowedTools",
                    "Read,Edit,Bash",
                    "--max-turns",
                    "30",
                ])
                .current_dir(&self.project_path)
                .output()
                .context(format!("Failed to run {} reviewer", name))?;

            if !output.status.success() {
                log::warn(&format!("{} reviewer exited with non-zero status", name));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                log::log(&format!("{} review output:\n{}", name, stdout));
            }
        }

        log::success(&format!("Review phase complete. Reports in: {}", review_dir.display()));
        Ok(review_dir)
    }

    fn run_aggregate_phase(&self, review_dir: &PathBuf) -> Result<PathBuf> {
        log::log("=== PHASE 2: AGGREGATE ===");

        let plan_path = self.output_dir.join(format!("{}-plan.md", self.timestamp));

        let output = Command::new("review-aggregator")
            .arg("--input-dir")
            .arg(review_dir)
            .arg("--output")
            .arg(&plan_path)
            .output()
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
        print!("{}", stdout);

        log::success(&format!("Aggregation complete. Plan: {}", plan_path.display()));
        Ok(plan_path)
    }

    fn run_execute_phase(&self, plan_path: &PathBuf) -> Result<()> {
        log::log("=== PHASE 3: EXECUTE ===");

        let plan_content = std::fs::read_to_string(plan_path)
            .context("Failed to read plan file")?;

        // Extract Do Now fixes from plan
        let do_now_section = markdown::extract_section(&plan_content, "Do Now");
        if do_now_section.is_empty() {
            log::warn("No Do Now fixes found in plan");
            return Ok(());
        }

        log::log(&format!("Found Do Now section ({} chars)", do_now_section.len()));

        // Parse individual fixes from Do Now section
        let fixes = self.parse_fixes(&do_now_section);
        log::log(&format!("Parsed {} fixes to execute", fixes.len()));

        for (i, fix) in fixes.iter().enumerate() {
            log::log(&format!("Executing fix {}/{}: {}", i + 1, fixes.len(), fix.title));

            let project_path_str = self.project_path.display().to_string();
            let project_path_quoted = try_quote(&project_path_str)?;
            let title_quoted = try_quote(&fix.title)?;
            let severity_quoted = try_quote(&fix.severity)?;
            let file_quoted = try_quote(fix.file.as_deref().unwrap_or("unknown"))?;
            let description_quoted = try_quote(&fix.description)?;
            
            let task = format!(
                "Fix the following issue in the project at {}:\n\n\
                Title: {}\n\
                Severity: {}\n\
                File: {}\n\
                Description: {}\n\n\
                Apply the fix directly to the source files. Use Read and Edit tools.",
                project_path_quoted,
                title_quoted,
                severity_quoted,
                file_quoted,
                description_quoted
            );

            self.execute_via_claude(&task)?;
            log::success(&format!("Fix {} complete", i + 1));
        }

        log::success("Execution phase complete");
        Ok(())
    }

    /// Parse individual fixes from Do Now markdown section
    fn parse_fixes(&self, do_now_section: &str) -> Vec<Fix> {
        let mut fixes = Vec::new();
        let lines: Vec<&str> = do_now_section.lines().collect();
        let mut current_fix: Option<Fix> = None;

        for line in lines {
            let trimmed = line.trim();
            
            // New fix starts with "### Fix N:"
            if trimmed.starts_with("### Fix ") {
                if let Some(fix) = current_fix.take() {
                    fixes.push(fix);
                }
                let title = trimmed
                    .trim_start_matches("### Fix ")
                    .split_once(':')
                    .map(|x| x.1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
                current_fix = Some(Fix {
                    title,
                    severity: "UNKNOWN".to_string(),
                    file: None,
                    description: String::new(),
                });
            } else if let Some(ref mut fix) = current_fix {
                if trimmed.starts_with("**Severity:**") {
                    fix.severity = trimmed
                        .trim_start_matches("**Severity:**")
                        .trim()
                        .to_string();
                } else if trimmed.starts_with("**File:**") {
                    let file_str = trimmed
                        .trim_start_matches("**File:**")
                        .trim()
                        .trim_matches('`')
                        .to_string();
                    fix.file = Some(file_str);
                } else if !trimmed.starts_with("**") && !trimmed.is_empty() && trimmed != "**Description:**" {
                    fix.description.push_str(line);
                    fix.description.push('\n');
                }
            }
        }

        if let Some(fix) = current_fix {
            fixes.push(fix);
        }

        fixes
    }

    fn execute_via_claude(&self, task: &str) -> Result<()> {
        let output = Command::new("claude")
            .args([
                "-p",
                task,
                "--allowedTools",
                "Read,Edit,Bash",
                "--max-turns",
                "15",
            ])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to run Claude Code")?;

        if !output.status.success() {
            log::warn("Claude Code exited with non-zero status");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);

        Ok(())
    }

    fn run_release_phase(&self, version: &str) -> Result<()> {
        log::log("=== PHASE 5: RELEASE ===");

        // Build release binary
        log::log("Building release binary...");
        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to build release binary")?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            anyhow::bail!("Release build failed: {}", stderr);
        }
        log::success("Release build complete");

        // Create git tag
        log::log(&format!("Creating tag: {}", version));
        let tag_output = Command::new("git")
            .args(["tag", "-a", version, "-m", &format!("Release {}", version)])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to create git tag")?;

        if !tag_output.status.success() {
            let stderr = String::from_utf8_lossy(&tag_output.stderr);
            anyhow::bail!("Failed to create tag: {}", stderr);
        }
        log::success(&format!("Tag {} created", version));

        // Push tag
        log::log("Pushing tag...");
        let push_output = Command::new("git")
            .args(["push", "origin", version])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to push tag")?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            anyhow::bail!("Failed to push tag: {}", stderr);
        }
        log::success("Tag pushed to origin");

        // Create GitHub Release via API (reqwest — token stays in process memory)
        log::log("Creating GitHub Release...");
        let repo = auto_dev_pipeline::git::get_repo_info(&self.project_path)?;
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GITHUB_PAT"))
            .context("GITHUB_TOKEN or GITHUB_PAT must be set")?;

        let release_url = format!("https://api.github.com/repos/{}/releases", repo);
        let release_body = format!(
            "{{\"tag_name\":\"{}\",\"name\":\"Release {}\",\"body\":\"Auto-generated release\",\"draft\":false,\"prerelease\":false}}",
            version, version
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        let response = client
            .post(&release_url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "auto-dev-pipeline/1.0")
            .body(release_body)
            .send()
            .context("Failed to create GitHub release")?;

        if response.status().is_success() {
            log::success(&format!("GitHub Release {} created", version));
        } else {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            log::warn(&format!("GitHub release creation failed ({}): {}", status, body));
        }

        log::success(&format!("Release {} complete", version));
        Ok(())
    }

    fn run_verify_phase(&self) -> Result<()> {
        log::log("=== PHASE 4: VERIFY ===");

        // Run local tests (fail-fast)
        self.run_local_tests()?;

        // Check CI status
        log::log("Checking CI status...");
        let ci_output = Command::new("ci-check")
            .arg(&self.project_path)
            .output()
            .context("Failed to run ci-check")?;

        if !ci_output.status.success() {
            let stderr = String::from_utf8_lossy(&ci_output.stderr);
            anyhow::bail!("CI check failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&ci_output.stdout);
        print!("{}", stdout);

        log::success("Verification complete");
        Ok(())
    }

    fn run_local_tests(&self) -> Result<()> {
        log::log("Checking local test status...");

        match test_runner::run_local_tests(&self.project_path) {
            Ok(result) => {
                log::log(&format!("Running: {}", result.runner.name()));
                if result.success {
                    log::success("Local tests passed");
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
            Err(e) => {
                log::warn(&format!("No test runner found: {}", e));
                anyhow::bail!("No test runner found in project: {}", e)
            }
        }
    }

    fn run(&self) -> Result<()> {
        log::log("Auto-Dev Pipeline v1.0.0 (Rust)");
        log::log(&format!("Project: {}", self.project_path.display()));
        log::log(&format!("Phase: {}", self.phase));
        log::log(&format!("Output: {}", self.output_dir.display()));

        self.check_prerequisites()?;

        match self.phase {
            Phase::Review => {
                self.run_review_phase()?;
            }
            Phase::Plan => {
                let review_dir = self.run_review_phase()?;
                self.run_aggregate_phase(&review_dir)?;
            }
            Phase::Full => {
                let review_dir = self.run_review_phase()?;
                let plan_path = self.run_aggregate_phase(&review_dir)?;
                self.run_execute_phase(&plan_path)?;
                self.run_verify_phase()?;
            }
            Phase::Release => {
                let version = self.version.as_ref()
                    .context("Release phase requires --version argument (e.g., --version v0.2.0)")?;
                self.run_verify_phase()?;
                self.run_release_phase(version)?;
            }
        }

        log::success("Pipeline complete!");
        log::log(&format!("Reports: {}", self.output_dir.display()));
        Ok(())
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pipeline = Pipeline::new(args)?;
    pipeline.run()?;
    Ok(())
}
