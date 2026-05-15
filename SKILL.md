# AutoDev Pipeline — Project Documentation

This document describes the AutoDev project structure and its components.

For the Hermes skill that orchestrates the pipeline, see:
`~/.hermes/skills/autonomous-ai-agents/autodev/SKILL.md`

Load with: `/skill autodev`

## What Is This

AutoDev is an automated development pipeline toolchain written in Rust.
It provides three binaries that accelerate the review-fix-release cycle:

- **run-pipeline** — orchestrates the full pipeline (review → aggregate → execute → verify → release)
- **review-aggregator** — collects review findings, deduplicates, classifies as Do Now / Defer
- **ci-check** — checks GitHub Actions CI status and runs local tests

The binaries are **optional accelerators**. The pipeline can run entirely through Hermes tools
(`delegate_task`, `terminal`, `patch`) as described in the skill.

## Build

```bash
cargo build --release
```

Binaries install to `target/release/`.

## Usage

```bash
# Full pipeline
run-pipeline /path/to/project full

# Review only
run-pipeline /path/to/project review

# Review + plan
run-pipeline /path/to/project plan

# Release (runs verify first, then tags and creates GitHub Release)
run-pipeline /path/to/project release --version v0.2.0
```

## Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `GITHUB_TOKEN` or `GITHUB_PAT` | For CI check and releases | GitHub API authentication |
| `AUTO_DEV_VERSION` | Optional | Fallback version for release phase |

## Project Structure

```
.
├── src/
│   ├── lib.rs                  # Shared modules: log, git, markdown, test_runner
│   └── bin/
│       ├── run_pipeline.rs     # Main pipeline entry point
│       ├── ci_check.rs         # CI status checker
│       └── review_aggregator.rs # Review aggregation + plan generation
├── references/                 # Design patterns and integration guides
│   ├── git-sync-checklist.md
│   ├── hermes-delegate-task-integration.md
│   └── iteration-2-patterns.md
├── .github/workflows/
│   └── ci.yml                  # CI (Arch Linux container)
├── Cargo.toml
├── Cargo.lock
├── README.md                   # Project overview and usage guide
├── CHANGELOG.md                # Version history
├── LICENSE                     # MIT
└── SKILL.md                    # This file
```

## CI

The project uses GitHub Actions with an Arch Linux container.
Pipeline: `cargo test` → `cargo clippy -- -D warnings` → `cargo build --release`.

## References

| File | Purpose |
|------|---------|
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |
| `references/hermes-delegate-task-integration.md` | delegate_task subagent integration guide |
| `references/iteration-2-patterns.md` | Partial fix traps, regressions, edge cases from Iteration 2 reviews |
