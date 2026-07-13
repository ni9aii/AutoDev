use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::{git, log};

impl Pipeline {
    pub(crate) fn run_release_phase(&self, version: &str) -> Result<()> {
        log::log("=== PHASE 5: RELEASE ===");

        // Validate version string (prevent injection)
        auto_dev_pipeline::validation::validate_version(version)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Build release binary
        log::log("Building release binary...");
        let build_output = self
            .runner
            .run("cargo", &["build", "--release"], Some(&self.project_path))
            .context("Failed to build release binary")?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            anyhow::bail!("Release build failed: {}", stderr);
        }
        log::success("Release build complete");

        // Create git tag
        log::log(&format!("Creating tag: {}", version));
        let tag_message = format!("Release {}", version);
        let tag_output = self
            .runner
            .run(
                "git",
                &["tag", "-a", version, "-m", &tag_message],
                Some(&self.project_path),
            )
            .context("Failed to create git tag")?;

        if !tag_output.status.success() {
            let stderr = String::from_utf8_lossy(&tag_output.stderr);
            anyhow::bail!("Failed to create tag: {}", stderr);
        }
        log::success(&format!("Tag {} created", version));

        // Push tag
        log::log("Pushing tag...");
        let push_output = self
            .runner
            .run(
                "git",
                &["push", "origin", version],
                Some(&self.project_path),
            )
            .context("Failed to push tag")?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            anyhow::bail!("Failed to push tag: {}", stderr);
        }
        log::success("Tag pushed to origin");

        // Create GitHub Release via API (reqwest — token stays in process memory)
        log::log("Creating GitHub Release...");
        let repo = git::get_repo_info(&self.project_path, self.runner.as_ref())?;
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GITHUB_PAT"))
            .context("GITHUB_TOKEN or GITHUB_PAT must be set")?;

        let release_url = url::Url::parse("https://api.github.com/repos/")
            .and_then(|base| base.join(&format!("{}/releases/", repo)))
            .context("Failed to build GitHub release URL")?;
        let release_body = format!(
            "{{\"tag_name\":\"{}\",\"name\":\"Release {}\",\"body\":\"Auto-generated release\",\"draft\":false,\"prerelease\":false}}",
            version, version
        );

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;
        let response = client
            .post(release_url)
            .header("Accept", "application/vnd.github+json")
            .bearer_auth(token)
            .header(
                "User-Agent",
                format!("auto-dev-pipeline/{}", env!("CARGO_PKG_VERSION")),
            )
            .body(release_body)
            .send()
            .context("Failed to create GitHub release")?;

        if response.status().is_success() {
            log::success(&format!("GitHub Release {} created", version));
        } else {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            log::warn(&format!(
                "GitHub release creation failed ({}): {}",
                status, body
            ));
        }

        log::success(&format!("Release {} complete", version));
        Ok(())
    }
}
