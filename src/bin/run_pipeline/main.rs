use anyhow::Result;
use auto_dev_pipeline::log;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

mod phases;
mod pipeline;

use crate::pipeline::build::Pipeline;

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

impl Pipeline {
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
                log::error(
                "Or use --hermes-mode for delegate_task-based execution (no Claude CLI needed).",
            );
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
}

fn main() -> Result<()> {
    let args = Args::parse();
    let pipeline = Pipeline::new(args)?;
    crate::pipeline::dispatch::run(&pipeline)?;
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
