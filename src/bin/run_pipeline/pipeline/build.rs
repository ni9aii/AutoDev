use anyhow::{Context, Result};
use auto_dev_pipeline::process::{ProcessRunner, SystemRunner};
use std::path::{Path, PathBuf};

use crate::Args;
use crate::Phase;

/// Resolve the effective project name from an explicit `--project` value or
/// the project directory name, and validate it before it is ever used as a
/// path component. This runs in **all** run-pipeline modes: hermes mode joins
/// the name onto the dev-notes root (`<root>/<project>/plans/…`) just like
/// the legacy branch joins it onto `<root>/<project>/reviews/…`, so skipping
/// validation there allowed `--project ../escape` path traversal.
pub(crate) fn resolve_project_name(project: Option<String>, project_path: &Path) -> Result<String> {
    let name = project
        .or_else(|| {
            project_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    auto_dev_pipeline::validation::validate_project_name(&name).map_err(|e| anyhow::anyhow!(e))?;
    Ok(name)
}

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
        // AUTO_DEV_TIMESTAMP pins the run timestamp (tests use it to locate
        // hermes-mode review dirs deterministically; also useful for
        // reproducing a specific run).
        let timestamp = std::env::var("AUTO_DEV_TIMESTAMP")
            .unwrap_or_else(|_| chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());

        // Validate project_path: must exist and not contain path traversal
        let project_path = std::fs::canonicalize(&args.project_path)
            .with_context(|| format!("Invalid project path: {}", args.project_path.display()))?;

        let dev_notes_root =
            auto_dev_pipeline::git::paths::resolve_dev_notes_root(args.dev_notes_root.as_ref())?;

        // Validate the project name unconditionally (see resolve_project_name):
        // both modes join it onto the dev-notes root.
        let project_name = resolve_project_name(args.project.clone(), &project_path)?;

        let output_dir = if !args.hermes_mode {
            dev_notes_root
                .join(&project_name)
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
            // CLI flag is --legacy-claude (opt-in); hermes_mode is its negation.
            hermes_mode: !args.hermes_mode,
            project_name: Some(project_name),
            timestamp,
            output_dir,
            dev_notes_root,
            json: args.json,
            runner: Box::new(SystemRunner),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal_project_name() {
        let dir = test_dir("traversal");
        assert!(resolve_project_name(Some("../escape".to_string()), &dir).is_err());
        assert!(resolve_project_name(Some("..\\escape".to_string()), &dir).is_err());
        assert!(resolve_project_name(Some("foo/bar".to_string()), &dir).is_err());
    }

    #[test]
    fn accepts_valid_explicit_and_derived_names() {
        let dir = test_dir("valid");
        assert_eq!(
            resolve_project_name(Some("my-project_1".to_string()), &dir)
                .expect("valid explicit name"),
            "my-project_1"
        );
        // Derived from the directory name when --project is omitted.
        assert_eq!(
            resolve_project_name(None, &dir).expect("derived name"),
            dir.file_name().and_then(|n| n.to_str()).unwrap()
        );
    }

    /// Unique scratch directory per test process (no extra dev-dependency).
    fn test_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-build-test-{}-{}-{}",
            label,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }
}
