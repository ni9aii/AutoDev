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

#[cfg(test)]
mod tests {
    use auto_dev_pipeline::process::{mock_output, MockRunner};
    use std::path::PathBuf as StdPathBuf;

    fn pipeline_with(mock: MockRunner) -> crate::Pipeline {
        crate::Pipeline {
            project_path: StdPathBuf::from("."),
            phase: crate::Phase::Release,
            version: Some("v1.2.3".to_string()),
            hermes_mode: false,
            project_name: None,
            timestamp: "20260101_000000".to_string(),
            output_dir: StdPathBuf::from("."),
            dev_notes_root: StdPathBuf::from("."),
            json: false,
            runner: Box::new(mock),
        }
    }

    #[test]
    fn test_release_phase_rejects_bad_version() {
        // Validation happens before any process call, so an empty MockRunner is fine.
        let p = pipeline_with(MockRunner::new());
        let res = p.run_release_phase("; rm -rf /");
        assert!(res.is_err(), "malicious version must be rejected");
        assert!(res.unwrap_err().to_string().contains("Invalid version"));
    }

    #[test]
    fn test_release_phase_bails_when_build_fails() {
        // First runner call is `cargo build --release`; simulate a compile failure.
        let mock = MockRunner::new();
        mock.push_response(mock_output(false, "", "error[E0001]: boom"));
        let p = pipeline_with(mock);
        let res = p.run_release_phase("v1.2.3");
        assert!(res.is_err(), "build failure must abort the release");
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Release build failed"));
    }

    #[test]
    fn test_release_phase_bails_when_tag_fails() {
        // build ok, then `git tag` fails -> must bail before push.
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "compiled", ""));
        mock.push_response(mock_output(false, "", "fatal: tag 'v1.2.3' already exists"));
        let p = pipeline_with(mock);
        let res = p.run_release_phase("v1.2.3");
        assert!(res.is_err(), "tag failure must abort the release");
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("Failed to create tag"));
    }

    // NOTE: the full happy path (build -> tag -> push -> GitHub API) is not
    // covered here because the release step makes a real blocking reqwest call
    // to api.github.com and requires GITHUB_TOKEN. Making that hermetic needs a
    // network-abstraction refactor, which is intentionally out of scope for
    // v0.5.0 (see plan Part C exclusions).
}
