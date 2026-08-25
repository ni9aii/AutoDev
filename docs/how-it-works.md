# How it works

AutoDev has two layers: an **agent-native skill** that does the thinking and
orchestration, and **optional Rust binaries** that accelerate the heavy
mechanical steps.

| Layer | Role | Implementation |
|-------|------|----------------|
| **Skill** (`SKILL.md`) | Orchestration & decision-making for the whole pipeline | Agent-native |
| `delegate_task` | Parallel reviewers, complex fixes | Agent-native (Hermes) |
| `read_file` + `patch` | Simple fixes (≤2 files, ≤20 lines) | Agent-native |
| `review-aggregator` | Finding aggregation, dedupe, plan generation | Rust binary (optional) |
| `ci-check` | CI status + local test run | Rust binary (optional) |
| `run-pipeline` | Full phase orchestration | Rust binary (optional) |

The pipeline is **agent-orchestrated**: reviews and fixes run as
`delegate_task` subagents (or `read_file`+`patch` for simple fixes) driven by
your agent. The Rust binaries are *accelerators* for the heavier mechanical
steps (deduplicating findings across reviewers, hitting the GitHub API for CI
status) — you can use the skill without them, or add them when you want the
speedup.

## Execution model

| Step | Executors | Requires |
|------|-----------|----------|
| Review / Execute | `delegate_task` / `read_file`+`patch` | Your agent only |
| Aggregate / Verify | `review-aggregator`, `ci-check` binaries | Rust toolchain |

The pipeline never invokes an external AI CLI, so it works regardless of any
other tool's auth state.

## Phases

1. **Review** — four parallel reviewers (code, security, architecture,
   devops) produce timestamped reports under `<project>/reviews/<ts>/`.
2. **Plan** (aggregate) — findings are deduplicated and classified into a
   Do Now / Defer plan: `<ts>-plan.md` (human artifact) plus
   `<ts>-plan.json` (machine-readable sidecar; authoritative for tools).
3. **Execute** — your agent fixes the Do Now items; instructions carry
   report data as untrusted quoted blocks.
4. **Verify** — local test suite + GitHub Actions must both pass.
5. **Release** — gated by pre-flight checks (clean tree, branch `main`,
   green CI on HEAD, Cargo.toml version == tag) and a curated CHANGELOG
   section, which becomes the GitHub Release body.

## The Rust binaries (optional accelerators)

```bash
cargo build --release
cargo install --path .        # puts run-pipeline, review-aggregator, ci-check on PATH
```

`run-pipeline` also supports a `--json` flag that emits a machine-readable
summary (status, version, phase, mode, timestamp, output dir) on **stdout**
with all human log output routed to **stderr** — useful when your harness
wraps the binary and parses its result programmatically. See
[references/json-output.md](../references/json-output.md).

## How the skill is built

One canonical source (`SKILL.core.md`) plus per-harness overlay files
(`harnesses/*.overlay`) render the per-harness surfaces:

```bash
bash tools/gen.sh             # re-render SKILL.md + skills/<harness>/SKILL.md
```

`tools/gen.sh` renders `SKILL.md`, `skills/hermes/SKILL.md`, and
`skills/claude-code/SKILL.md`, and creates or refreshes the per-harness
symlinks `skills/<harness>/references -> ../../references`, idempotently.
The repo keeps ONE canonical `references/` at the root — the skill dirs carry
only the link. CI runs `gen.sh` and fails if a committed surface ever
diverges from the source (`gen-check` job). Never edit rendered files
directly; edit `SKILL.core.md` or the overlays.
