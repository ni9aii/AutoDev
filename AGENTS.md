# AGENTS.md — AutoDev (ni9aii/AutoDev)

## What this project is

AutoDev MVP: a self-improving review → plan → execute → verify → release
pipeline for AI coding agents. Rust workspace with three binaries
(`run-pipeline`, `review-aggregator`, `ci-check`) and one library (`src/lib.rs`).
Skill surfaces are generated: edit `SKILL.core.md` + `harnesses/*.overlay`,
never the rendered `SKILL.md` files; keep renders in sync with
`bash tools/gen.sh` (CI fails on drift).

## Non-negotiables

- No Python anywhere in this repo — scripts are bash or Rust. The user cannot
  debug Python.
- Every feature/fix ships with tests AND CI coverage; docs updated in the same PR.
- No `unwrap()`/`expect()` in non-test code paths reachable from user input.
- Skill-surface edits always go through SKILL.core.md + overlays + gen.sh.
- No real infrastructure data (hosts, IPs, usernames, key paths) in code or history.

## Build / test / lint commands

```bash
cargo build --locked
cargo test --locked            # unit + integration suite (lib, 3 bins, per-feature test files)
                               # current count: <!-- TEST-COUNT:BEGIN -->75<!-- TEST-COUNT:END -->
cargo clippy --locked -- -D warnings
cargo fmt --check
bash tools/gen.sh && bash tools/gen-structure.sh && git diff --exit-code  # skill + structure drift check
```

## Git conventions

- Branches `fix/*`, `feat/*`, `chore/*`; never push directly to main.
- Commits: imperative subject with `feat:`/`fix:`/`chore:` prefix, logical units.
- Dependency updates via Renovate only (sole dependency bot).
- Local git email must be ni9aii@users.noreply.github.com.

## Where good code lives

- `src/lib.rs` — thin crate root (module declarations only); each module lives
  in its own file (`src/log.rs`, `src/process.rs`, `src/git.rs`, ...). Error
  handling with thiserror/anyhow split, regex Lazy patterns. Module tests live
  in a `#[cfg(test)] mod tests` block inside the same module file.
- `src/bin/run_pipeline/phases/` and the sibling binary directories
  (`src/bin/review_aggregator/`, `src/bin/ci_check/`) — phase/module
  separation and dispatch patterns: thin `main.rs` plus focused submodules.
- `tests/` — per-feature integration tests (`aggregator.rs`, `run_pipeline.rs`,
  `release.rs` + shared fixtures in `tests/common/mod.rs`).

## Dogfooding note

AutoDev runs on itself. Review reports land in
`<dev-notes-root>/AutoDev/reviews/<timestamp>/` (4 files: code, security,
architecture, devops), plans in `<dev-notes-root>/AutoDev/plans/`. When you fix
something found by a dogfood run, mention the run's timestamp dir in the commit body.
