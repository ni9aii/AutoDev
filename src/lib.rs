pub mod log;

pub mod bin_contract;

pub mod process;

pub mod test_runner;

pub mod git;

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

pub mod severity;

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

pub mod github;

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
    fn test_get_repo_info_redacts_credentials_in_error() {
        use process::{mock_output, MockRunner};

        // Regression (plan finding: credential-bearing remote URL echoed
        // verbatim): a non-GitHub remote with embedded token must never reach
        // the error message with the secret intact.
        let mock = MockRunner::new();
        mock.push_response(mock_output(
            true,
            "https://x-access-token:ghs_SUPERSECRET@gitlab.example.com/acme/widget.git\n",
            "",
        ));
        let err = git::get_repo_info(std::path::Path::new("."), &mock)
            .expect_err("non-GitHub remote must fail");
        let msg = err.to_string();
        assert!(!msg.contains("ghs_SUPERSECRET"), "token leaked: {}", msg);
        assert!(msg.contains("***@"), "userinfo not redacted: {}", msg);
        assert!(msg.contains("gitlab.example.com"), "host lost: {}", msg);
    }

    #[test]
    fn test_redact_url_variants() {
        // https + user:pass
        assert_eq!(
            git::redact_url("https://user:pass@example.com/repo.git"),
            "https://***@example.com/repo.git"
        );
        // ssh-style scp syntax has no scheme → nothing matches → unchanged
        assert_eq!(
            git::redact_url("git@github.com:owner/repo.git"),
            "git@github.com:owner/repo.git"
        );
        // ssh:// scheme with userinfo
        assert_eq!(
            git::redact_url("ssh://oauth2:TOKEN@gitlab.com/group/proj.git"),
            "ssh://***@gitlab.com/group/proj.git"
        );
        // no userinfo → unchanged
        assert_eq!(
            git::redact_url("https://github.com/owner/repo.git"),
            "https://github.com/owner/repo.git"
        );
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
