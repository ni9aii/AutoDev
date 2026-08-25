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
/// dev-notes tree (`<root>/<project>/reviews/…`). This is a security
/// control — `--project` (and a derived repo name) is attacker-influenced
/// input that is joined onto the dev-notes root.
///
/// Uses an allowlist (`^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`) rather than a
/// blocklist: only alphanumeric/`.`/`_`/`-` names up to 64 chars starting
/// with an alphanumeric are accepted. This rejects path separators, `.`/`..`
/// traversal components, hidden names like `.git`, control characters and NUL,
/// Windows reserved device names (CON, NUL, AUX), trailing dots/spaces, empty
/// strings, and filesystem-hostile over-long names.
pub fn validate_project_name(name: &str) -> Result<(), String> {
    static PROJECT_NAME_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").expect("Invalid PROJECT_NAME_RE pattern")
    });
    // Windows reserved device names are not excluded by the character
    // allowlist alone, so reject them explicitly.
    static RESERVED: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
        [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
        ]
        .into_iter()
        .collect()
    });
    if !PROJECT_NAME_RE.is_match(name) {
        if name.is_empty() {
            return Err("Project name is empty".to_string());
        }
        return Err(format!(
            "Invalid project name '{}': must be 1-64 chars of [A-Za-z0-9._-] starting with an alphanumeric",
            name
        ));
    }
    // A trailing dot breaks Windows path handling (stripped by Win32 APIs).
    if name.ends_with('.') {
        return Err(format!(
            "Invalid project name '{}': must not end with a dot",
            name
        ));
    }
    if RESERVED.contains(&name.to_ascii_uppercase().as_str()) {
        return Err(format!(
            "Invalid project name '{}': reserved device name",
            name
        ));
    }
    Ok(())
}

/// Validate the AUTO_DEV_TIMESTAMP pin value before it is joined onto paths.
/// Must be exactly `YYYYMMDD_HHMMSS` (the format produced by
/// `chrono::Local::now().format("%Y%m%d_%H%M%S")`): digits-only segments
/// reject path separators, traversal components and arbitrary strings from
/// the environment.
pub fn validate_timestamp(ts: &str) -> Result<(), String> {
    static TIMESTAMP_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[0-9]{8}_[0-9]{6}$").expect("Invalid TIMESTAMP_RE pattern"));
    if !TIMESTAMP_RE.is_match(ts) {
        return Err(format!(
            "Invalid AUTO_DEV_TIMESTAMP '{}': expected YYYYMMDD_HHMMSS (e.g. 20260825_232131)",
            ts
        ));
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
    fn test_validate_timestamp() {
        assert!(validate_timestamp("20260825_232131").is_ok());
        assert!(validate_timestamp("19700101_000000").is_ok());

        // Traversal, separators, arbitrary strings all rejected.
        assert!(validate_timestamp("").is_err());
        assert!(validate_timestamp("../../etc").is_err());
        assert!(validate_timestamp("foo/bar").is_err());
        assert!(validate_timestamp("2026-08-25 23:21:31").is_err());
        assert!(validate_timestamp("20260825_23213").is_err()); // too short
        assert!(validate_timestamp("202608252_32131").is_err()); // wrong split
        assert!(validate_timestamp("20260825_232131 ").is_err()); // trailing space
        assert!(validate_timestamp("2026082a_232131").is_err()); // non-digit
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

    #[test]
    fn test_validate_project_name_allowlist_rejects_hostile_names() {
        // Dot-run and hidden names collide with filesystem metadata.
        assert!(validate_project_name("...").is_err());
        assert!(validate_project_name(".git").is_err());
        assert!(validate_project_name("..").is_err());
        assert!(validate_project_name(".").is_err());

        // Windows reserved device names.
        assert!(validate_project_name("CON").is_err());
        assert!(validate_project_name("NUL").is_err());
        assert!(validate_project_name("AUX").is_err());

        // Trailing dots/spaces break path handling on Windows.
        assert!(validate_project_name("proj.").is_err());
        assert!(validate_project_name("proj ").is_err());

        // Control characters / NUL-adjacent bytes.
        assert!(validate_project_name("pro\u{0}j").is_err());
        assert!(validate_project_name("pro\nj").is_err());

        // Over-long names (>64 chars) fail cleanly instead of at the FS layer.
        let long = "a".repeat(65);
        assert!(validate_project_name(&long).is_err());
        let max = "a".repeat(64);
        assert!(validate_project_name(&max).is_ok());
    }
}
