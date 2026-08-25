use crate::Pipeline;
use anyhow::Result;
use auto_dev_pipeline::log;
use std::path::PathBuf;

impl Pipeline {
    /// Review phase: print delegate_task instructions for the orchestrating
    /// agent (one call per reviewer, sequential to avoid rate limits).
    pub(crate) fn run_review_phase(&self) -> Result<PathBuf> {
        log::log("=== PHASE 1: REVIEW ===");
        log::log("Reviews are performed by delegate_task subagents.");
        log::log("Run the following 4 delegate_task calls one at a time (sequential to avoid rate limits):");
        eprintln!();

        let _project_name = self
            .project_name
            .clone()
            .or_else(|| {
                self.project_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());

        let review_dir = auto_dev_pipeline::devnotes::paths(
            &self.dev_notes_root,
            self.project_name.as_deref().unwrap_or("unknown"),
        )
        .reviews
        .join(&self.timestamp);
        std::fs::create_dir_all(&review_dir)?;

        let reviewers = [
            (
                "code",
                "Code Reviewer: check logic, style, idioms, performance",
            ),
            (
                "security",
                "Security Reviewer: check vulnerabilities, unsafe code, secrets",
            ),
            (
                "architecture",
                "Architecture Reviewer: check structure, coupling, patterns",
            ),
            (
                "devops",
                "DevOps Reviewer: check CI/CD, dependencies, build, deploy",
            ),
        ];

        for (name, prompt) in &reviewers {
            let output_path = review_dir.join(format!("{}-review.md", name));
            eprintln!("--- {} Reviewer ---", name);
            eprintln!("delegate_task(");
            eprintln!("    goal=\"{}\",", prompt);
            eprintln!("    context=\"\"\"");
            eprintln!("    PROJECT_PATH: {}", self.project_path.display());
            eprintln!("    OUTPUT_PATH: {}", output_path.display());
            eprintln!("    \"\"\"");
            eprintln!("    toolsets=['file', 'search_files', 'terminal']");
            eprintln!(")");
            eprintln!();
        }

        log::success(&format!(
            "Review instructions generated. Output dir: {}",
            review_dir.display()
        ));
        Ok(review_dir)
    }
}
