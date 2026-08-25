use crate::Pipeline;
use anyhow::{Context, Result};
use auto_dev_pipeline::{git, log};

/// Read the `[package] version` from a Cargo.toml string without pulling in
/// a TOML parser: scan lines, tracking whether we are inside `[package]`.
/// Everything after a `#` on a line is stripped first (TOML comments), so
/// `version = "1.2.3" # bumped` parses correctly instead of poisoning the
/// extracted value with the trailing comment text. Returns None when absent
/// or malformed.
fn cargo_package_version(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        // Strip TOML comments: everything from the first '#' onward. A '#'
        // inside a quoted value would be truncated too, but semver values
        // never contain '#' and this is intentionally minimal.
        let no_comment = line.split('#').next().unwrap_or(line);
        let trimmed = no_comment.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

impl Pipeline {
    pub(crate) fn run_release_phase(&self, version: &str) -> Result<()> {
        log::log("=== PHASE 5: RELEASE ===");

        // Validate version string (prevent injection)
        auto_dev_pipeline::validation::validate_version(version)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Strip an optional leading 'v' for Cargo.toml / CHANGELOG lookups.
        let bare_version = version.strip_prefix('v').unwrap_or(version);

        // ---- Pre-flight checks: all BEFORE any side effect ----------------

        // 1. Working tree must be clean.
        log::log("Pre-flight: checking working tree is clean...");
        let status_output = self
            .runner
            .run("git", &["status", "--porcelain"], Some(&self.project_path))
            .context("Failed to run git status")?;
        let dirty = String::from_utf8_lossy(&status_output.stdout);
        if !dirty.trim().is_empty() {
            anyhow::bail!(
                "Working tree is not clean — commit or stash before releasing:\n{}",
                dirty.trim()
            );
        }

        // 2. Must be on main.
        log::log("Pre-flight: checking current branch is main...");
        let branch_output = self
            .runner
            .run(
                "git",
                &["rev-parse", "--abbrev-ref", "HEAD"],
                Some(&self.project_path),
            )
            .context("Failed to read current branch")?;
        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();
        if branch != "main" {
            anyhow::bail!(
                "Releases run from 'main' but current branch is '{}'. \
                 Merge to main first.",
                branch
            );
        }

        // 3. Cargo.toml version must match the requested tag.
        log::log(&format!(
            "Pre-flight: checking Cargo.toml version matches {}...",
            version
        ));
        let cargo_toml_path = self.project_path.join("Cargo.toml");
        let cargo_toml = std::fs::read_to_string(&cargo_toml_path).with_context(|| {
            format!(
                "Cannot read {}: run this from the project root or pass the project path",
                cargo_toml_path.display()
            )
        })?;
        let cargo_version = cargo_package_version(&cargo_toml)
            .context("No [package] version found in Cargo.toml")?;
        if cargo_version != bare_version {
            anyhow::bail!(
                "Cargo.toml version is '{}' but requested release tag is '{}'. \
                 Bump the version and commit before releasing.",
                cargo_version,
                bare_version
            );
        }

        // 4. Changelog gate: the released version needs a curated section,
        // which also becomes the GitHub Release body.
        log::log(&format!(
            "Pre-flight: checking CHANGELOG.md has a [{}] section...",
            bare_version
        ));
        let changelog_path = self.project_path.join("CHANGELOG.md");
        let changelog = std::fs::read_to_string(&changelog_path)
            .with_context(|| format!("Cannot read {}", changelog_path.display()))?;
        let release_body =
            auto_dev_pipeline::markdown::extract_changelog_section(&changelog, bare_version)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "CHANGELOG.md has no '## [{}]' section. Add one describing \
                         this release before tagging.",
                        bare_version
                    )
                })?;

        // 5. A GitHub token must be available.
        let token = auto_dev_pipeline::github::resolve_token(self.runner.as_ref())?.context(
            "No GitHub token found. Provide one via GITHUB_PAT, GITHUB_TOKEN, \
                 or `gh auth login` before releasing.",
        )?;

        // 6. HEAD commit's GitHub Actions CI must be green.
        log::log("Pre-flight: checking GitHub Actions CI status for HEAD...");
        let repo = git::get_repo_info(&self.project_path, self.runner.as_ref())?;
        let sha_output = self
            .runner
            .run("git", &["rev-parse", "HEAD"], Some(&self.project_path))
            .context("Failed to resolve HEAD commit")?;
        let sha = String::from_utf8_lossy(&sha_output.stdout)
            .trim()
            .to_string();
        let conclusions =
            auto_dev_pipeline::github::head_ci_success(&repo, &sha, &token, self.http.as_ref())?;
        if conclusions.is_empty() {
            anyhow::bail!(
                "No GitHub Actions workflow runs found for HEAD ({}) — cannot \
                 verify CI. Push the commit and wait for CI before releasing.",
                &sha[..sha.len().min(12)]
            );
        }
        let failures: Vec<&String> = conclusions
            .iter()
            .filter(|c| c.as_str() != "success")
            .collect();
        if !failures.is_empty() {
            anyhow::bail!(
                "GitHub Actions CI for HEAD is not green (conclusions: {}). Fix CI \
                 on main before releasing.",
                failures
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        log::success("All pre-flight checks passed");

        // ---- Side effects --------------------------------------------------

        // Build release binary
        log::log("Building release binary...");
        let build_output = self
            .runner
            .run("cargo", &["build", "--release"], Some(&self.project_path))
            .context("Failed to build release binary")?;

        if !build_output.status.success() {
            let stderr = String::from_utf8_lossy(&build_output.stderr);
            anyhow::bail!("Release build failed: {}", stderr);
        }
        log::success("Release build complete");

        // Create git tag
        log::log(&format!("Creating tag: {}", version));
        let tag_message = format!("Release {}", version);
        let tag_output = self
            .runner
            .run(
                "git",
                &["tag", "-a", version, "-m", &tag_message],
                Some(&self.project_path),
            )
            .context("Failed to create git tag")?;

        if !tag_output.status.success() {
            let stderr = String::from_utf8_lossy(&tag_output.stderr);
            anyhow::bail!("Failed to create tag: {}", stderr);
        }
        log::success(&format!("Tag {} created", version));

        // Push tag
        log::log("Pushing tag...");
        let push_output = self
            .runner
            .run(
                "git",
                &["push", "origin", version],
                Some(&self.project_path),
            )
            .context("Failed to push tag")?;

        if !push_output.status.success() {
            let stderr = String::from_utf8_lossy(&push_output.stderr);
            anyhow::bail!("Failed to push tag: {}", stderr);
        }
        log::success("Tag pushed to origin");

        // Create GitHub Release via shared client (token stays in process memory)
        log::log("Creating GitHub Release...");
        let (status, body) = auto_dev_pipeline::github::create_release(
            &repo,
            version,
            &format!("Release {}", version),
            &release_body,
            &token,
            self.http.as_ref(),
        )?;

        if (200..300).contains(&status) {
            log::success(&format!("GitHub Release {} created", version));
        } else {
            // Fatal (plan finding: partial release reported as success — tag
            // pushed but release missing must not look like a completed
            // release).
            anyhow::bail!(
                "GitHub release creation failed ({}): {}. Tag {} is already pushed — \
                 create the release manually or delete the tag before retrying.",
                status,
                body,
                version
            );
        }

        log::success(&format!("Release {} complete", version));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use auto_dev_pipeline::github::HttpClient;
    use auto_dev_pipeline::process::{mock_output, MockRunner};
    use serde_json::Value;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::path::PathBuf;

    /// Mock [`HttpClient`]: replays queued GET results in FIFO order and a
    /// fixed POST result; records POST payloads for assertions via a
    /// shared handle that survives moving the mock into the Pipeline.
    struct MockHttp {
        gets: RefCell<VecDeque<Result<Value>>>,
        post: RefCell<Option<Result<(u16, String)>>>,
        posts: std::rc::Rc<RefCell<Vec<String>>>,
    }

    impl MockHttp {
        fn with_ci(conclusions: &[&str]) -> Self {
            let runs: Vec<Value> = conclusions
                .iter()
                .map(|c| serde_json::json!({ "conclusion": c }))
                .collect();
            let mut gets = VecDeque::new();
            gets.push_back(Ok(serde_json::json!({ "workflow_runs": runs })));
            Self {
                gets: RefCell::new(gets),
                post: RefCell::new(Some(Ok((201, "{}".to_string())))),
                posts: std::rc::Rc::new(RefCell::new(Vec::new())),
            }
        }

        fn no_token_get() -> Self {
            Self {
                gets: RefCell::new(VecDeque::new()),
                post: RefCell::new(None),
                posts: std::rc::Rc::new(RefCell::new(Vec::new())),
            }
        }

        /// Handle for reading recorded POST payloads after the mock is moved.
        fn posts_handle(&self) -> std::rc::Rc<RefCell<Vec<String>>> {
            self.posts.clone()
        }
    }

    impl HttpClient for MockHttp {
        fn post_json(&self, _url: &str, _token: &str, payload: &Value) -> Result<(u16, String)> {
            self.posts.borrow_mut().push(payload.to_string());
            match self.post.borrow_mut().take() {
                Some(r) => r,
                None => anyhow::bail!("MockHttp: unexpected POST"),
            }
        }

        fn get_json(&self, _url: &str, _token: Option<&str>) -> Result<Value> {
            match self.gets.borrow_mut().pop_front() {
                Some(r) => r,
                None => anyhow::bail!("MockHttp: unexpected GET"),
            }
        }
    }

    /// Fixture project directory with Cargo.toml/CHANGELOG.md matching v1.2.3.
    fn fixture_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autodev-release-test-{}-{}-{}",
            label,
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n- wip\n\n## [1.2.3] - 2026-08-25\n\n### Added\n- the thing\n",
        )
        .unwrap();
        dir
    }

    /// Queue the runner responses every pre-flight-passing test needs:
    /// clean tree, main branch, gh token fallback, remote URL, HEAD sha.
    fn happy_runner_responses(mock: &MockRunner) {
        mock.push_response(mock_output(true, "", "")); // git status: clean
        mock.push_response(mock_output(true, "main\n", "")); // branch
        mock.push_response(mock_output(true, "tok-from-gh\n", "")); // gh auth token
        mock.push_response(mock_output(true, "https://github.com/owner/repo.git\n", "")); // remote
        mock.push_response(mock_output(true, "abcdef1234567890\n", "")); // rev-parse HEAD
    }

    fn pipeline_for(label: &str, mock: MockRunner, http: MockHttp) -> crate::Pipeline {
        let mut p = crate::Pipeline::test_default(Box::new(mock));
        p.phase = crate::Phase::Release;
        p.version = Some("v1.2.3".to_string());
        p.project_path = fixture_dir(label);
        p.http = Box::new(http);
        p
    }

    #[test]
    fn test_release_phase_rejects_bad_version() {
        // Validation happens before any process call, so an empty MockRunner is fine.
        let p = pipeline_for("badver", MockRunner::new(), MockHttp::no_token_get());
        let res = p.run_release_phase("; rm -rf /");
        assert!(res.is_err(), "malicious version must be rejected");
        assert!(res.unwrap_err().to_string().contains("Invalid version"));
    }

    #[test]
    fn test_release_happy_path_build_tag_push_release() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        mock.push_response(mock_output(true, "compiled\n", "")); // cargo build
        mock.push_response(mock_output(true, "", "")); // git tag
        mock.push_response(mock_output(true, "", "")); // git push
        let http = MockHttp::with_ci(&["success", "success"]);
        let posts = http.posts_handle();
        let p = pipeline_for("happy", mock, http);
        let res = p.run_release_phase("v1.2.3");
        assert!(res.is_ok(), "happy path failed: {:?}", res.err());

        // The GitHub Release body is the CHANGELOG.md section, not a
        // generic "Auto-generated release" string.
        let bodies = posts.borrow();
        assert_eq!(bodies.len(), 1);
        assert!(bodies[0].contains("### Added"), "{}", bodies[0]);
        assert!(bodies[0].contains("the thing"), "{}", bodies[0]);
        assert!(!bodies[0].contains("Auto-generated"), "{}", bodies[0]);
        assert!(!bodies[0].contains("wip"), "{}", bodies[0]);
        // Ordering is enforced structurally: the FIFO MockRunner queue only
        // lines up if every pre-flight call happens before the side effects,
        // and this test passing proves build/tag/push consumed their
        // responses after the five pre-flight ones.
    }

    #[test]
    fn test_release_bails_on_dirty_tree_before_any_side_effect() {
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, " M src/lib.rs\n", ""));
        let http = MockHttp::no_token_get();
        let p = pipeline_for("dirty", mock, http);
        let res = p.run_release_phase("v1.2.3");
        assert!(res.unwrap_err().to_string().contains("not clean"));
    }

    #[test]
    fn test_release_bails_when_not_on_main() {
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "", "")); // clean tree
        mock.push_response(mock_output(true, "feat/x\n", "")); // branch
        let http = MockHttp::no_token_get();
        let p = pipeline_for("branch", mock, http);
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("current branch is 'feat/x'"), "{}", err);
    }

    #[test]
    fn test_release_bails_on_cargo_version_mismatch() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        // Fixture says 1.2.3; request something else.
        let mut p = pipeline_for("vermismatch", mock, MockHttp::with_ci(&["success"]));
        p.version = Some("v9.9.9".to_string());
        let res = p.run_release_phase("v9.9.9");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("Cargo.toml version is '1.2.3'"), "{}", err);
    }

    #[test]
    fn test_release_bails_without_changelog_section() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        let p = pipeline_for("nochange", mock, MockHttp::with_ci(&["success"]));
        std::fs::write(
            p.project_path.join("CHANGELOG.md"),
            "# Changelog\n\n## [9.9.9] - date\n",
        )
        .unwrap();
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("CHANGELOG.md has no '## [1.2.3]'"), "{}", err);
    }

    #[test]
    fn test_release_bails_without_token() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _env = ENV_LOCK.lock().unwrap();
        let prev_pat = std::env::var("GITHUB_PAT").ok();
        let prev_tok = std::env::var("GITHUB_TOKEN").ok();
        std::env::remove_var("GITHUB_PAT");
        std::env::remove_var("GITHUB_TOKEN");

        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "", "")); // clean tree
        mock.push_response(mock_output(true, "main\n", "")); // branch
        mock.push_error("no gh cli"); // token resolution fails everywhere

        let p = pipeline_for("notoken", mock, MockHttp::no_token_get());
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("No GitHub token found"), "{}", err);
        assert!(err.contains("GITHUB_PAT"), "{}", err);

        // Restore ambient env for other tests.
        if let Some(v) = prev_pat {
            std::env::set_var("GITHUB_PAT", v);
        } else {
            std::env::remove_var("GITHUB_PAT");
        }
        if let Some(v) = prev_tok {
            std::env::set_var("GITHUB_TOKEN", v);
        } else {
            std::env::remove_var("GITHUB_TOKEN");
        }
    }

    #[test]
    fn test_release_bails_when_ci_not_green() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        let http = MockHttp::with_ci(&["success", "failure"]);
        let p = pipeline_for("cired", mock, http);
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("not green"), "{}", err);
    }

    #[test]
    fn test_release_bails_when_no_ci_runs_for_head() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        let http = MockHttp::with_ci(&[]);
        let p = pipeline_for("noci", mock, http);
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(err.contains("No GitHub Actions workflow runs"), "{}", err);
    }

    #[test]
    fn test_release_bails_when_github_release_api_fails_after_push() {
        let mock = MockRunner::new();
        happy_runner_responses(&mock);
        mock.push_response(mock_output(true, "compiled\n", ""));
        mock.push_response(mock_output(true, "", ""));
        mock.push_response(mock_output(true, "", ""));
        let http = MockHttp::with_ci(&["success"]);
        *http.post.borrow_mut() = Some(Ok((422, "{\"message\":\"already_exists\"}".to_string())));
        let p = pipeline_for("apifail", mock, http);
        let res = p.run_release_phase("v1.2.3");
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("GitHub release creation failed (422)"),
            "{}",
            err
        );
        assert!(err.contains("already pushed"), "{}", err);
    }

    #[test]
    fn test_cargo_package_version_parses_and_scopes_to_package_table() {
        let toml =
            "[package]\nname = \"x\"\nversion = \"1.2.3\"\n\n[dependencies]\nserde = \"1\"\n";
        assert_eq!(super::cargo_package_version(toml).as_deref(), Some("1.2.3"));
        assert_eq!(
            super::cargo_package_version("[dependencies]\nserde = \"1\""),
            None
        );
        assert_eq!(super::cargo_package_version(""), None);
    }

    #[test]
    fn test_cargo_package_version_strips_inline_comment() {
        // Fix 4 regression: a hand-edited Cargo.toml with a trailing comment
        // must yield the clean version, not "1.2.3\" # release".
        let toml = "[package]\nname = \"x\"\nversion = \"1.2.3\" # release cut\n";
        assert_eq!(super::cargo_package_version(toml).as_deref(), Some("1.2.3"));
        // Full-line comments and commented-out duplicates are ignored.
        let toml2 = concat!(
            "# workspace root\n",
            "[package]\n",
            "# version = \"9.9.9\"\n",
            "version = \"2.0.0\"\n"
        );
        assert_eq!(
            super::cargo_package_version(toml2).as_deref(),
            Some("2.0.0")
        );
    }
}
