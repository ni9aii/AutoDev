//! Local test-runner detection and execution.

use crate::process::ProcessRunner;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRunner {
    Make,
    Npm,
    Pytest,
    Cargo,
}

impl TestRunner {
    pub fn name(&self) -> &'static str {
        match self {
            TestRunner::Make => "make test",
            TestRunner::Npm => "npm test",
            TestRunner::Pytest => "pytest",
            TestRunner::Cargo => "cargo test",
        }
    }

    fn program(&self) -> &'static str {
        match self {
            TestRunner::Make => "make",
            TestRunner::Npm => "npm",
            TestRunner::Pytest => "pytest",
            TestRunner::Cargo => "cargo",
        }
    }

    fn args(&self) -> &'static [&'static str] {
        match self {
            TestRunner::Make => &["test"],
            TestRunner::Npm => &["test"],
            TestRunner::Pytest => &[],
            TestRunner::Cargo => &["test"],
        }
    }
}

pub fn detect_test_runner(project_path: &Path) -> Option<TestRunner> {
    if project_path.join("Cargo.toml").exists() {
        Some(TestRunner::Cargo)
    } else if project_path.join("Makefile").exists() {
        Some(TestRunner::Make)
    } else if project_path.join("package.json").exists() {
        Some(TestRunner::Npm)
    } else if project_path.join("pyproject.toml").exists() || project_path.join("setup.py").exists()
    {
        Some(TestRunner::Pytest)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub runner: TestRunner,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run the project's detected local test runner.
///
/// Returns `Ok(Some(result))` when a runner was found and executed,
/// `Ok(None)` when no runner is configured *or* the configured command is
/// not available (e.g. `make` absent on a Windows CI runner) — callers
/// should treat `None` as a non-fatal skip, not a failure. `Err` is reserved
/// for unexpected internal errors.
pub fn run_local_tests(
    project_path: &Path,
    runner: &dyn ProcessRunner,
) -> Result<Option<TestResult>> {
    let test_runner = match detect_test_runner(project_path) {
        Some(tr) => tr,
        None => return Ok(None),
    };

    let output = match runner.run(
        test_runner.program(),
        test_runner.args(),
        Some(project_path),
    ) {
        Ok(o) => o,
        Err(e) => {
            // Runner is configured but the command can't be launched
            // (e.g. `make` not installed). Non-fatal: skip, don't fail.
            crate::log::warn(&format!(
                "Test runner '{}' not available, skipping: {}",
                test_runner.name(),
                e
            ));
            return Ok(None);
        }
    };

    Ok(Some(TestResult {
        runner: test_runner,
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }))
}
