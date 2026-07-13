# Changelog

## [Unreleased]

### Fixed (Known Limitations from 0.3.0)
- PATH hijack risk — all bare `Command::new("claude"/"cargo"/"git"/"gh"/...)`
  call sites replaced with a `ProcessRunner` trait (`SystemRunner` in
  production) backed by a hand-rolled `resolve_exe()` that resolves and
  canonicalizes the executable from `$PATH` before spawning it.
- `parse_fixes` fragile parser — `**Description:**` label matching is now
  case/whitespace-tolerant, and fix titles with no colon use the full
  remainder instead of silently falling back to `"Unknown"`.
- Pipeline "God object" — `src/bin/run_pipeline.rs` (739 lines, 15 methods,
  direct subprocess coupling) split into
  `src/bin/run_pipeline/{main.rs,phases/{review,aggregate,execute,release,verify}.rs}`;
  phases depend on `ProcessRunner`, not concrete `Command`, so they're
  unit-testable via `MockRunner` without spawning real processes.
- No release/deploy CI workflow — added `.github/workflows/release.yml`,
  triggered on `v*` tag push, builds release binaries and attaches them to
  the GitHub Release via `softprops/action-gh-release`.
- No Dependabot/Renovate — added `.github/dependabot.yml` (cargo +
  github-actions ecosystems, weekly).
- Default `dev-notes` root mismatch — `resolve_dev_notes_root()` in all three
  binaries (`run_pipeline`, `review_aggregator`, `ci_check`) now falls back to
  `~/obsidian-vault/dev-notes` instead of `~/dev-notes`, matching the
  documented `$DEV_NOTES_ROOT` canonical default. Previously without
  `DEV_NOTES_ROOT` set, binaries wrote reports to the wrong directory.
- Hardcoded version strings — replaced literal `"1.1.0"` (and the
  `"auto-dev-pipeline/1.0"` User-Agent) with `env!("CARGO_PKG_VERSION")` in
  all three binaries, so `--version` and logs track `Cargo.toml` automatically.
- Legacy-mode Claude auth pre-flight — `run-pipeline` now runs a `claude -p`
  smoke-test before invoking Claude Code CLI (review/execute phases). A
  present-but-unauthenticated CLI (e.g. expired OAuth) used to report success
  on `--version` yet fail deep inside the run; now it fails fast with a clear
  message and points to `--hermes-mode` as the workaround (issue #1).
  `run_review_phase_legacy`/`execute_via_claude` now bail on non-zero Claude
  exit instead of only warning.

### Added
- Unit tests for `check_claude_auth`: authenticated OK, expired-OAuth error,
  missing-binary error (via `MockRunner`).

### Changed
- Docs reconciliation: `ni9aii/AutoDev` declared single source of truth for
  pipeline conventions (paths, invocation, config). Replaced hardcoded
  `~/dev-notes` example paths with `$DEV_NOTES_ROOT` (canonical default:
  `~/obsidian-vault/dev-notes`) in `SKILL.md`, `README.md`, and
  `references/hermes-delegate-task-integration.md`. Added a "Source of Truth"
  section to root `SKILL.md` and a pointer note in
  `skills/claude-code/SKILL.md`. The Hermes skill
  (`~/.hermes/skills/autonomous-ai-agents/autodev/SKILL.md`, separate repo)
  updated to match and to reference this repo as canonical. Runtime fallback
  default is now `~/obsidian-vault/dev-notes` (see Fixed above).

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