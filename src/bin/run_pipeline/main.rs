use anyhow::Result;
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

    /// Hermes mode: reviews are performed by delegate_task subagents (default).
    /// Legacy mode shells out to the Claude Code CLI; opt in with
    /// `--legacy-claude` (plan finding: `--hermes-mode` with SetTrue +
    /// default=true could never be disabled, leaving legacy code unreachable).
    #[arg(long = "legacy-claude", default_value = "false")]
    hermes_mode: bool, // true when NOT --legacy-claude

    /// Project name for dev-notes path construction
    #[arg(long)]
    project: Option<String>,

    /// Root directory for dev-notes (overrides $DEV_NOTES_ROOT and ~/Notes/dev-notes default)
    #[arg(long)]
    dev_notes_root: Option<PathBuf>,

    /// Emit a machine-readable JSON summary instead of the human log tail
    #[arg(long, default_value = "false")]
    json: bool,
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
