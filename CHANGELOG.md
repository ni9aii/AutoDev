# Changelog

## [0.2.0] - 2026-05-15

### Fixed
- GITHUB_TOKEN interpolation in release curl command (CRITICAL)
- Release phase now runs verify before creating tag
- ci_check returns Err when no test runner found
- Removed regex look-ahead (not supported in Rust regex crate)
- Fixed clippy warnings: needless_borrow, deprecated shlex::quote
- Fixed review_aggregator path validation blocking output

### Added
- README.md with installation and usage instructions
- LICENSE (MIT)
- CHANGELOG.md
- .gitignore for Rust project
- Cargo.lock (was empty, now generated)
- reqwest timeout (30s) in ci_check
- --version CLI argument for release phase

### Changed
- CI workflow: merged redundant build-arch job, pacman -Syu, clippy -D warnings
- Replaced shlex::quote with try_quote
- Reviewer prompts now use structured file discovery (less context overflow)

## [0.1.0] - 2026-05-14

### Added
- Initial release
- 4 reviewer pipeline (Code, Security, Architecture, DevOps)
- Review aggregation with Do Now / Defer classification
- Execute phase with Claude Code delegation
- CI status checking via GitHub API
- Release phase (tag + GitHub Release)
