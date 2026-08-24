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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::MockRunner;

    #[test]
    fn test_run_local_tests_no_runner_is_none() {
        let td = std::env::temp_dir().join(format!("autodev-norunner-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&td);
        let runner = MockRunner::new();
        // Empty dir: no Makefile/Cargo.toml/package.json/pyproject.toml/setup.py.
        let res = run_local_tests(&td, &runner);
        assert!(
            matches!(res, Ok(None)),
            "expected Ok(None) when no runner, got {:?}",
            res
        );
        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn test_run_local_tests_unavailable_command_is_none() {
        let td = std::env::temp_dir().join(format!("autodev-makefail-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&td);
        std::fs::write(td.join("Makefile"), "test:\n\t@echo ok\n").unwrap();
        let runner = MockRunner::new();
        // Makefile present -> Make detected, but the command can't launch -> None (skip).
        runner.push_error("make: command not found");
        let res = run_local_tests(&td, &runner);
        assert!(
            matches!(res, Ok(None)),
            "unavailable runner must be Ok(None), got {:?}",
            res
        );
        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn test_run_local_tests_success_is_some() {
        let td = std::env::temp_dir().join(format!("autodev-makeok-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&td);
        std::fs::write(td.join("Makefile"), "test:\n\t@echo ok\n").unwrap();
        let runner = MockRunner::new();
        runner.push_response(crate::process::mock_output(true, "ok", ""));
        let res = run_local_tests(&td, &runner);
        match res {
            Ok(Some(r)) => assert!(r.success, "expected success"),
            other => panic!("expected Ok(Some), got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&td);
    }
}
