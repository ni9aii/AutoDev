//! Shared helpers for the per-feature integration test targets.

use std::fs;
use std::path::PathBuf;

/// A self-cleaning temporary directory under the system temp dir.
pub(crate) struct TempDir {
    pub(crate) path: PathBuf,
}

impl TempDir {
    pub(crate) fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("autodev-it-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Fake review report in the format the aggregator's parser understands
/// (### [SEVERITY] Title + File:/Description: body). The body deliberately
/// repeats "File:" / "Description:" lead-ins to verify the aggregator strips
/// them from the description (no duplication in the generated plan), and uses a
/// multi-line description so we also catch the clean_body prefix-strip bug
/// (the first line after "Description:" must be preserved, not dropped).
#[allow(dead_code)]
pub(crate) const FAKE_REVIEW: &str = r#"# Code Review Report

### [CRITICAL] SQL injection in db.rs
File: `src/db.rs`
Description: User input concatenated into a query string without parameterization.
This second line must also survive aggregation.
File: `src/db.rs`
Description: This is a duplicate metadata line that must be stripped.

### [IMPORTANT] Missing error handling in main.rs
File: `src/main.rs`
Description: `unwrap()` on a fallible call can panic at runtime.
"#;
