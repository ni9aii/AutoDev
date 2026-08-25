//! Shared dev-notes directory layout for all binaries.
//!
//! Every binary that reads or writes dogfooding artifacts joins the same
//! path segments (`<root>/<project>/reviews`, `.../plans`, `.../ci-reports`)
//! onto its own dev-notes root. Centralizing that construction here keeps
//! the layout contract in exactly one place.

use std::path::{Path, PathBuf};

/// Resolved dev-notes directories for one project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevNotesPaths {
    /// `<root>/<project>/reviews`
    pub reviews: PathBuf,
    /// `<root>/<project>/plans`
    pub plans: PathBuf,
    /// `<root>/<project>/ci-reports`
    pub ci_reports: PathBuf,
}

/// Build the dev-notes directory layout for `project` under `root`.
pub fn paths(root: &Path, project: &str) -> DevNotesPaths {
    let base = root.join(project);
    DevNotesPaths {
        reviews: base.join("reviews"),
        plans: base.join("plans"),
        ci_reports: base.join("ci-reports"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_all_three_directories_under_project() {
        let p = paths(Path::new("/tmp/notes"), "AutoDev");
        assert_eq!(p.reviews, PathBuf::from("/tmp/notes/AutoDev/reviews"));
        assert_eq!(p.plans, PathBuf::from("/tmp/notes/AutoDev/plans"));
        assert_eq!(p.ci_reports, PathBuf::from("/tmp/notes/AutoDev/ci-reports"));
    }

    #[test]
    fn paths_are_independent_per_project() {
        let a = paths(Path::new("/n"), "a");
        let b = paths(Path::new("/n"), "b");
        assert_ne!(a.reviews, b.reviews);
        assert!(a.plans.starts_with(Path::new("/n/a")));
    }

    #[test]
    fn relative_root_stays_relative() {
        // Compare as Paths, not strings: separators are platform-specific
        // (backslash on Windows) and must not leak into assertions.
        let p = paths(Path::new("notes"), "x");
        assert_eq!(p.reviews, PathBuf::from("notes").join("x").join("reviews"));
    }
}
