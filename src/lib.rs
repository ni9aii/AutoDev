pub mod log {
    use colored::Colorize;

    pub fn log(msg: &str) {
        println!("{} {}", "[auto-dev]".blue(), msg);
    }

    pub fn warn(msg: &str) {
        println!("{} {}", "[auto-dev]".yellow(), msg);
    }

    pub fn error(msg: &str) {
        println!("{} {}", "[auto-dev]".red(), msg);
    }

    pub fn success(msg: &str) {
        println!("{} {}", "[auto-dev]".green(), msg);
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
    }

    impl TestRunner {
        pub fn name(&self) -> &'static str {
            match self {
                TestRunner::Make => "make test",
                TestRunner::Npm => "npm test",
                TestRunner::Pytest => "pytest",
            }
        }
    }

    pub fn detect_test_runner(project_path: &Path) -> Option<TestRunner> {
        if project_path.join("Makefile").exists() {
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
    use regex::Regex;
    use std::path::Path;
    use std::process::Command;

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

        let re = Regex::new(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?$")?;
        if let Some(caps) = re.captures(&remote_url) {
            let owner = &caps[1];
            let repo = &caps[2];
            Ok(format!("{}/{}", owner, repo))
        } else {
            anyhow::bail!("Not a GitHub repository: {}", remote_url)
        }
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
}
