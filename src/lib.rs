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
        eprintln!("{} {}", prefix("INFO"), msg);
    }

    pub fn warn(msg: &str) {
        eprintln!("{} {}", prefix("WARN"), msg);
    }

    pub fn error(msg: &str) {
        eprintln!("{} {}", prefix("ERROR"), msg);
    }

    pub fn success(msg: &str) {
        eprintln!("{} {}", prefix("OK"), msg);
    }
}

pub mod process {
    use anyhow::{Context, Result};
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitStatus, Output};

    /// Resolve an executable name to an absolute path via `$PATH`, so callers
    /// never hand a bare name to `Command::new` (which trusts whatever the
    /// current `PATH` resolves to — a hijack risk if `PATH` is attacker-controlled).
    pub fn resolve_exe(name: &str) -> Result<PathBuf> {
        if name.contains('/') {
            return std::fs::canonicalize(name)
                .with_context(|| format!("Executable not found: {}", name));
        }
        let path_var = std::env::var_os("PATH").context("PATH environment variable not set")?;
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                // Canonicalize the directory only, not the file itself: some
                // toolchains (e.g. rustup) ship `cargo`/`rustc` as symlinks to
                // a single multiplexer binary that dispatches on argv[0], so
                // fully resolving the symlink would rename the program and
                // break that dispatch.
                let canonical_dir = std::fs::canonicalize(&dir)
                    .with_context(|| format!("Failed to canonicalize {}", dir.display()))?;
                return Ok(canonical_dir.join(name));
            }
        }
        anyhow::bail!("Executable '{}' not found on PATH", name);
    }

    /// Abstraction over spawning a subprocess, so pipeline phase logic can be
    /// unit-tested with `MockRunner` instead of spawning real processes.
    pub trait ProcessRunner {
        fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output>;
    }

    /// Default `ProcessRunner` — resolves `program` via `resolve_exe` and spawns it.
    pub struct SystemRunner;

    impl ProcessRunner for SystemRunner {
        fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
            let resolved = resolve_exe(program)?;
            let mut cmd = Command::new(&resolved);
            cmd.args(args);
            if let Some(dir) = cwd {
                cmd.current_dir(dir);
            }
            cmd.output()
                .with_context(|| format!("Failed to run '{}'", program))
        }
    }

    /// A single recorded invocation, captured by `MockRunner` for assertions.
    #[derive(Debug, Clone)]
    pub struct RecordedCall {
        pub program: String,
        pub args: Vec<String>,
        pub cwd: Option<PathBuf>,
    }

    /// Build a canned `std::process::Output` for use with `MockRunner`.
    #[cfg(unix)]
    pub fn mock_output(success: bool, stdout: &str, stderr: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: ExitStatus::from_raw(if success { 0 } else { 1 }),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    /// Test double for `ProcessRunner`: records every call and replays queued
    /// responses in FIFO order, so phase logic can be exercised without
    /// spawning real processes.
    #[derive(Default)]
    pub struct MockRunner {
        pub calls: RefCell<Vec<RecordedCall>>,
        responses: RefCell<VecDeque<std::result::Result<Output, String>>>,
    }

    impl MockRunner {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn push_response(&self, output: Output) {
            self.responses.borrow_mut().push_back(Ok(output));
        }

        pub fn push_error(&self, msg: &str) {
            self.responses.borrow_mut().push_back(Err(msg.to_string()));
        }
    }

    impl ProcessRunner for MockRunner {
        fn run(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> Result<Output> {
            self.calls.borrow_mut().push(RecordedCall {
                program: program.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                cwd: cwd.map(|p| p.to_path_buf()),
            });
            match self.responses.borrow_mut().pop_front() {
                Some(Ok(out)) => Ok(out),
                Some(Err(msg)) => anyhow::bail!(msg),
                None => anyhow::bail!("MockRunner: no response queued for '{}'", program),
            }
        }
    }
}

pub mod test_runner {
    use crate::process::ProcessRunner;
    use anyhow::{Context, Result};
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

    pub fn run_local_tests(project_path: &Path, runner: &dyn ProcessRunner) -> Result<TestResult> {
        let test_runner = detect_test_runner(project_path).context(
            "No test runner detected (Makefile, package.json, pyproject.toml, setup.py)",
        )?;

        let output = runner
            .run(
                test_runner.program(),
                test_runner.args(),
                Some(project_path),
            )
            .with_context(|| format!("Failed to run '{}'", test_runner.name()))?;

        Ok(TestResult {
            runner: test_runner,
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub mod git {
    use crate::process::ProcessRunner;
    use anyhow::{Context, Result};
    use once_cell::sync::Lazy;
    use regex::Regex;
    use std::path::Path;

    pub mod paths {
        use anyhow::{Context, Result};
        use std::path::Path;

        /// Validate and canonicalize a project path
        /// Returns Ok(canonical_path) or Err with descriptive message
        pub fn validate_project_path(path: &Path) -> Result<std::path::PathBuf> {
            let canonical = std::fs::canonicalize(path)
                .with_context(|| format!("Invalid project path: {}", path.display()))?;

            if !canonical.join(".git").exists() && !canonical.join("Cargo.toml").exists() {
                anyhow::bail!(
                    "Not a project directory (missing .git or Cargo.toml): {}",
                    canonical.display()
                );
            }

            Ok(canonical)
        }
    }

    static GITHUB_REMOTE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?$")
            .expect("Invalid GITHUB_REMOTE_RE pattern")
    });

    pub fn get_repo_info(project_path: &Path, runner: &dyn ProcessRunner) -> Result<String> {
        let output = runner
            .run("git", &["remote", "get-url", "origin"], Some(project_path))
            .context("Failed to get git remote")?;

        if !output.status.success() {
            anyhow::bail!("No git remote 'origin' found");
        }

        let remote_url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if let Some(caps) = GITHUB_REMOTE_RE.captures(&remote_url) {
            let owner = caps[1].trim();
            let repo = caps[2].trim();
            validate_github_slug(owner)?;
            validate_github_slug(repo)?;
            Ok(format!("{}/{}", owner, repo))
        } else {
            anyhow::bail!("Not a GitHub repository: {}", remote_url)
        }
    }

    fn validate_github_slug(slug: &str) -> Result<()> {
        // GitHub username/repo names: alphanumeric, hyphens, underscores, dots
        // Cannot start/end with hyphen, cannot be empty, max 39 chars for users
        static SLUG_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_.-]+$").expect("Invalid SLUG_RE pattern"));
        if slug.is_empty() || slug.len() > 100 || !SLUG_RE.is_match(slug) {
            anyhow::bail!("Invalid GitHub slug: '{}'", slug);
        }
        Ok(())
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
    /// Returns the heading text if `line` is a Markdown heading (any depth 1-6), else None.
    fn heading_text(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();
        if hashes == 0 || hashes > 6 {
            return None;
        }
        let rest = &trimmed[hashes..];
        rest.strip_prefix(' ').map(|s| s.trim())
    }

    pub fn extract_section(content: &str, section_name: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut in_section = false;
        let target = section_name.trim().to_lowercase();

        for line in &lines {
            if let Some(heading) = heading_text(line) {
                if heading_matches(heading, &target) {
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

    /// Does a Markdown heading identify the requested section?
    ///
    /// Matching is tolerant of leading decoration (emoji/symbols the aggregator
    /// prepends, e.g. `🔴 Do Now (Quick Wins)`) and of a trailing parenthetical
    /// or descriptive suffix, while still being strict on word boundaries so
    /// `"Do"` does NOT match `"Don't Do This"`. Rule: strip leading
    /// non-alphanumeric characters, lowercase, then accept an exact match or a
    /// `target + " "` prefix (word-boundary safe).
    fn heading_matches(heading: &str, target: &str) -> bool {
        let normalized = heading
            .trim_start_matches(|c: char| !c.is_alphanumeric())
            .trim()
            .to_lowercase();
        normalized == *target || normalized.starts_with(&format!("{} ", target))
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
    fn test_extract_section_any_heading_depth() {
        let content = "# Plan\n\n### Do Now\n- Fix 1\n\n### Defer\n- Fix 2";
        let section = markdown::extract_section(content, "Do Now");
        assert!(section.contains("Fix 1"));
        assert!(!section.contains("Fix 2"));
    }

    #[test]
    fn test_extract_section_exact_match_not_substring() {
        let content = "# Plan\n\n## Don't Do This\n- Fix 1\n\n## Do\n- Fix 2";
        let section = markdown::extract_section(content, "Do");
        assert!(!section.contains("Fix 1"));
        assert!(section.contains("Fix 2"));
    }

    #[test]
    fn test_extract_section_matches_aggregator_decorated_heading() {
        // Regression: review-aggregator emits "## 🔴 Do Now (Quick Wins)".
        // The execute phase calls extract_section(plan, "Do Now"); a strict
        // whole-heading equality check silently missed this, so execute found
        // zero fixes on real aggregator output.
        let content =
            "# Auto-Dev Fix Plan\n\n## 🔴 Do Now (Quick Wins)\n- Fix A\n- Fix B\n\n## 🟡 Defer\n- Fix C";
        let section = markdown::extract_section(content, "Do Now");
        assert!(
            section.contains("Fix A"),
            "decorated 'Do Now' heading not matched"
        );
        assert!(section.contains("Fix B"));
        assert!(!section.contains("Fix C"), "section bled into Defer");
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

    #[test]
    fn test_resolve_exe_finds_known_binary() {
        // "sh" is guaranteed present in any POSIX environment CI runs in.
        let resolved = process::resolve_exe("sh").expect("sh should be on PATH");
        assert!(resolved.is_absolute());
        assert!(resolved.is_file());
    }

    #[test]
    fn test_resolve_exe_rejects_unknown_binary() {
        assert!(process::resolve_exe("definitely-not-a-real-binary-xyz").is_err());
    }

    #[test]
    fn test_mock_runner_records_calls_and_replays_responses() {
        use process::{mock_output, MockRunner, ProcessRunner};

        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "origin-output", ""));

        let output = mock
            .run("git", &["remote", "get-url", "origin"], None)
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "origin-output");

        let calls = mock.calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].program, "git");
    }

    #[test]
    fn test_get_repo_info_via_mock_runner() {
        use process::{mock_output, MockRunner};

        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "git@github.com:ni9aii/AutoDev.git\n", ""));

        let repo = git::get_repo_info(std::path::Path::new("."), &mock).unwrap();
        assert_eq!(repo, "ni9aii/AutoDev");
    }
}
