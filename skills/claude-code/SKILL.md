---
name: autodev
description: "AutoDev: automated review → plan → execute → verify pipeline for Claude Code"
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

Invoke from Claude Code with `/autodev [review|plan|execute|full] <project-name> [project-path]`.
- `project-name` — matches the directory under `dev-notes`.
- `project-path` — path to the repo (defaults to the current directory).

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

# Release
run-pipeline /path/to/project release --release-version v0.6.0
```

## Build (optional)

```bash
cargo build --release
```

Binaries install to `target/release/`. `cargo install --path .` puts
`run-pipeline`, `review-aggregator`, `ci-check` on your `PATH`.

## Project Structure

<!-- STRUCTURE:BEGIN -->
```text
.
├── bin_contract.rs
├── git.rs
├── github.rs
├── lib.rs
├── log.rs
├── markdown.rs
├── plan.rs
├── process.rs
├── severity.rs
├── test_runner.rs
├── validation.rs
├── bin/
│   ├── ci_check/
│   │   ├── checks.rs
│   │   ├── main.rs
│   │   ├── report.rs
│   ├── review_aggregator/
│   │   ├── findings.rs
│   │   ├── main.rs
│   │   ├── parse.rs
│   │   ├── plan.rs
│   ├── run_pipeline/
│   │   ├── main.rs
├── aggregator.rs
├── release.rs
├── run_pipeline.rs
├── common/
│   ├── mod.rs
├── gen-structure.sh
├── gen.sh
├── workflows/
│   ├── ci.yml
│   ├── release.yml
├── claude-code/
│   ├── SKILL.md
├── hermes/
│   ├── SKILL.md
├── claude-code.overlay
├── generic.overlay
├── hermes.overlay
├── agents-md-bootstrap.md
├── dev-notes-schema.md
├── git-sync-checklist.md
├── hermes-delegate-task-integration.md
├── iteration-2-patterns.md
├── json-output.md
├── plan-autodev-integration.md
├── rust-conventions.md
├── skill-walkthrough.md
├── troubleshooting.md
├── Cargo.toml
├── SKILL.core.md
├── README.md
├── AGENTS.md
├── install.sh
├── renovate.json
```
<!-- STRUCTURE:END -->

## CI

GitHub Actions with an ubuntu + windows matrix:
`cargo test` → `cargo clippy -- -D warnings` → `cargo build --release`.

## Phases

### `review` — reviewers

**Phase-0 (before launching reviewers):** check the project root for
`AGENTS.md`. If absent, generate it from the template in
`references/agents-md-bootstrap.md` and commit it as the first fix. If
present, read it — its rules override `references/rust-conventions.md`.

Launch the four reviewers (code, security, architecture, devops) as **4
parallel sub-agents via the Workflow tool**. Each reads the sources and writes
its report to:

$DEV_NOTES_ROOT/<project>/reviews/<YYYYMMDD_HHMMSS>/<role>-review.md

Prompt each sub-agent:

You are the <role> Reviewer. Check the project at <project-path>.
Read all source files. Write the report in this format:

### [CRITICAL] Title
Description. File: `path/to/file.rs`. Line: 42.
### [IMPORTANT] ...
### [MINOR] ...

Save to: <output-path>

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

To carry unresolved "Defer to Next Phase" items from the previous run into the
new plan, point `--carry-over-from` at that plan:

```bash
review-aggregator \
  --dev-notes \
  --project <project-name> \
  --dev-notes-root $DEV_NOTES_ROOT \
  --carry-over-from $DEV_NOTES_ROOT/<project>/plans/<prev-ts>-plan.md
```

Carried items appear at the top of the Defer section with an attempt counter;
items carried 3+ times are flagged "⚠️ WONTFIX candidate — requires human
decision". A missing or unparseable file is skipped with a warning.

Result: a plan in `$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md` with
"Do Now" and "Defer" sections.

### `execute` — apply fixes

Read the latest plan from `$DEV_NOTES_ROOT/<project>/plans/`. For each fix in the
"Do Now" section:

- **Simple fixes** (≤2 files, ≤20 lines): apply directly with Read/Edit.
- **Complex fixes**: delegate to an `Agent` sub-agent.
  Commit after each fix; push promptly. Do NOT add a `Co-Authored-By: Claude`
  trailer to commits.

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
directory (currently: `~/.claude/skills/autodev/ (via install.sh or copy SKILL.md there)`).