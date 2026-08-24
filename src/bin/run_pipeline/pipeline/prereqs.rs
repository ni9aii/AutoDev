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
    pub(crate) fn check_claude_auth(&self) -> Result<()> {
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
