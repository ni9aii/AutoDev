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
