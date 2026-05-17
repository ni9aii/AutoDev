# Changelog

## [0.3.0] - 2026-05-15

### Fixed (CRITICAL)
- GitHub token exposed in process list via `curl` — replaced with reqwest (token stays in memory)
- No Rust toolchain pinning — added rust-toolchain.toml (channel 1.95)
- CI checkout after toolchain install — reordered steps (checkout first)
- Duplicated test-runner logic — ci_check now uses lib::test_runner

### Fixed (IMPORTANT)
- Version string injection — added validation (semver-only: v0.1.0 or 1.0.0)
- API errors swallowed as Ok(false) — now propagate as Err
- CI failures silently discarded — now propagate as Err
- Regex recompiled on every call in git::get_repo_info — cached via once_cell::Lazy
- UTF-8 truncation panic risk — added safe_truncate() function

### Fixed (MINOR)
- Dead fields removed: files_affected, estimated_effort
- Dead functions removed: count_files, estimate_effort
- Dead regex removed: CODE_FILE_RE
- Dead comment removed: "Old severity-based sections (kept for compatibility)"
- TODO removed from Cargo.toml dev-dependencies
- Inconsistent indentation in Pipeline::new
- Extra blank lines removed
- Phase numbering: Release now labeled "PHASE 5"
- CI badge added to README
- .env.example added
- CONTRIBUTING.md added

### Known Limitations
- PATH hijack risk: bare Command::new("claude"), etc. Users must control PATH.
- parse_fixes: fragile markdown parser. Works with current aggregator format.
- Architectural debt: Pipeline God object (500 LOC), tight coupling via subprocess.
- No release/deploy CI workflow.
- No Dependabot/Renovate.

## [0.2.0] - 2026-05-15

### Fixed
- GITHUB_TOKEN interpolation in release curl command (CRITICAL)
- Add verify phase before release tag creation
- Fix ci_check to return Err when no test runner found

### Added
- README.md with installation and usage
- LICENSE (MIT)
- CHANGELOG.md

## [0.1.0] - 2026-05-11

Initial release. Basic pipeline: review → plan → execute → verify → release.