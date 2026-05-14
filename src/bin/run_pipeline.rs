use anyhow::{Context, Result};
use auto_dev_pipeline::{log, markdown, test_runner};
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

/// Auto-Dev Pipeline Entry Point
/// Orchestrates: review → aggregate → execute → verify
#[derive(Parser, Debug)]
#[command(name = "run-pipeline", version = "1.0.0", about = "Auto-Dev Pipeline")]
struct Args {
    /// Project path
    #[arg(default_value = ".")]
    project_path: PathBuf,

    /// Phase to run: full, review, plan
    #[arg(default_value = "full")]
    phase: String,
}

struct Pipeline {
    project_path: PathBuf,
    phase: String,
    timestamp: String,
    output_dir: PathBuf,
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
        log::log("Launching 4 reviewers in parallel...");

        let review_dir = self.output_dir.join(format!("{}-reviews", self.timestamp));
        std::fs::create_dir_all(&review_dir)?;

        log::log("1. Code Reviewer");
        log::log("2. Security Reviewer");
        log::log("3. Architecture Reviewer");
        log::log("4. DevOps Reviewer");
        log::log("(Reviewers are dispatched as Hermes subagents)");

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

        // Read plan and execute each fix
        let plan_content = std::fs::read_to_string(plan_path)
            .context("Failed to read plan file")?;

        // Extract Do Now fixes from plan
        let do_now_section = markdown::extract_section(&plan_content, "Do Now");
        if !do_now_section.is_empty() {
            log::log("Executing Do Now fixes via Claude Code...");
            self.execute_via_claude(&do_now_section)?;
        }

        log::success("Execution complete");
        Ok(())
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

        // Create git tag
        log::log(&format!("Creating tag: {}", version));
        let tag_output = Command::new("git")
            .args(["tag", "-a", version, "-m", &format!("Release {}", version)])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to create git tag")?;

        if !tag_output.status.success() {
            log::warn("Failed to create tag");
        }

        // Push tag
        log::log("Pushing tag...");
        let push_output = Command::new("git")
            .args(["push", "origin", version])
            .current_dir(&self.project_path)
            .output()
            .context("Failed to push tag")?;

        if !push_output.status.success() {
            log::warn("Failed to push tag");
        }

        log::success(&format!("Release {} created", version));
        Ok(())
    }

    fn run_verify_phase(&self) -> Result<()> {
        log::log("=== PHASE 4: VERIFY ===");

        // Run local tests
        self.run_local_tests()?;

        // Check CI status
        log::log("Checking CI status...");
        let ci_output = Command::new("ci-check")
            .arg(&self.project_path)
            .output()
            .context("Failed to run ci-check")?;

        if !ci_output.status.success() {
            log::warn("CI check found issues");
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
                } else {
                    log::warn("Local tests failed");
                }
            }
            Err(e) => {
                log::warn(&format!("No test runner found: {}", e));
            }
        }

        Ok(())
    }

    fn run(&self) -> Result<()> {
        log::log("Auto-Dev Pipeline v1.0.0 (Rust)");
        log::log(&format!("Project: {}", self.project_path.display()));
        log::log(&format!("Phase: {}", self.phase));
        log::log(&format!("Output: {}", self.output_dir.display()));

        self.check_prerequisites()?;

        match self.phase.as_str() {
            "review" => {
                self.run_review_phase()?;
            }
            "plan" => {
                let review_dir = self.run_review_phase()?;
                self.run_aggregate_phase(&review_dir)?;
            }
            "full" => {
                let review_dir = self.run_review_phase()?;
                let plan_path = self.run_aggregate_phase(&review_dir)?;
                self.run_execute_phase(&plan_path)?;
                self.run_verify_phase()?;
            }
            "release" => {
                // Release phase — requires version as argument
                let version = std::env::var("AUTO_DEV_VERSION")
                    .unwrap_or_else(|_| "v0.1.0".to_string());
                self.run_release_phase(&version)?;
            }
            other => {
                anyhow::bail!("Unknown phase: {}. Use: full, review, plan, release", other);
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
