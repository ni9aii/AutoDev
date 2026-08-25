use anyhow::Result;
use auto_dev_pipeline::log;

use crate::pipeline::build::Pipeline;

impl Pipeline {
    pub(crate) fn check_prerequisites(&self) -> Result<()> {
        log::log("Checking prerequisites...");

        // Check git repo
        let git_dir = self.project_path.join(".git");
        if !git_dir.exists() {
            anyhow::bail!("Not a git repository: {}", self.project_path.display());
        }

        log::success("Prerequisites OK");
        Ok(())
    }
}
