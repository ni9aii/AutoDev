use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::{log, test_runner};

/// Resolve the `ci-check` companion binary.
///
/// Same rationale as `resolve_aggregator`: prefer the binary sitting next to
/// the running executable (works under `cargo test`/`target/`), fall back to
/// the bare name for `$PATH` installs.
fn resolve_ci_check() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("ci-check");
            if candidate.is_file() {
                return candidate.display().to_string();
            }
        }
    }
    "ci-check".to_string()
}

impl Pipeline {
    pub(crate) fn run_verify_phase(&self) -> Result<()> {
        log::log("=== PHASE 4: VERIFY ===");

        // Run local tests (fail-fast)
        self.run_local_tests()?;

        // Check CI status
        log::log("Checking CI status...");
        let project_path_str = self.project_path.display().to_string();
        let mut args: Vec<String> = vec![project_path_str];

        if self.hermes_mode {
            if let Some(ref project) = self.project_name {
                args.push("--project".to_string());
                args.push(project.clone());
            }
            args.push("--dev-notes".to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let ci_check = resolve_ci_check();
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
                log::warn(&format!("No test runner found: {}", e));
                anyhow::bail!("No test runner found in project: {}", e)
            }
        }
    }
}
