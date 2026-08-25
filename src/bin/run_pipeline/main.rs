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
}
