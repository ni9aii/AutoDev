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
    use std::process::{Command, Output};

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
    /// Build a fake `Output` with the given success flag and captured streams.
    /// Cross-platform: uses the OS-specific `ExitStatusExt::from_raw` so the
    /// same helper compiles and behaves identically on Unix and Windows.
    pub fn mock_output(success: bool, stdout: &str, stderr: &str) -> Output {
        let code = if success { 0 } else { 1 };
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(code);
        #[cfg(windows)]
        let status = std::os::windows::process::ExitStatusExt::from_raw(code);
        #[cfg(not(any(unix, windows)))]
        let status = std::process::Command::new(if success { "true" } else { "false" })
            .status()
            .unwrap();
        Output {
            status,
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

pub mod bin_contract {
    use std::path::PathBuf;

    pub const AGGREGATOR: &str = "review-aggregator";
    pub const CI_CHECK: &str = "ci-check";

    /// Append the platform executable suffix (`` on Unix, `.exe` on Windows).
    pub fn companion_exe_name(base: &str) -> String {
        format!("{}{}", base, std::env::consts::EXE_SUFFIX)
    }

    /// Resolve a companion binary: prefer the file sitting next to the running
    /// executable (works under `cargo test`/`target/` and `$PATH` installs),
    /// fall back to the bare name so a `$PATH` install still works.
    pub fn resolve_companion(base: &str) -> String {
        let exe_name = companion_exe_name(base);
        if let Ok(current) = std::env::current_exe() {
            if let Some(dir) = current.parent() {
                let candidate: PathBuf = dir.join(&exe_name);
                if candidate.is_file() {
                    return candidate.display().to_string();
                }
            }
        }
        exe_name
    }

    /// Typed request for the `review-aggregator` companion. Renders the exact
    /// CLI arg vector the aggregator already parses (`--input-dir`, `--output`,
    /// optional `--project`, `--dev-notes --dev-notes-root`).
    #[derive(Clone, Debug)]
    pub struct AggregateRequest {
        pub input_dir: PathBuf,
        pub output: PathBuf,
        pub project: Option<String>,
        pub dev_notes_root: Option<PathBuf>,
    }

    impl AggregateRequest {
        pub fn to_args(&self) -> Vec<String> {
            let mut a = vec![
                "--input-dir".to_string(),
                self.input_dir.display().to_string(),
                "--output".to_string(),
                self.output.display().to_string(),
            ];
            if let Some(p) = &self.project {
                a.push("--project".to_string());
                a.push(p.clone());
            }
            if let Some(root) = &self.dev_notes_root {
                a.push("--dev-notes".to_string());
                a.push("--dev-notes-root".to_string());
                a.push(root.display().to_string());
            }
            a
        }
    }

    /// Typed request for the `ci-check` companion.
    #[derive(Clone, Debug)]
    pub struct CiCheckRequest {
        pub project_path: PathBuf,
        pub project: Option<String>,
        pub dev_notes: bool,
    }

    impl CiCheckRequest {
        pub fn to_args(&self) -> Vec<String> {
            let mut a = vec![self.project_path.display().to_string()];
            if let Some(p) = &self.project {
                a.push("--project".to_string());
                a.push(p.clone());
            }
            if self.dev_notes {
                a.push("--dev-notes".to_string());
            }
            a
        }
    }
}

pub mod test_runner {
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
        } else if project_path.join("pyproject.toml").exists()
            || project_path.join("setup.py").exists()
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
}

pub mod git {
    use crate::process::ProcessRunner;
    use anyhow::{Context, Result};
    use once_cell::sync::Lazy;
    use regex::Regex;
    use std::path::Path;

    pub mod paths {
        use anyhow::{Context, Result};
        use std::path::PathBuf;

        /// Resolve the dev-notes root directory.
        ///
        /// Precedence: explicit `--dev-notes-root` override > `$DEV_NOTES_ROOT`
        /// env var > `~/obsidian-vault/dev-notes` default. Shared by all three
        /// binaries (`run-pipeline`, `review-aggregator`, `ci-check`) so their
        /// behaviour can't drift.
        pub fn resolve_dev_notes_root(override_path: Option<&PathBuf>) -> Result<PathBuf> {
            if let Some(p) = override_path {
                return Ok(p.clone());
            }
            if let Ok(env_root) = std::env::var("DEV_NOTES_ROOT") {
                return Ok(PathBuf::from(env_root));
            }
            let home = dirs::home_dir().context("Could not determine home directory")?;
            Ok(home.join("obsidian-vault").join("dev-notes"))
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
    use once_cell::sync::Lazy;
    use regex::Regex;

    /// Semver-like version matcher, compiled once. Allows optional `v` prefix
    /// and an optional pre-release suffix: v0.1.0, 1.0.0, v2.0.0-alpha.
    static VERSION_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^v?\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?$").expect("Invalid VERSION_RE pattern")
    });

    /// Validate a version string for use in git tags and JSON payloads.
    /// Only allows semver-like strings: starts with optional 'v', then digits and dots.
    pub fn validate_version(version: &str) -> Result<(), String> {
        let clean = version.trim();
        if clean.is_empty() {
            return Err("Version string is empty".to_string());
        }
        if !VERSION_RE.is_match(clean) {
            return Err(format!(
                "Invalid version '{}'. Expected semver format: v0.1.0 or 1.0.0",
                clean
            ));
        }
        Ok(())
    }

    /// Validate a project name before it is used as a path component in the
    /// dev-notes tree (`<root>/<project>/reviews/…`). Rejects anything that
    /// could escape the root via path traversal: path separators, `..`
    /// components, absolute paths, and empty/whitespace names. This is a
    /// security control — `--project` (and a derived repo name) is attacker-
    /// influenced input that is joined onto the dev-notes root.
    pub fn validate_project_name(name: &str) -> Result<(), String> {
        let clean = name.trim();
        if clean.is_empty() {
            return Err("Project name is empty".to_string());
        }
        if clean.contains('/') || clean.contains('\\') {
            return Err(format!(
                "Invalid project name '{}': must not contain path separators",
                clean
            ));
        }
        if clean == ".." || clean == "." {
            return Err(format!(
                "Invalid project name '{}': reserved path component",
                clean
            ));
        }
        // Belt-and-suspenders: reject any embedded parent-dir traversal.
        if clean
            .split(|c| ['/', '\\'].contains(&c))
            .any(|seg| seg == "..")
        {
            return Err(format!("Invalid project name '{}': path traversal", clean));
        }
        Ok(())
    }
}

pub mod severity {
    use std::fmt;
    use std::str::FromStr;

    /// Finding severity, ordered most-severe first (`Critical` < `Important` < `Minor`)
    /// so that `sort()` places critical findings at the top.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Severity {
        Critical,
        Important,
        Minor,
    }

    impl Severity {
        pub fn as_str(&self) -> &'static str {
            match self {
                Severity::Critical => "CRITICAL",
                Severity::Important => "IMPORTANT",
                Severity::Minor => "MINOR",
            }
        }
    }

    impl fmt::Display for Severity {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.as_str())
        }
    }

    impl FromStr for Severity {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.trim().to_uppercase().as_str() {
                "CRITICAL" => Ok(Severity::Critical),
                "IMPORTANT" => Ok(Severity::Important),
                "MINOR" => Ok(Severity::Minor),
                other => Err(format!("Unknown severity: {}", other)),
            }
        }
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
    fn test_validate_project_name_blocks_traversal() {
        // Valid names pass.
        assert!(validation::validate_project_name("AutoDev").is_ok());
        assert!(validation::validate_project_name("my-project_1").is_ok());

        // Reject path traversal and separators (Fix 21 from dogfood review).
        assert!(validation::validate_project_name("../escape").is_err());
        assert!(validation::validate_project_name("foo/bar").is_err());
        assert!(validation::validate_project_name("foo\\bar").is_err());
        assert!(validation::validate_project_name("..\\escape").is_err());
        assert!(validation::validate_project_name("..").is_err());
        assert!(validation::validate_project_name(".").is_err());
        assert!(validation::validate_project_name("").is_err());
        assert!(validation::validate_project_name("  ").is_err());
    }

    #[test]
    fn test_resolve_exe_finds_known_binary() {
        // A shell present on the running OS: `sh` on Unix, `cmd.exe` on Windows.
        #[cfg(unix)]
        let name = "sh";
        #[cfg(windows)]
        let name = "cmd.exe";
        let resolved = process::resolve_exe(name).expect("known shell should be on PATH");
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

    #[test]
    fn test_severity_parse_display_order() {
        use crate::severity::Severity;
        assert_eq!("CRITICAL".parse::<Severity>().unwrap(), Severity::Critical);
        assert_eq!("critical".parse::<Severity>().unwrap(), Severity::Critical);
        assert_eq!(
            " Important ".parse::<Severity>().unwrap(),
            Severity::Important
        );
        assert_eq!(Severity::Minor.to_string(), "MINOR");
        assert!("bogus".parse::<Severity>().is_err());
        // Ordering: Critical is most severe (sorts first).
        let mut v = vec![Severity::Minor, Severity::Critical, Severity::Important];
        v.sort();
        assert_eq!(
            v,
            vec![Severity::Critical, Severity::Important, Severity::Minor]
        );
    }

    #[test]
    fn test_resolve_companion_uses_exe_suffix() {
        let name = crate::bin_contract::companion_exe_name("review-aggregator");
        assert!(name.ends_with(std::env::consts::EXE_SUFFIX));
        assert!(name.starts_with("review-aggregator"));
        assert_eq!(crate::bin_contract::AGGREGATOR, "review-aggregator");
        assert_eq!(crate::bin_contract::CI_CHECK, "ci-check");
    }

    #[test]
    fn test_aggregate_request_args_roundtrip() {
        let req = crate::bin_contract::AggregateRequest {
            input_dir: "/tmp/r".into(),
            output: "/tmp/p.md".into(),
            project: Some("proj".into()),
            dev_notes_root: Some("/dn".into()),
        };
        let args = req.to_args();
        assert_eq!(args[0], "--input-dir");
        assert_eq!(args[1], "/tmp/r");
        assert!(args.contains(&"--dev-notes".to_string()));
        assert!(args.contains(&"--project".to_string()));
    }

    #[test]
    fn test_mock_output_cross_platform_helper() {
        let o = crate::process::mock_output(true, "x", "");
        assert!(o.status.success());
        assert_eq!(String::from_utf8_lossy(&o.stdout), "x");
        let e = crate::process::mock_output(false, "", "boom");
        assert!(!e.status.success());
        assert_eq!(String::from_utf8_lossy(&e.stderr), "boom");
    }

    #[test]
    fn test_run_local_tests_no_runner_is_none() {
        let td = std::env::temp_dir().join(format!("autodev-norunner-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&td);
        let runner = crate::process::MockRunner::new();
        // Empty dir: no Makefile/Cargo.toml/package.json/pyproject.toml/setup.py.
        let res = crate::test_runner::run_local_tests(&td, &runner);
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
        let runner = crate::process::MockRunner::new();
        // Makefile present -> Make detected, but the command can't launch -> None (skip).
        runner.push_error("make: command not found");
        let res = crate::test_runner::run_local_tests(&td, &runner);
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
        let runner = crate::process::MockRunner::new();
        runner.push_response(crate::process::mock_output(true, "ok", ""));
        let res = crate::test_runner::run_local_tests(&td, &runner);
        match res {
            Ok(Some(r)) => assert!(r.success, "expected success"),
            other => panic!("expected Ok(Some), got {:?}", other),
        }
        let _ = std::fs::remove_dir_all(&td);
    }
}
