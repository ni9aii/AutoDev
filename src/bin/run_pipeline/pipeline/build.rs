use anyhow::{Context, Result};
use auto_dev_pipeline::process::{ProcessRunner, SystemRunner};
use std::path::PathBuf;

use crate::Args;
use crate::Phase;

/// Auto-Dev pipeline state. Built once from parsed CLI `Args`; holds the
/// resolved project paths, run mode, and the process runner used for every
/// spawned step (a `SystemRunner` in production, a `MockRunner` in tests).
pub(crate) struct Pipeline {
    pub(crate) project_path: PathBuf,
    pub(crate) phase: Phase,
    pub(crate) version: Option<String>,
    pub(crate) hermes_mode: bool,
    pub(crate) project_name: Option<String>,
    pub(crate) timestamp: String,
    pub(crate) output_dir: PathBuf,
    pub(crate) dev_notes_root: PathBuf,
    pub(crate) json: bool,
    pub(crate) runner: Box<dyn ProcessRunner>,
}

impl Pipeline {
    pub(crate) fn new(args: Args) -> Result<Self> {
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
            auto_dev_pipeline::validation::validate_project_name(&project)
                .map_err(|e| anyhow::anyhow!(e))?;
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
}
