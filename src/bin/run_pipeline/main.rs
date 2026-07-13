use anyhow::{Context, Result};
use auto_dev_pipeline::log;
use auto_dev_pipeline::process::{ProcessRunner, SystemRunner};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

mod phases;

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
#[command(name = "run-pipeline", version = env!("CARGO_PKG_VERSION"), about = "Auto-Dev Pipeline", disable_version_flag = true)]
struct Args {
    /// Project path
    #[arg(default_value = ".")]
    project_path: PathBuf,

    /// Phase to run
    #[arg(value_enum, default_value = "full")]
    phase: Phase,

    /// Version tag for release (e.g., v0.2.0)
    #[arg(short = 'V', long = "release-version")]
    version: Option<String>,

    /// Hermes mode: use delegate_task instead of Claude CLI (default)
    #[arg(long, default_value = "true")]
    hermes_mode: bool,

    /// Project name for dev-notes path construction
    #[arg(long)]
    project: Option<String>,

    /// Root directory for dev-notes (overrides $DEV_NOTES_ROOT and ~/obsidian-vault/dev-notes default)
    #[arg(long)]
    dev_notes_root: Option<PathBuf>,

    /// Emit a machine-readable JSON summary instead of the human log tail
    #[arg(long, default_value = "false")]
    json: bool,
}

struct Pipeline {
    project_path: PathBuf,
    phase: Phase,
    version: Option<String>,
    hermes_mode: bool,
    project_name: Option<String>,
    timestamp: String,
    output_dir: PathBuf,
    dev_notes_root: PathBuf,
    json: bool,
    runner: Box<dyn ProcessRunner>,
}

impl Pipeline {
    fn new(args: Args) -> Result<Self> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();

        // Validate project_path: must exist and not contain path traversal
        let project_path = std::fs::canonicalize(&args.project_path)
            .with_context(|| format!("Invalid project path: {}", args.project_path.display()))?;

        let dev_notes_root =
            auto_dev_pipeline::git::paths::resolve_dev_notes_root(args.dev_notes_root.as_ref())?;

        let output_dir = if args.hermes_mode {
            let project = args
                .project
                .clone()
                .or_else(|| {
                    project_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            dev_notes_root
                .join(project)
                .join("reviews")
                .join(&timestamp)
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".hermes/plans/auto-dev")
        };

        std::fs::create_dir_all(&output_dir)?;

        Ok(Self {
            project_path,
            phase: args.phase,
            version: args.version,
            hermes_mode: args.hermes_mode,
            project_name: args.project,
            timestamp,
            output_dir,
            dev_notes_root,
            json: args.json,
            runner: Box::new(SystemRunner),
        })
    }

    fn check_prerequisites(&self) -> Result<()> {
        log::log("Checking prerequisites...");

        // Check git repo
        let git_dir = self.project_path.join(".git");
        if !git_dir.exists() {
            anyhow::bail!("Not a git repository: {}", self.project_path.display());
        }

        if !self.hermes_mode {
            // Check Claude Code CLI (legacy mode only). A present binary is not
            // enough — `claude --version` exits 0 even when the OAuth session is
            // expired, so we smoke-test an actual `-p` call to verify auth.
            self.check_claude_auth()?;
        } else {
            log::log("Hermes mode: skipping Claude Code CLI check");
        }

        log::success("Prerequisites OK");
        Ok(())
    }

    /// Verify the Claude Code CLI is both installed AND authenticated.
    ///
    /// `claude --version` returns success even with an expired OAuth session,
    /// which is exactly the failure mode reported in issue #1 ("Rust scripts
    /// not working" — legacy pipeline shells out to `claude -p` and it dies
    /// with "Failed to authenticate"). We perform a minimal `-p` call and
    /// inspect both the exit status and the output for auth errors.
    fn check_claude_auth(&self) -> Result<()> {
        log::log("Checking Claude Code CLI authentication...");

        let output = self.runner.run(
            "claude",
            &["-p", "reply with the single word: OK", "--max-turns", "1"],
            Some(&self.project_path),
        );

        match output {
            Err(e) => {
                log::error(&format!(
                    "Claude Code CLI not found or could not run: {}",
                    e
                ));
                log::error("Install: npm install -g @anthropic-ai/claude-code");
                log::error("Or use --hermes-mode for delegate_task-based execution (no Claude CLI needed).");
                anyhow::bail!("Claude Code CLI unavailable");
            }
            Ok(out) => {
                // Claude Code CLI prints auth errors to stdout (not stderr),
                // so inspect the combined output regardless of exit status.
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                )
                .to_lowercase();

                if combined.contains("failed to authenticate")
                    || combined.contains("oauth session expired")
                    || combined.contains("not authenticated")
                {
                    log::error("Claude Code CLI is installed but NOT authenticated.");
                    log::error("Re-authenticate with: claude (interactive login)");
                    log::error("Or use --hermes-mode for delegate_task-based execution (no Claude CLI needed).");
                    anyhow::bail!("Claude Code CLI authentication required");
                }

                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let detail = if !stderr.is_empty() { stderr } else { stdout };
                    log::error(&format!("Claude Code CLI exited with error: {}", detail));
                    anyhow::bail!("Claude Code CLI reported an error (see above)");
                }

                log::log("Claude Code CLI: authenticated");
                Ok(())
            }
        }
    }

    fn run(&self) -> Result<()> {
        log::log(&format!(
            "Auto-Dev Pipeline v{} (Rust)",
            env!("CARGO_PKG_VERSION")
        ));
        log::log(&format!("Project: {}", self.project_path.display()));
        log::log(&format!("Phase: {}", self.phase));
        log::log(&format!(
            "Mode: {}",
            if self.hermes_mode {
                "Hermes (delegate_task)"
            } else {
                "Legacy (Claude CLI)"
            }
        ));
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
                let version = self.version.as_ref().context(
                    "Release phase requires --release-version argument (e.g., --release-version v0.5.0)",
                )?;
                self.run_verify_phase()?;
                self.run_release_phase(version)?;
            }
        }

        if self.json {
            let summary = PipelineSummary {
                status: "success",
                version: env!("CARGO_PKG_VERSION"),
                project: self.project_path.display().to_string(),
                phase: self.phase.to_string(),
                mode: if self.hermes_mode { "hermes" } else { "legacy" },
                timestamp: &self.timestamp,
                output_dir: self.output_dir.display().to_string(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&summary).context("serialize JSON summary")?
            );
        } else {
            log::success("Pipeline complete!");
            log::log(&format!("Reports: {}", self.output_dir.display()));
        }

        Ok(())
    }
}

/// Machine-readable result of a pipeline run (emitted with `--json`).
#[derive(Debug, Serialize)]
struct PipelineSummary<'a> {
    status: &'a str,
    version: &'a str,
    project: String,
    phase: String,
    mode: &'a str,
    timestamp: &'a str,
    output_dir: String,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let pipeline = Pipeline::new(args)?;
    pipeline.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_dev_pipeline::process::{mock_output, MockRunner};
    use std::path::PathBuf as StdPathBuf;

    #[test]
    fn test_phase_display() {
        assert_eq!(Phase::Full.to_string(), "full");
        assert_eq!(Phase::Review.to_string(), "review");
        assert_eq!(Phase::Plan.to_string(), "plan");
        assert_eq!(Phase::Release.to_string(), "release");
    }

    #[test]
    fn test_validate_version_accepts_valid() {
        assert!(auto_dev_pipeline::validation::validate_version("v1.0.0").is_ok());
        assert!(auto_dev_pipeline::validation::validate_version("1.0.0").is_ok());
        assert!(auto_dev_pipeline::validation::validate_version("v2.0.0-alpha").is_ok());
    }

    #[test]
    fn test_validate_version_rejects_invalid() {
        assert!(auto_dev_pipeline::validation::validate_version("").is_err());
        assert!(auto_dev_pipeline::validation::validate_version("not-a-version").is_err());
        assert!(auto_dev_pipeline::validation::validate_version("1.0").is_err());
        assert!(auto_dev_pipeline::validation::validate_version("-v1.0.0").is_err());
    }

    #[test]
    fn test_check_claude_auth_passes_when_authenticated() {
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "OK", ""));
        let pipeline = Pipeline {
            project_path: StdPathBuf::from("."),
            phase: Phase::Full,
            version: None,
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            json: false,
            runner: Box::new(mock),
        };
        assert!(pipeline.check_claude_auth().is_ok());
    }

    #[test]
    fn test_check_claude_auth_fails_on_expired_oauth() {
        let mock = MockRunner::new();
        mock.push_response(mock_output(
            false,
            "",
            "Failed to authenticate: OAuth session expired",
        ));
        let pipeline = Pipeline {
            project_path: StdPathBuf::from("."),
            phase: Phase::Full,
            version: None,
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            json: false,
            runner: Box::new(mock),
        };
        assert!(pipeline.check_claude_auth().is_err());
    }

    #[test]
    fn test_check_claude_auth_fails_when_binary_missing() {
        let mock = MockRunner::new();
        mock.push_error("No such file or directory (os error 2)");
        let pipeline = Pipeline {
            project_path: StdPathBuf::from("."),
            phase: Phase::Full,
            version: None,
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            json: false,
            runner: Box::new(mock),
        };
        assert!(pipeline.check_claude_auth().is_err());
    }
}
