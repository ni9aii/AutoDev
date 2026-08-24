// Companion binary contract helpers extracted from lib.rs.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_companion_uses_exe_suffix() {
        let name = companion_exe_name("review-aggregator");
        assert!(name.ends_with(std::env::consts::EXE_SUFFIX));
        assert!(name.starts_with("review-aggregator"));
        assert_eq!(AGGREGATOR, "review-aggregator");
        assert_eq!(CI_CHECK, "ci-check");
    }

    #[test]
    fn test_aggregate_request_args_roundtrip() {
        let req = AggregateRequest {
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
}
