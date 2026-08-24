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
        // Redact userinfo before echoing the remote anywhere (plan finding:
        // credential-bearing remote URL leaked into error messages that
        // reach stderr/logs). Remotes like https://x-access-token:TOKEN@…
        // are exactly the non-GitHub case this error fires for.
        anyhow::bail!("Not a GitHub repository: {}", redact_url(&remote_url))
    }
}

/// Replace `user:password@` userinfo in a URL-ish string with `***@` so
/// tokens never land in logs or error output. Keeps the scheme prefix
/// (`https://`, `ssh://`) and everything after the userinfo.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn redact_url(url: &str) -> String {
    static USERINFO_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(^[a-zA-Z][a-zA-Z0-9+.-]*://)[^/@\s]+:[^/@\s]+@")
            .expect("Invalid USERINFO_RE pattern")
    });
    USERINFO_RE.replace_all(url, "${1}***@").to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{mock_output, MockRunner};

    #[test]
    fn test_get_repo_info_via_mock_runner() {
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "git@github.com:ni9aii/AutoDev.git\n", ""));

        let repo = get_repo_info(std::path::Path::new("."), &mock).unwrap();
        assert_eq!(repo, "ni9aii/AutoDev");
    }

    #[test]
    fn test_get_repo_info_redacts_credentials_in_error() {
        // Regression (plan finding: credential-bearing remote URL echoed
        // verbatim): a non-GitHub remote with embedded token must never reach
        // the error message with the secret intact.
        let mock = MockRunner::new();
        mock.push_response(mock_output(
            true,
            "https://x-access-token:ghs_secret123@gitlab.example.com/group/repo.git\n",
            "",
        ));
        let err = get_repo_info(std::path::Path::new("."), &mock)
            .expect_err("non-GitHub remote must fail");
        let msg = err.to_string();
        assert!(!msg.contains("ghs_secret123"), "token leaked: {}", msg);
        assert!(
            msg.contains("***@gitlab.example.com"),
            "userinfo not redacted: {}",
            msg
        );
    }

    #[test]
    fn test_redact_url_variants() {
        // https + user:pass
        assert_eq!(
            redact_url("https://user:pass@example.com/repo.git"),
            "https://***@example.com/repo.git"
        );
        // ssh-style scp syntax has no scheme → nothing matches → unchanged
        assert_eq!(
            redact_url("git@github.com:owner/repo.git"),
            "git@github.com:owner/repo.git"
        );
        // ssh:// scheme with userinfo
        assert_eq!(
            redact_url("ssh://oauth2:TOKEN@gitlab.com/group/proj.git"),
            "ssh://***@gitlab.com/group/proj.git"
        );
        // no userinfo → unchanged
        assert_eq!(
            redact_url("https://github.com/owner/repo.git"),
            "https://github.com/owner/repo.git"
        );
    }
}
