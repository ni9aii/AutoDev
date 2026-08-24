---
name: autodev
description: "AutoDev: review → plan → execute → verify → release pipeline for your agent"
version: 1.0.0
author: ni9aii
license: MIT
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
+ `harnesses/<h>.overlay` by `tools/gen.sh`; edit those, not the rendered output.

## Invocation

Load this SKILL.md into your harness (see README → "Install the skill into
your harness"), then invoke it so your agent drives the pipeline with its own
native tools.

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
# Full pipeline (agent-native inside the binary; delegate_task mode is the default)
run-pipeline /path/to/project full --project myproject

# Review only
run-pipeline /path/to/project review --project myproject

# Review + plan
run-pipeline /path/to/project plan --project myproject

# Release (same in both modes)
run-pipeline /path/to/project release --release-version v0.6.0
```

> **Legacy mode** (shells out to the `claude -p` CLI) exists for agents that
> wrap Claude Code; opt in with `--legacy-claude`. `run-pipeline` runs a
> pre-flight auth check and fails fast with a clear message if the CLI is
> missing or its OAuth session is expired. When Claude Code auth is
> unavailable, stay in the default delegate_task mode.

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
│   ├── lib.rs                  # Thin crate root; modules in src/*.rs
│   └── bin/
│       ├── run_pipeline/       # Main pipeline entry point
│       │   ├── main.rs
│       │   ├── phases/{review,aggregate,execute,release,verify}.rs
│       │   └── pipeline/{build,dispatch,prereqs}.rs
│       ├── review_aggregator/  # Review aggregation + plan generation
│       │   └── {main,parse,findings,plan}.rs
│       └── ci_check/           # CI status checker
│           └── {main,checks,report}.rs
├── references/                 # Design patterns and integration guides (single canonical copy)
├── skills/<harness>/references # Committed SYMLINK -> ../../references (created by tools/gen.sh)
├── .github/workflows/
│   ├── ci.yml                  # CI (ubuntu + windows matrix)
│   └── release.yml             # Tag-triggered release with 3 binaries
├── SKILL.core.md + harnesses/  # Skill source (rendered by tools/gen.sh)
├── Cargo.toml / Cargo.lock
├── README.md / CHANGELOG.md / LICENSE (MIT)
└── SKILL.md                    # Rendered (generic) skill — do not edit by hand
```

## CI

GitHub Actions with an ubuntu + windows matrix:
`cargo test` → `cargo clippy -- -D warnings` → `cargo build --release`.

## Phases

### `review` — reviewers

**Phase-0 (before launching reviewers):** check the project root for
`AGENTS.md`. If absent, generate it from the template in
`references/agents-md-bootstrap.md` and commit it as the first fix. If
present, read it — its rules override `references/rust-conventions.md`.

Run the four reviewers (code, security, architecture, devops) with your
harness's subagent / parallel-execution mechanism. Each reads the sources and
writes its report to:

$DEV_NOTES_ROOT/<project>/reviews/<YYYYMMDD_HHMMSS>/<role>-review.md

Finding format per reviewer:

### [CRITICAL] Title
Description. File: `path/to/file.rs`. Line: 42.

Each reviewer loads `references/rust-conventions.md` (the user's Rust quality
bar) and, when present, the project's `AGENTS.md`, and enforces both in
addition to its own role checklist.

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

- **Simple fixes** (≤2 files, ≤20 lines): apply directly with your harness's
  read/edit tools.
- **Complex fixes**: delegate to a subagent.
  Commit after each logical fix.

Before declaring any fix complete, walk the hard rules in
`references/rust-conventions.md` over your own diff.

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
| `DEV_NOTES_ROOT` | Optional | Root for `--dev-notes` paths (default: `~/Notes/dev-notes`; overridable via `--dev-notes-root`) |

## dev-notes Integration

When using `--dev-notes` flag, reports are written under `$DEV_NOTES_ROOT`
(default `~/Notes/dev-notes`, overridable via `--dev-notes-root`).

**Artifact path contract (single source of truth — reviewers MUST write to
this exact layout; the aggregator reads it):**

```text
$DEV_NOTES_ROOT/
└── <project>/
    ├── reviews/
    │   └── <YYYYMMDD_HHMMSS>/          ← one timestamped dir per run
    │       ├── code-review.md          ← <role>-review.md, flat inside the dir
    │       ├── security-review.md
    │       ├── architecture-review.md
    │       └── devops-review.md
    ├── plans/
    │   └── YYYYMMDD_HHMMSS-plan.md     ← aggregator picks the LATEST reviews/<ts>/ dir
    └── ci-reports/
        └── YYYYMMDD_HHMMSS-ci-status.md
```

Reviewer output files are named `<role>-review.md` (code, security,
architecture, devops) and live directly inside the run's timestamped
directory — never flat under `reviews/`, never with a date-suffixed filename.
The `plan` phase aggregates exactly the latest `<YYYYMMDD_HHMMSS>/`
subdirectory.

## References

| File | Purpose |
|------|---------|
| `references/skill-walkthrough.md` | Phase-by-phase view of what the skill does |
| `references/hermes-delegate-task-integration.md` | `delegate_task` subagent integration (Hermes) |
| `references/dev-notes-schema.md` | Exact dev-notes layout, artifact paths, finding format |
| `references/json-output.md` | `run-pipeline --json` output contract |
| `references/iteration-2-patterns.md` | Report parser patterns, Do Now/Defer, regression checklist |
| `references/rust-conventions.md` | The user's Rust quality bar: hard rules, style, process — reviewers and executors load this |
| `references/agents-md-bootstrap.md` | Phase-0 AGENTS.md protocol + template for target repos |
| `references/troubleshooting.md` | FAQ: Claude auth, empty reviews, dev-notes not found |
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |

## Install

This skill is distributed via the repo's `install.sh` (one command, auto-detects
your harness) or by copying the generated `SKILL.md` into your harness's skill
directory (currently: `any harness that can load SKILL.md + references/`).