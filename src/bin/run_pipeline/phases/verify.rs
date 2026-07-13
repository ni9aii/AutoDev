use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::{log, test_runner};

impl Pipeline {
    pub(crate) fn run_verify_phase(&self) -> Result<()> {
        log::log("=== PHASE 4: VERIFY ===");

        // Run local tests (non-fatal: a missing runner is only a warning)
        self.run_local_tests()?;

        // Check CI status
        log::log("Checking CI status...");
        let req = auto_dev_pipeline::bin_contract::CiCheckRequest {
            project_path: self.project_path.clone(),
            project: if self.hermes_mode {
                self.project_name.clone()
            } else {
                None
            },
            dev_notes: self.hermes_mode,
        };
        let args = req.to_args();
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let ci_check = auto_dev_pipeline::bin_contract::resolve_companion(
            auto_dev_pipeline::bin_contract::CI_CHECK,
        );
        let ci_output = self
            .runner
            .run(&ci_check, &arg_refs, None)
            .context("Failed to run ci-check")?;

        if !ci_output.status.success() {
            let stderr = String::from_utf8_lossy(&ci_output.stderr);
            anyhow::bail!("CI check failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&ci_output.stdout);
        print!("{}", stdout);

        log::success("Verification complete");
        Ok(())
    }

    fn run_local_tests(&self) -> Result<()> {
        log::log("Checking local test status...");

        match test_runner::run_local_tests(&self.project_path, self.runner.as_ref()) {
            Ok(result) => {
                log::log(&format!("Running: {}", result.runner.name()));
                if result.success {
                    log::success("Local tests passed");
                    Ok(())
                } else {
                    let stderr_preview =
                        auto_dev_pipeline::markdown::safe_truncate(&result.stderr, 200);
                    anyhow::bail!(
                        "Local tests failed ({}):\nstdout: {}\nstderr: {}...",
                        result.runner.name(),
                        result.stdout,
                        stderr_preview
                    )
                }
            }
            Err(e) => {
                // No test runner available (e.g. `make` absent on Windows CI
                // runners) — this is non-fatal: the verify phase should not
                // fail solely because a local test runner isn't installed.
                log::warn(&format!(
                    "Skipping local test runner (not available): {}",
                    e
                ));
                Ok(())
            }
        }
    }
}
