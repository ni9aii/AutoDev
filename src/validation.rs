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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_version_valid() {
        assert!(validate_version("v0.1.0").is_ok());
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("v2.0.0-alpha").is_ok());
    }

    #[test]
    fn test_validate_version_invalid() {
        assert!(validate_version("").is_err());
        assert!(validate_version("; rm -rf /").is_err());
        assert!(validate_version("$(whoami)").is_err());
    }

    #[test]
    fn test_validate_project_name_blocks_traversal() {
        // Valid names pass.
        assert!(validate_project_name("AutoDev").is_ok());
        assert!(validate_project_name("my-project_1").is_ok());

        // Reject path traversal and separators (Fix 21 from dogfood review).
        assert!(validate_project_name("../escape").is_err());
        assert!(validate_project_name("foo/bar").is_err());
        assert!(validate_project_name("foo\\bar").is_err());
        assert!(validate_project_name("..\\escape").is_err());
        assert!(validate_project_name("..").is_err());
        assert!(validate_project_name(".").is_err());
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("  ").is_err());
    }
}
