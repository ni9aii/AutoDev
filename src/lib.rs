pub mod log {
    use colored::Colorize;
    use std::sync::atomic::{AtomicBool, Ordering};

    static NO_COLOR: AtomicBool = AtomicBool::new(false);

    /// Disable colored output (respects NO_COLOR env convention).
    pub fn set_no_color(enabled: bool) {
        NO_COLOR.store(enabled, Ordering::Relaxed);
    }

    fn prefix(level: &str) -> String {
        let no_color = NO_COLOR.load(Ordering::Relaxed);
        if no_color {
            format!("[auto-dev] {}", level)
        } else {
            match level {
                "INFO" => format!("{} {}", "[auto-dev]".blue(), "INFO".blue()),
                "WARN" => format!("{} {}", "[auto-dev]".yellow(), "WARN".yellow()),
                "ERROR" => format!("{} {}", "[auto-dev]".red(), "ERROR".red()),
                "OK" => format!("{} {}", "[auto-dev]".green(), "OK".green()),
                _ => format!("[auto-dev] {}", level),
            }
        }
    }

    pub fn log(msg: &str) {
        println!("{} {}", prefix("INFO"), msg);
    }

    pub fn warn(msg: &str) {
        eprintln!("{} {}", prefix("WARN"), msg);
    }

    pub fn error(msg: &str) {
        eprintln!("{} {}", prefix("ERROR"), msg);
    }

    pub fn success(msg: &str) {
        println!("{} {}", prefix("OK"), msg);
    }
}

pub mod test_runner {
    use anyhow::{Context, Result};
    use std::path::Path;
    use std::process::Command;

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
    }

    pub fn detect_test_runner(project_path: &Path) -> Option<TestRunner> {
        if project_path.join("Cargo.toml").exists() {
            Some(TestRunner::Cargo)
        } else if project_path.join("Makefile").exists() {
            Some(TestRunner::Make)
        } else if project_path.join("package.json").exists() {
            Some(TestRunner::Npm)
        } else if project_path.join("pyproject.toml").exists()
            || project_path.join("setup.py").exists()
        {
            Some(TestRunner::Pytest)
        } else {
            None
        }
    }

    pub struct TestResult {
        pub runner: TestRunner,
        pub success: bool,
        pub stdout: String,
        pub stderr: String,
    }

    pub fn run_local_tests(project_path: &Path) -> Result<TestResult> {
        let runner = detect_test_runner(project_path)
            .context("No test runner detected (Makefile, package.json, pyproject.toml, setup.py)")?;

        let output = match runner {
            TestRunner::Cargo => Command::new("cargo")
                .arg("test")
                .current_dir(project_path)
                .output()
                .context("Failed to run 'cargo test'")?,
            TestRunner::Make => Command::new("make")
                .arg("test")
                .current_dir(project_path)
                .output()
                .context("Failed to run 'make test'")?,
            TestRunner::Npm => Command::new("npm")
                .arg("test")
                .current_dir(project_path)
                .output()
                .context("Failed to run 'npm test'")?,
            TestRunner::Pytest => Command::new("pytest")
                .current_dir(project_path)
                .output()
                .context("Failed to run 'pytest'")?,
        };

        Ok(TestResult {
            runner,
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub mod git {
    use anyhow::{Context, Result};
    use once_cell::sync::Lazy;
    use regex::Regex;
    use std::path::Path;
    use std::process::Command;

    pub mod paths {
        use anyhow::{Context, Result};
        use std::path::Path;

        /// Validate and canonicalize a project path
        /// Returns Ok(canonical_path) or Err with descriptive message
        pub fn validate_project_path(path: &Path) -> Result<std::path::PathBuf> {
            let canonical = std::fs::canonicalize(path)
                .with_context(|| format!("Invalid project path: {}", path.display()))?;

            if !canonical.join(".git").exists() && !canonical.join("Cargo.toml").exists() {
                anyhow::bail!("Not a project directory (missing .git or Cargo.toml): {}", canonical.display());
            }

            Ok(canonical)
        }
    }

    static GITHUB_REMOTE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?$")
            .expect("Invalid GITHUB_REMOTE_RE pattern")
    });

    pub fn get_repo_info(project_path: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(project_path)
            .output()
            .context("Failed to get git remote")?;

        if !output.status.success() {
            anyhow::bail!("No git remote 'origin' found");
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if let Some(caps) = GITHUB_REMOTE_RE.captures(&remote_url) {
            let owner = &caps[1];
            let repo = &caps[2];
            Ok(format!("{}/{}", owner, repo))
        } else {
            anyhow::bail!("Not a GitHub repository: {}", remote_url)
        }
    }
}

pub mod validation {
    /// Validate a version string for use in git tags and JSON payloads.
    /// Only allows semver-like strings: starts with optional 'v', then digits and dots.
    pub fn validate_version(version: &str) -> Result<(), String> {
        let clean = version.trim();
        if clean.is_empty() {
            return Err("Version string is empty".to_string());
        }
        // Allow v-prefixed semver: v0.1.0, 1.0.0, v2.0.0-alpha
        let re = regex::Regex::new(r"^v?\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$")
            .map_err(|e| format!("Failed to compile version regex: {}", e))?;
        if !re.is_match(clean) {
            return Err(format!(
                "Invalid version '{}'. Expected semver format: v0.1.0 or 1.0.0",
                clean
            ));
        }
        Ok(())
    }
}

pub mod markdown {
    pub fn extract_section(content: &str, section_name: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut in_section = false;

        for line in &lines {
            let trimmed = line.trim();

            if trimmed.starts_with("## ") {
                let heading = trimmed.trim_start_matches("## ").trim();
                if heading.contains(section_name) {
                    in_section = true;
                    result.push(line.to_string());
                    continue;
                } else if in_section {
                    break;
                }
            }

            if in_section {
                result.push(line.to_string());
            }
        }

        result.join("\n")
    }

    /// Truncate string safely at char boundary to avoid UTF-8 panic.
    pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
        if s.len() <= max_chars {
            return s;
        }
        let mut boundary = max_chars;
        while boundary > 0 && !s.is_char_boundary(boundary) {
            boundary -= 1;
        }
        &s[..boundary]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_section_found() {
        let content = "# Plan\n\n## Do Now\n- Fix 1\n- Fix 2\n\n## Defer\n- Fix 3";
        let section = markdown::extract_section(content, "Do Now");
        assert!(section.contains("Fix 1"));
        assert!(section.contains("Fix 2"));
        assert!(!section.contains("Fix 3"));
    }

    #[test]
    fn test_extract_section_not_found() {
        let content = "# Plan\n\n## Other\n- Something";
        let section = markdown::extract_section(content, "Do Now");
        assert!(section.is_empty());
    }

    #[test]
    fn test_log_functions() {
        log::log("test message");
        log::warn("test warning");
        log::error("test error");
        log::success("test success");
    }

    #[test]
    fn test_safe_truncate_ascii() {
        assert_eq!(markdown::safe_truncate("hello world", 5), "hello");
    }

    #[test]
    fn test_safe_truncate_multibyte() {
        // Russian: each char is 2 bytes
        let s = "привет";
        let truncated = markdown::safe_truncate(s, 5);
        assert!(truncated.len() <= 5);
        assert!(s.starts_with(truncated));
    }

    #[test]
    fn test_validate_version_valid() {
        assert!(validation::validate_version("v0.1.0").is_ok());
        assert!(validation::validate_version("1.0.0").is_ok());
        assert!(validation::validate_version("v2.0.0-alpha").is_ok());
    }

    #[test]
    fn test_validate_version_invalid() {
        assert!(validation::validate_version("").is_err());
        assert!(validation::validate_version("; rm -rf /").is_err());
        assert!(validation::validate_version("$(whoami)").is_err());
    }
}
