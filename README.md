# AutoDev Pipeline

[![CI](https://github.com/ni9aii/AutoDev/actions/workflows/ci.yml/badge.svg)](https://github.com/ni9aii/AutoDev/actions/workflows/ci.yml)

<p align="center">
  <img src="logo.png" alt="AutoDev logo" width="256">
</p>

**An AI-agent skill for the review → plan → execute → verify → release cycle.**
AutoDev is a self-contained workflow you drop into your own agent harness — not
a CLI app you drive by hand. Once installed, your agent gains a structured
pipeline it runs with its own native tools.

### Why AutoDev

Most "vibe coding" stops at a first working draft. AutoDev is built to take an
existing concept and **cycle it to done** — review → code → test (locally and
on CI) → repeat, until *you* decide the project quality is good enough to
release. Each loop tightens the code instead of shipping the lucky first pass.

What makes that reliable:

- **Loop until release-ready.** The pipeline re-runs review → execute → verify
  on every iteration, so defects found late still get fixed, not deferred to a
  "later" that never comes.
- **Local + CI, not just local.** Fixes are verified by your test suite *and*
  GitHub Actions, so a green local run can't hide a broken CI.
- **Reproducible, file-based trail.** Every review, plan, and CI report lands
  in `dev-notes/` as plain markdown — traceable, diffable, and git-friendly.
- **Multi-harness by design.** The skill is just `SKILL.md` + `references/`;
  any agent harness can load it. The bundled Rust scripts are *optional
  accelerators* for the mechanical steps (aggregation, CI status) — harnesses
  that don't run them still get the full workflow through the agent's own
  tools.

See [`examples/`](examples/) for a fully worked sample: four review reports →
a generated fix plan ([`examples/sample-project/plans/`](examples/sample-project/plans/))
and a machine-readable [`--json` summary](examples/json-output.json).

## Install the skill into your harness

The skill is the product. The fastest path is the one-command installer:

```bash
# From a checkout of this repo:
./install.sh                 # auto-detects your harness, installs there
./install.sh --harness hermes      # or force a specific harness
./install.sh --harness claude-code
./install.sh --list          # show supported harnesses + install paths
./install.sh --check         # verify an install without changing anything
```

`install.sh` re-renders the skill from `SKILL.core.md` + `harnesses/*.overlay`
(see "How the skill is built" below) and copies the right `SKILL.md` (with its
`references/`) into your harness's skill directory. Currently supported:

| Harness      | Install path                                      | Invoke with        |
|--------------|---------------------------------------------------|--------------------|
| Hermes       | `~/.hermes/skills/autonomous-ai-agents/autodev`  | `/skill autodev`  |
| Claude Code  | `~/.claude/skills/autodev`                        | `/autodev`         |

After install, load the skill in your agent and run a phase, e.g.
`/autodev /path/to/project review`.

### Manual install (alternative)

If you prefer not to run the script, copy the rendered skill by hand — the
installer does exactly this:

```bash
# Hermes
cp skills/hermes/SKILL.md ~/.hermes/skills/autonomous-ai-agents/autodev/SKILL.md
cp -r skills/hermes/references   ~/.hermes/skills/autonomous-ai-agents/autodev/

# Claude Code
cp skills/claude-code/SKILL.md ~/.claude/skills/autodev/SKILL.md
cp -r skills/claude-code/references ~/.claude/skills/autodev/
```

> **That's it.** There is nothing to "run" from a terminal to use AutoDev — you
> load the skill and let your agent drive it. The Rust binaries below are
> optional accelerators, not a prerequisite.

## How the skill is built

`SKILL.md` surfaces are **generated**, not hand-written, so they can't drift
across harnesses. The source of truth is:

- `SKILL.core.md` — the workflow body, with `{{PLACEHOLDER}}` markers.
- `harnesses/<h>.overlay` — frontmatter + per-harness text (invocation,
  how reviewers/executors map to that harness's tools).

`tools/gen.sh` (pure bash, no Python) renders `SKILL.md`,
`skills/hermes/SKILL.md`, and `skills/claude-code/SKILL.md`, each with a
self-contained `references/` copy. CI runs `gen.sh` and fails if a committed
surface ever diverges from the source (`gen-check` job). To rebuild locally:

```bash
bash tools/gen.sh
```

## How it works

| Layer | Role | Implementation |
|-------|------|----------------|
| **Skill** (`SKILL.md`) | Orchestration & decision-making for the whole pipeline | Agent-native |
| `delegate_task` | Parallel reviewers, complex fixes | Agent-native (Hermes) |
| `read_file` + `patch` | Simple fixes (≤2 files, ≤20 lines) | Agent-native |
| `review-aggregator` | Finding aggregation, dedupe, plan generation | Rust binary (optional) |
| `ci-check` | CI status + local test run | Rust binary (optional) |
| `run-pipeline` | Full phase orchestration (Hermes or legacy mode) | Rust binary (optional) |

In the default **Hermes mode** the entire pipeline executes with your agent's
own tools. The Rust binaries are *accelerators* for the heavier mechanical
steps (deduplicating findings across reviewers, hitting the GitHub API for CI
status) — you can use the skill without them, or add them when you want the
speedup.

### Two execution modes

| Mode | Executors | Requires |
|------|-----------|----------|
| **Hermes** (default) | `delegate_task` / `read_file`+`patch` | Your agent only |
| Legacy | shells out to the `claude -p` CLI | Claude Code CLI, authenticated |

Hermes mode is the integration target for harness users — it never invokes an
external binary, so it works regardless of any other tool's auth state. The
legacy mode is a fallback for agents that wrap Claude Code; it runs a pre-flight
auth check and fails fast with a clear message if `claude` is missing or its
OAuth session has expired (see issue #1).

## Rust binaries (optional accelerators)

Only relevant if you want the binary speedups. Build and install:

```bash
cargo build --release
cargo install --path .        # puts run-pipeline, review-aggregator, ci-check on PATH
```

`run-pipeline` also supports a `--json` flag that emits a machine-readable
summary (status, version, phase, mode, timestamp, output dir) on **stdout** with
all human log output routed to **stderr** — useful when your harness wraps the
binary and parses its result programmatically.

## dev-notes layout

AutoDev keeps its intermediate artifacts in a dev-notes tree (default
`~/obsidian-vault/dev-notes`, override via `--dev-notes-root` or the
`DEV_NOTES_ROOT` env var). This is where the skill writes reviews, plans, and
CI reports per project:

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

## Configuration

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` / `GITHUB_PAT` | GitHub API auth (CI checks, releases) |
| `DEV_NOTES_ROOT` | Root for dev-notes paths (default: `~/obsidian-vault/dev-notes`) |

## Project structure

```
.
├── src/
│   ├── lib.rs                  # Shared modules (log, git, markdown, test_runner)
│   └── bin/
│       ├── run_pipeline/       # Optional pipeline entry point
│       │   ├── main.rs
│       │   └── phases/{review,aggregate,execute,release,verify}.rs
│       ├── ci_check.rs         # Optional CI status checker
│       └── review_aggregator.rs # Optional aggregation + plan generation
├── skills/
│   └── claude-code/SKILL.md    # Claude Code skill surface
├── references/                 # Integration & pattern guides
├── .github/workflows/ci.yml    # CI (Arch Linux)
├── Cargo.toml / Cargo.lock
├── README.md
├── SKILL.md                    # The skill — primary integration artifact
└── CHANGELOG.md
```

## References

Deeper integration and pattern notes (not required to use the skill, but
useful when adapting it):

| File | Purpose |
|------|---------|
| `references/skill-walkthrough.md` | Phase-by-phase view of what the skill does |
| `references/hermes-delegate-task-integration.md` | `delegate_task` subagent integration (current code) |
| `references/dev-notes-schema.md` | Exact dev-notes layout, artifact paths, finding format |
| `references/json-output.md` | `run-pipeline --json` output contract |
| `references/iteration-2-patterns.md` | Report parser patterns, Do Now/Defer, regression checklist |
| `references/troubleshooting.md` | FAQ: Claude auth, empty reviews, dev-notes not found |
| `references/git-sync-checklist.md` | Pre/post-work git sync steps |

## License

MIT — see [LICENSE](LICENSE).
