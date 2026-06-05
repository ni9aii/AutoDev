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

## Two Modes of Operation

### Hermes Mode (MVP, default going forward)

All tasks execute via Hermes Agent native tools:
- Reviews: `delegate_task` subagents (4 parallel reviewers)
- Simple fixes: `read_file` + `patch`
- Complex fixes: `delegate_task` subagents
- Aggregation: `review-aggregator --dev-notes`
- Verification: `ci-check --dev-notes`

No external harnesses required. Rust binaries are optional accelerators.

### Legacy Mode

Uses Claude Code CLI (`claude -p`) for reviews and execution.
Requires `npm install -g @anthropic-ai/claude-code`.

## Build

```bash
cargo build --release
```

Binaries install to `target/release/`.

## Usage

### Hermes Mode

```bash
# Full pipeline
run-pipeline /path/to/project full --hermes-mode --project myproject

# Review only
run-pipeline /path/to/project review --hermes-mode --project myproject

# Review + plan
run-pipeline /path/to/project plan --hermes-mode --project myproject

# Release (same in both modes)
run-pipeline /path/to/project release --version v0.2.0
```

### Legacy Mode

```bash
# Full pipeline
run-pipeline /path/to/project full

# Review only
run-pipeline /path/to/project review

# Review + plan
run-pipeline /path/to/project plan

# Release
run-pipeline /path/to/project release --version v0.2.0
```

### Direct binary usage

```bash
# Aggregation with dev-notes auto-paths
review-aggregator --dev-notes --project myproject

# CI check with dev-notes report
 ci-check /path/to/project --dev-notes --project myproject
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

## dev-notes Integration

When using `--dev-notes` flag:

```
~/dev-notes/
└── <project>/
    ├── reviews/
    │   └── YYYYMMDD_HHMMSS/
    │       ├── code-review.md
    │       ├── security-review.md
    │       ├── architecture-review.md
    │       └── devops-review.md
    ├── plans/
    │   └── YYYYMMDD_HHMMSS-plan.md
    └── ci-reports/
        └── YYYYMMDD_HHMMSS-ci-status.md
```

## References

| File | Purpose |
|------|---------|
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |
| `references/hermes-delegate-task-integration.md` | delegate_task subagent integration guide |
| `references/iteration-2-patterns.md` | Partial fix traps, regressions, edge cases from Iteration 2 reviews |
