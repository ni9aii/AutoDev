//! CI status markdown report writer (dev-notes integration).

use super::CiChecker;
use anyhow::Result;
use auto_dev_pipeline::{git, log};
use std::fs;

impl CiChecker {
    pub(crate) fn save_dev_notes_report(
        &self,
        project: &str,
        ci_passed: bool,
        local_passed: bool,
        root: &std::path::Path,
    ) -> Result<()> {
        let reports_dir = {
            auto_dev_pipeline::validation::validate_project_name(project)
                .map_err(|e| anyhow::anyhow!(e))?;
            auto_dev_pipeline::devnotes::paths(root, project).ci_reports
        };
        fs::create_dir_all(&reports_dir)?;

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let report_path = reports_dir.join(format!("{}-ci-status.md", timestamp));

        let status_icon = |passed: bool| if passed { "✅" } else { "❌" };

        let content = format!(
            "# CI Status Report\n\n\
            **Project:** {}\n\
            **Timestamp:** {}\n\
            **Repository:** {}\n\n\
            ## Results\n\n\
            | Check | Status |\n\
            |-------|--------|\n\
            | GitHub Actions CI | {} |\n\
            | Local Tests | {} |\n\n\
            ## Overall\n\n\
            {}\n",
            project,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            git::get_repo_info(&self.project_path, self.runner.as_ref())
                .unwrap_or_else(|_| "unknown".to_string()),
            status_icon(ci_passed),
            status_icon(local_passed),
            if ci_passed && local_passed {
                "✅ All checks passed"
            } else {
                "❌ Some checks failed"
            }
        );

        fs::write(&report_path, content)?;
        log::log(&format!("CI report saved: {}", report_path.display()));
        Ok(())
    }
}
