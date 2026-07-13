use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::log;
use shlex::try_quote;
use std::path::PathBuf;

impl Pipeline {
    pub(crate) fn run_review_phase(&self) -> Result<PathBuf> {
        if self.hermes_mode {
            self.run_review_phase_hermes()
        } else {
            self.run_review_phase_legacy()
        }
    }

    /// Hermes mode: print delegate_task instructions instead of calling Claude CLI
    fn run_review_phase_hermes(&self) -> Result<PathBuf> {
        log::log("=== PHASE 1: REVIEW (Hermes Mode) ===");
        log::log("In Hermes mode, reviews are performed by delegate_task subagents.");
        log::log("Run the following 4 delegate_task calls one at a time (sequential to avoid rate limits):");
        println!();

        let _project_name = self.project_name.clone()
            .or_else(|| self.project_path.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string()))
            .unwrap_or_else(|| "unknown".to_string());

        let review_dir = self.output_dir.clone();

        let reviewers = [
            ("code", "Code Reviewer: check logic, style, idioms, performance"),
            ("security", "Security Reviewer: check vulnerabilities, unsafe code, secrets"),
            ("architecture", "Architecture Reviewer: check structure, coupling, patterns"),
            ("devops", "DevOps Reviewer: check CI/CD, dependencies, build, deploy"),
        ];

        for (name, prompt) in &reviewers {
            let output_path = review_dir.join(format!("{}-review.md", name));
            println!("--- {} Reviewer ---", name);
            println!("delegate_task(");
            println!("    goal=\"{}\",", prompt);
            println!("    context=\"\"\"");
            println!("    PROJECT_PATH: {}", self.project_path.display());
            println!("    OUTPUT_PATH: {}", output_path.display());
            println!("    \"\"\",");
            println!("    toolsets=['file', 'search_files', 'terminal']");
            println!(")");
            println!();
        }

        log::success(&format!("Review instructions generated. Output dir: {}", review_dir.display()));
        Ok(review_dir)
    }

    /// Legacy mode: launch reviewers via Claude Code CLI
    fn run_review_phase_legacy(&self) -> Result<PathBuf> {

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

            let output = self
                .runner
                .run(
                    "claude",
                    &[
                        "-p",
                        &review_prompt,
                        "--allowedTools",
                        "Read,Edit,Bash",
                        "--max-turns",
                        "30",
                    ],
                    Some(&self.project_path),
                )
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
}
