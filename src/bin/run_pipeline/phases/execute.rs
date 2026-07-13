use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::{log, markdown};
use shlex::try_quote;
use std::path::PathBuf;

/// Individual fix parsed from Do Now section
pub(crate) struct Fix {
    pub(crate) title: String,
    pub(crate) severity: String,
    pub(crate) file: Option<String>,
    pub(crate) description: String,
}

impl Pipeline {
    pub(crate) fn run_execute_phase(&self, plan_path: &PathBuf) -> Result<()> {
        if self.hermes_mode {
            self.run_execute_phase_hermes(plan_path)
        } else {
            self.run_execute_phase_legacy(plan_path)
        }
    }

    /// Hermes mode: print fix instructions instead of calling Claude CLI
    fn run_execute_phase_hermes(&self, plan_path: &PathBuf) -> Result<()> {
        log::log("=== PHASE 3: EXECUTE (Hermes Mode) ===");

        let plan_content = std::fs::read_to_string(plan_path)
            .context("Failed to read plan file")?;

        let do_now_section = markdown::extract_section(&plan_content, "Do Now");
        if do_now_section.is_empty() {
            log::warn("No Do Now fixes found in plan");
            return Ok(());
        }

        log::log(&format!("Found Do Now section ({} chars)", do_now_section.len()));

        let fixes = self.parse_fixes(&do_now_section);
        log::log(&format!("Parsed {} fixes to execute", fixes.len()));

        if fixes.is_empty() {
            log::warn("No actionable fixes found");
            return Ok(());
        }

        println!();
        println!("=== Hermes Execute Instructions ===");
        println!("For each fix below, use delegate_task (complex) or patch (simple):");
        println!();

        for (i, fix) in fixes.iter().enumerate() {
            println!("--- Fix {}: {} ---", i + 1, fix.title);
            println!("Severity: {}", fix.severity);
            if let Some(ref file) = fix.file {
                println!("File: {}", file);
            }
            println!();
            println!("Option A - Simple fix (≤2 files, ≤20 lines):");
            println!("  read_file(path=\"...\")");
            println!("  patch(path=\"...\", old_string=\"...\", new_string=\"...\")");
            println!();
            println!("Option B - Complex fix:");
            println!("  delegate_task(");
            println!("      goal=\"Fix: {}\",", fix.title);
            println!("      context=\"\"\"");
            println!("      PROJECT_PATH: {}", self.project_path.display());
            if let Some(ref file) = fix.file {
                println!("      FILE: {}", file);
            }
            println!("      DESCRIPTION: {}", fix.description.trim());
            println!("      \"\"\",");
            println!("      toolsets=['file', 'patch', 'terminal']");
            println!("  )");
            println!();
        }

        log::success("Execution instructions generated");
        Ok(())
    }

    /// Legacy mode: execute fixes via Claude Code CLI
    fn run_execute_phase_legacy(&self, plan_path: &PathBuf) -> Result<()> {

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
                let rest = trimmed.trim_start_matches("### Fix ").trim();
                let title = match rest.split_once(':') {
                    Some((_, title)) if !title.trim().is_empty() => title.trim().to_string(),
                    _ => rest.to_string(),
                };
                current_fix = Some(Fix {
                    title,
                    severity: "UNKNOWN".to_string(),
                    file: None,
                    description: String::new(),
                });
            } else if let Some(ref mut fix) = current_fix {
                let label = trimmed
                    .trim_matches(|c: char| c == '*' || c == ':' || c.is_whitespace())
                    .to_lowercase();
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
                } else if label == "description" {
                    // Skip the label itself, next lines go to description
                } else if !trimmed.is_empty() {
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
        let output = self
            .runner
            .run(
                "claude",
                &[
                    "-p",
                    task,
                    "--allowedTools",
                    "Read,Edit,Bash",
                    "--max-turns",
                    "15",
                ],
                Some(&self.project_path),
            )
            .context("Failed to run Claude Code")?;

        if !output.status.success() {
            log::warn("Claude Code exited with non-zero status");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        print!("{}", stdout);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_dev_pipeline::process::{MockRunner, SystemRunner};
    use std::path::PathBuf as StdPathBuf;

    fn test_pipeline() -> Pipeline {
        Pipeline {
            project_path: StdPathBuf::from("."),
            phase: crate::Phase::Full,
            version: None,
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            runner: Box::new(SystemRunner),
        }
    }

    #[test]
    fn test_parse_fixes_well_formed() {
        let pipeline = test_pipeline();
        let input = "### Fix 1: Improve error handling\n\
            **Severity:** CRITICAL\n\
            **File:** `src/lib.rs`\n\
            **Description:**\n\
            Errors are swallowed silently.\n";
        let fixes = pipeline.parse_fixes(input);
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].title, "Improve error handling");
        assert_eq!(fixes[0].severity, "CRITICAL");
        assert_eq!(fixes[0].file.as_deref(), Some("src/lib.rs"));
        assert!(fixes[0].description.contains("swallowed silently"));
    }

    #[test]
    fn test_parse_fixes_title_without_colon() {
        let pipeline = test_pipeline();
        let input = "### Fix Improve error handling\n\
            **Severity:** MINOR\n";
        let fixes = pipeline.parse_fixes(input);
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].title, "Improve error handling");
    }

    #[test]
    fn test_parse_fixes_description_label_trailing_whitespace() {
        let pipeline = test_pipeline();
        let input = "### Fix 1: Title\n\
            **Description:**   \n\
            Some detail here.\n";
        let fixes = pipeline.parse_fixes(input);
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].description.contains("Some detail here."));
        assert!(!fixes[0].description.contains("**Description:**"));
    }

    #[test]
    fn test_parse_fixes_missing_severity_and_file() {
        let pipeline = test_pipeline();
        let input = "### Fix 1: Title only\n\
            Just a description line.\n";
        let fixes = pipeline.parse_fixes(input);
        assert_eq!(fixes.len(), 1);
        assert_eq!(fixes[0].severity, "UNKNOWN");
        assert_eq!(fixes[0].file, None);
        assert!(fixes[0].description.contains("Just a description line."));
    }

    #[test]
    fn test_execute_via_claude_uses_mock_runner() {
        let mock = MockRunner::new();
        mock.push_response(auto_dev_pipeline::process::mock_output(true, "ok", ""));
        let pipeline = Pipeline {
            project_path: StdPathBuf::from("."),
            phase: crate::Phase::Full,
            version: None,
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            runner: Box::new(mock),
        };
        pipeline.execute_via_claude("do the thing").unwrap();
    }
}
