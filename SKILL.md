# AutoDev Pipeline — Agent Skill

This is the skill definition for AutoDev: a self-contained workflow your agent
runs for the review → plan → execute → verify → release cycle. Install it into
your harness (see README → "Install the skill into your harness"), then invoke
it and let your agent drive the pipeline with its own native tools.

## Source of Truth

This repository (`ni9aii/AutoDev`) is the canonical source for AutoDev pipeline
conventions: paths, invocation, and configuration. Other skill surfaces must
follow what's documented here:

- **Claude Code skill** — `skills/claude-code/SKILL.md` (in this repo)
- **Hermes skill** — `~/.hermes/skills/autonomous-ai-agents/autodev/SKILL.md`
  (lives in the separate `~/.hermes` repo, but its config/paths must match
  this doc)

Load the Hermes skill with: `/skill autodev`

## What This Skill Does

AutoDev is a workflow your agent runs to accelerate the review-fix-release
cycle. In the default Hermes mode it uses only your agent's native tools —
no external binaries required:

- **run-pipeline** — orchestrates the full pipeline (review → aggregate → execute → verify → release)
- **review-aggregator** — collects review findings, deduplicates, classifies as Do Now / Defer
- **ci-check** — checks GitHub Actions CI status and runs local tests

The three Rust binaries above are *optional accelerators* for the mechanical
steps; the skill itself runs end to end with agent-native capabilities.

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
Requires `npm install -g @anthropic-ai/claude-code` **and an authenticated
session**. `run-pipeline` runs a pre-flight `claude -p` auth check and fails
fast with a clear message if the CLI is missing or its OAuth session is
expired (see issue #1).

> When Claude Code auth is unavailable, use `--hermes-mode` — it runs the
> whole pipeline inside Hermes Agent and never calls `claude`.

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
run-pipeline /path/to/project release --release-version v0.5.0
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
run-pipeline /path/to/project release --release-version v0.5.0
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
| `DEV_NOTES_ROOT` | Optional | Root for `--dev-notes` paths (default: `~/obsidian-vault/dev-notes`; overridable via `--dev-notes-root`) |

## Project Structure

```
.
├── src/
│   ├── lib.rs                  # Shared modules: log, git, markdown, test_runner
│   └── bin/
│       ├── run_pipeline/       # Main pipeline entry point
│       │   ├── main.rs
│       │   └── phases/{review,aggregate,execute,release,verify}.rs
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

When using `--dev-notes` flag, reports are written under `$DEV_NOTES_ROOT`
(default `~/obsidian-vault/dev-notes`, overridable via `--dev-notes-root`):

```
$DEV_NOTES_ROOT/
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
| `references/skill-walkthrough.md` | Phase-by-phase view of what the skill does |
| `references/hermes-delegate-task-integration.md` | `delegate_task` subagent integration (current code) |
| `references/dev-notes-schema.md` | Exact dev-notes layout, artifact paths, finding format |
| `references/json-output.md` | `run-pipeline --json` output contract |
| `references/iteration-2-patterns.md` | Report parser patterns, Do Now/Defer, regression checklist |
| `references/troubleshooting.md` | FAQ: Claude auth, empty reviews, dev-notes not found |
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |
