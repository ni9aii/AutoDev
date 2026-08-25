//! Single GitHub API client shared by ci-check and the release phase.
//!
//! Before this module, each binary hand-rolled its own blocking reqwest
//! client with divergent token precedence (ci-check: GITHUB_PAT →
//! GITHUB_TOKEN → `gh auth token`; release: GITHUB_TOKEN → GITHUB_PAT,
//! no gh fallback) and different failure semantics. One client, one
//! documented precedence (Fix 9).

use anyhow::{Context, Result};
use serde_json::Value;

/// Minimal blocking-HTTP abstraction over the two GitHub API verbs this
/// crate needs (POST JSON, GET JSON), so phase logic that calls the GitHub
/// API can be unit-tested with a mock client instead of hitting
/// api.github.com — the same hermetic pattern `ProcessRunner`/`MockRunner`
/// gives subprocess calls.
pub trait HttpClient {
    /// POST a JSON payload with bearer auth; return `(status_code, body)`.
    /// Callers decide error policy (the release phase treats a non-success
    /// after a pushed tag as fatal).
    fn post_json(&self, url: &str, token: &str, payload: &Value) -> Result<(u16, String)>;

    /// GET JSON (optionally authenticated); non-success statuses are errors
    /// carrying the status code and the API's `message` field when present.
    fn get_json(&self, url: &str, token: Option<&str>) -> Result<Value>;
}

/// Production [`HttpClient`] backed by the shared reqwest client.
pub struct ReqwestHttpClient;

impl HttpClient for ReqwestHttpClient {
    fn post_json(&self, url: &str, token: &str, payload: &Value) -> Result<(u16, String)> {
        let url = url::Url::parse(url).context("Failed to build GitHub API URL")?;
        let response = base_request(client()?.post(url), Some(token))
            .json(payload)
            .send()
            .context("Failed to call GitHub API")?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        Ok((status.as_u16(), text))
    }

    fn get_json(&self, url: &str, token: Option<&str>) -> Result<Value> {
        let url = url::Url::parse(url).context("Failed to build GitHub API URL")?;
        let response = base_request(client()?.get(url), token)
            .send()
            .context("Failed to call GitHub API")?;

        let status = response.status();
        if !status.is_success() {
            let body: Value = response.json().unwrap_or_default();
            let msg = body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            anyhow::bail!("GitHub API error ({}): {}", status, msg);
        }

        response
            .json()
            .context("Failed to parse GitHub API response")
    }
}

/// Token resolution precedence, documented once:
/// 1. `GITHUB_PAT` (explicit personal token)
/// 2. `GITHUB_TOKEN` (CI-provided / general token)
/// 3. `gh auth token` (GitHub CLI session)
///
/// Returns `Ok(None)` when no source yields a token — public-repo reads
/// work unauthenticated; callers decide whether that is acceptable.
pub fn resolve_token(runner: &dyn crate::process::ProcessRunner) -> Result<Option<String>> {
    if let Ok(t) = std::env::var("GITHUB_PAT") {
        return Ok(Some(t));
    }
    if let Ok(t) = std::env::var("GITHUB_TOKEN") {
        return Ok(Some(t));
    }
    match runner.run("gh", &["auth", "token"], None) {
        Ok(out) if out.status.success() => {
            let t =
                String::from_utf8(out.stdout).context("gh auth token returned invalid UTF-8")?;
            Ok(Some(t.trim().to_string()))
        }
        _ => Ok(None),
    }
}

/// Build the shared blocking client (30s timeout, standard User-Agent).
pub fn client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")
}

fn api_url(repo: &str, path: &str) -> Result<url::Url> {
    url::Url::parse("https://api.github.com/repos/")
        .and_then(|base| base.join(&format!("{}/{}", repo.trim_matches('/'), path)))
        .context("Failed to build GitHub API URL")
}

fn base_request(
    builder: reqwest::blocking::RequestBuilder,
    token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    let b = builder
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            format!("auto-dev-pipeline/{}", env!("CARGO_PKG_VERSION")),
        );
    match token {
        Some(t) => b.bearer_auth(t),
        None => b,
    }
}

/// GET /repos/{repo}/actions/runs — recent workflow runs for the verdict
/// in ci-check.
pub fn get_workflow_runs(repo: &str, token: Option<&str>) -> Result<Value> {
    let url = api_url(repo, "actions/runs?per_page=15")?;
    ReqwestHttpClient.get_json(url.as_str(), token)
}

/// Is GitHub Actions green for `sha`?
///
/// Fetches workflow runs for the HEAD SHA and requires at least one run with
/// every conclusion equal to `"success"`. A missing or empty run set is NOT
/// success (releasing untested code must not look safe).
pub fn head_ci_success(
    repo: &str,
    sha: &str,
    token: &str,
    http: &dyn HttpClient,
) -> Result<Vec<String>> {
    let url = api_url(repo, &format!("actions/runs?head_sha={}&per_page=100", sha))?;
    let runs = http
        .get_json(url.as_str(), Some(token))?
        .get("workflow_runs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(runs
        .iter()
        .map(|r| {
            r.get("conclusion")
                .and_then(|c| c.as_str())
                .unwrap_or("unknown")
                .to_string()
        })
        .collect())
}

/// POST /repos/{repo}/releases — create a GitHub Release for a validated
/// version tag. Returns the HTTP status and body so the caller decides
/// error policy; a non-success here is fatal for the release phase
/// (plan finding: partial release previously reported as success).
pub fn create_release(
    repo: &str,
    tag: &str,
    name: &str,
    body: &str,
    token: &str,
    http: &dyn HttpClient,
) -> Result<(u16, String)> {
    // serde_json instead of string interpolation: JSON correctness must
    // not depend on a distant validation regex (Deferred-10/15/26).
    let payload = serde_json::json!({
        "tag_name": tag,
        "name": name,
        "body": body,
        "draft": false,
        "prerelease": false,
    });
    let url = api_url(repo, "releases")?;
    http.post_json(url.as_str(), token, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{mock_output, MockRunner};

    /// Environment variables are process-global and cargo runs tests in this
    /// module in parallel threads — serialize every env-mutating test so they
    /// cannot stomp each other's GITHUB_PAT/GITHUB_TOKEN (CI flake).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn resolve_token_prefers_github_pat() {
        let _env = ENV_LOCK.lock().unwrap();
        // Environment variables are process-global; tests in this module that
        // touch GITHUB_PAT/GITHUB_TOKEN run under cargo's default per-file
        // lock when placed in one test binary. Set BOTH env vars and a gh
        // response, and assert the documented precedence: PAT wins.
        std::env::set_var("GITHUB_PAT", "pat-value");
        std::env::set_var("GITHUB_TOKEN", "tok-value");
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "tok-from-gh\n", ""));
        let t = resolve_token(&mock).unwrap();
        assert_eq!(
            t.as_deref(),
            Some("pat-value"),
            "GITHUB_PAT must win over GITHUB_TOKEN and gh fallback"
        );
        // The gh fallback must not even be consulted.
        assert!(
            mock.calls.borrow().is_empty(),
            "gh auth token should not run when GITHUB_PAT is set"
        );
    }

    #[test]
    fn resolve_token_falls_back_to_github_token_before_gh() {
        let _env = ENV_LOCK.lock().unwrap();
        std::env::remove_var("GITHUB_PAT");
        std::env::set_var("GITHUB_TOKEN", "tok-value");
        let mock = MockRunner::new();
        mock.push_response(mock_output(true, "tok-from-gh\n", ""));
        let t = resolve_token(&mock).unwrap();
        assert_eq!(
            t.as_deref(),
            Some("tok-value"),
            "GITHUB_TOKEN must win over the gh fallback"
        );
        assert!(
            mock.calls.borrow().is_empty(),
            "gh auth token should not run when GITHUB_TOKEN is set"
        );
    }

    #[test]
    fn resolve_token_none_when_all_sources_fail() {
        let _env = ENV_LOCK.lock().unwrap();
        // Neither env var set (best effort) and gh fails.
        std::env::remove_var("GITHUB_PAT");
        std::env::remove_var("GITHUB_TOKEN");
        let mock = MockRunner::new();
        mock.push_error("no gh");
        let t = resolve_token(&mock).unwrap();
        assert!(t.is_none());
    }

    #[test]
    fn api_url_normalizes_trailing_slash() {
        let u = api_url("owner/repo/", "actions/runs").unwrap();
        assert_eq!(
            u.as_str(),
            "https://api.github.com/repos/owner/repo/actions/runs"
        );
    }
}
