---
{{FRONTMATTER}}
---

# AutoDev Pipeline — Agent Skill

This is the skill definition for AutoDev: a self-contained workflow your agent
runs for the review → plan → execute → verify → release cycle. Install it into
your harness, then invoke it and let your agent drive the pipeline with its own
native tools.

## Why AutoDev

Most "vibe coding" stops at a first working draft. AutoDev takes an existing
concept and **cycles it to done** — review → code → test (locally and on CI) →
repeat, until *you* judge the project quality good enough to release. Each loop
tightens the code instead of shipping the lucky first pass, and fixes are
verified by both your test suite and GitHub Actions.

The skill is harness-agnostic: it's `SKILL.md` + `references/`, loadable into
any agent. The bundled Rust scripts are *optional accelerators* for the
mechanical steps — harnesses that don't run them still get the full workflow
through the agent's own tools.

## Source of Truth

This repository (`ni9aii/AutoDev`) is the canonical source for AutoDev pipeline
conventions: paths, invocation, and configuration. Other skill surfaces must
follow what's documented here. This file is **generated** from `SKILL.core.md`
+ `harnesses/<h>.yaml` by `tools/gen.py`; edit those, not the rendered output.

## Invocation

{{INVOKE}}

## What This Skill Does

AutoDev is a workflow your agent runs to accelerate the review-fix-release
cycle. In the default agent-native mode it uses only your agent's native tools —
no external binaries required:

- **run-pipeline** — orchestrates the full pipeline (review → aggregate → execute → verify → release)
- **review-aggregator** — collects review findings, deduplicates, classifies as Do Now / Defer
- **ci-check** — checks GitHub Actions CI status and runs local tests

The three Rust binaries above are *optional accelerators* for the mechanical
steps; the skill itself runs end to end with agent-native capabilities.

## Two Modes of Operation

### Agent-native mode (default)

All tasks execute via your agent's native tools:

- Reviews: parallel subagents (4 reviewers)
- Simple fixes: read + edit
- Complex fixes: subagents
- Aggregation: `review-aggregator --dev-notes`
- Verification: `ci-check --dev-notes`

No external binaries required. This is the integration target for every harness
surface — it never invokes an external binary, so it works regardless of any
other tool's auth state.

### Rust-binary mode (optional accelerator)

If you installed the binaries (`cargo build --release`), `run-pipeline` can
orchestrate the whole pipeline for you:

```bash
# Full pipeline (agent-native inside the binary)
run-pipeline /path/to/project full --hermes-mode --project myproject

# Review only
run-pipeline /path/to/project review --hermes-mode --project myproject

# Review + plan
run-pipeline /path/to/project plan --hermes-mode --project myproject

# Release (same in both modes)
run-pipeline /path/to/project release --release-version v0.6.0
```

> **Legacy mode** (shells out to the `claude -p` CLI) exists for agents that
> wrap Claude Code. `run-pipeline` runs a pre-flight auth check and fails fast
> with a clear message if the CLI is missing or its OAuth session is expired.
> When Claude Code auth is unavailable, use `--hermes-mode`.

## Build (optional)

```bash
cargo build --release
```

Binaries install to `target/release/`. `cargo install --path .` puts
`run-pipeline`, `review-aggregator`, `ci-check` on your `PATH`.

## Project Structure

```text
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
├── .github/workflows/
│   ├── ci.yml                  # CI (ubuntu + windows matrix)
│   └── release.yml             # Tag-triggered release with 3 binaries
├── SKILL.core.md + harnesses/  # Skill source (rendered by tools/gen.py)
├── Cargo.toml / Cargo.lock
├── README.md / CHANGELOG.md / LICENSE (MIT)
└── SKILL.md                    # Rendered (generic) skill — do not edit by hand
```

## CI

GitHub Actions with an ubuntu + windows matrix:
`cargo test` → `cargo clippy -- -D warnings` → `cargo build --release`.

## Phases

### `review` — reviewers

{{REVIEWERS}}

### `plan` — aggregation

After all reviewers finish, run the aggregator **once** (not per-reviewer) to
produce a unified plan:

```bash
review-aggregator \
  --dev-notes \
  --project <project-name> \
  --dev-notes-root $DEV_NOTES_ROOT
```

Result: a plan in `$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md` with
"Do Now" and "Defer" sections.

### `execute` — apply fixes

Read the latest plan from `$DEV_NOTES_ROOT/<project>/plans/`. For each fix in the
"Do Now" section:

{{EXECUTE}}

### `full` — full pipeline

`review` → `plan` → `execute` → `verify` → (optionally) `release`.

### `verify` — check

```bash
ci-check <project-path> --dev-notes --project <project-name> --dev-notes-root $DEV_NOTES_ROOT
```

### `release`

Validate version, build release binary, tag, push, create GitHub Release. Ask the
user before pushing to main or creating a release.

## Environment Variables

| Variable | Required | Purpose |
|----------|----------|---------|
| `GITHUB_TOKEN` or `GITHUB_PAT` | For CI check and releases | GitHub API authentication |
| `DEV_NOTES_ROOT` | Optional | Root for `--dev-notes` paths (default: `~/obsidian-vault/dev-notes`; overridable via `--dev-notes-root`) |

## dev-notes Integration

When using `--dev-notes` flag, reports are written under `$DEV_NOTES_ROOT`
(default `~/obsidian-vault/dev-notes`, overridable via `--dev-notes-root`):

```text
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
| `references/hermes-delegate-task-integration.md` | `delegate_task` subagent integration (Hermes) |
| `references/dev-notes-schema.md` | Exact dev-notes layout, artifact paths, finding format |
| `references/json-output.md` | `run-pipeline --json` output contract |
| `references/iteration-2-patterns.md` | Report parser patterns, Do Now/Defer, regression checklist |
| `references/troubleshooting.md` | FAQ: Claude auth, empty reviews, dev-notes not found |
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |

## Install

This skill is distributed via the repo's `install.sh` (one command, auto-detects
your harness) or by copying the generated `SKILL.md` into your harness's skill
directory (currently: `{{INSTALL_PATH_HINT}}`).
