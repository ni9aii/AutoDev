# Hermes `delegate_task` Integration for AutoDev

This guide describes how AutoDev's skill drives Hermes Agent's native
`delegate_task` capability to run the review → plan → execute → verify →
release pipeline. It reflects the **current** code (Rust binaries +
`run-pipeline`/`review-aggregator`/`ci-check`), not a future plan.

## Two execution modes

AutoDev runs in one of two modes. Harness integrators should target **Hermes
mode** — it never calls an external binary and works with your agent alone.

| Mode | Reviewers | Executor for fixes | Requires |
|------|-----------|-------------------|----------|
| **Hermes** (default, `--hermes-mode`) | `delegate_task` (4 parallel reviewers) | `read_file`+`patch` (simple) or `delegate_task` (complex) | Your agent only |
| Legacy | `claude -p` CLI | `claude -p` CLI | Claude Code CLI, authenticated |

Legacy mode is a fallback for agents that wrap Claude Code. It runs a pre-flight
`claude -p "reply with the single word: OK"` auth check and fails fast if the CLI
is missing or its OAuth session has expired (see issue #1). Prefer Hermes mode.

## Phase 1 — Review (parallel `delegate_task`)

In Hermes mode `run-pipeline review --hermes-mode` prints `delegate_task`
instructions for four reviewers. Each reviewer is its own subagent call. Launch
three first, then the fourth separately (Hermes `max_concurrent_children` is 3).

```python
# Code Reviewer
delegate_task(
    goal="Code Reviewer: check logic, style, idioms, performance",
    context=""""
    PROJECT_PATH: /path/to/project
    OUTPUT_PATH: $DEV_NOTES_ROOT/<project>/reviews/<timestamp>/code-review.md

    Read the source files, analyze for bugs/style/edge cases/tests.
    Save the report to OUTPUT_PATH (markdown).
    """,
    toolsets=['file', 'search_files', 'terminal']
)

# Security Reviewer (parallel)
delegate_task(
    goal="Security Reviewer: check vulnerabilities, unsafe code, secrets",
    context="""PROJECT_PATH: /path/to/project
    OUTPUT_PATH: $DEV_NOTES_ROOT/<project>/reviews/<timestamp>/security-review.md""",
    toolsets=['file', 'search_files', 'terminal']
)

# Architecture Reviewer (parallel)
delegate_task(
    goal="Architecture Reviewer: check structure, coupling, patterns",
    context="""PROJECT_PATH: /path/to/project
    OUTPUT_PATH: $DEV_NOTES_ROOT/<project>/reviews/<timestamp>/architecture-review.md""",
    toolsets=['file', 'search_files', 'terminal']
)

# DevOps Reviewer (launch after the first three complete)
delegate_task(
    goal="DevOps Reviewer: check CI/CD, dependencies, build, deploy",
    context="""PROJECT_PATH: /path/to/project
    OUTPUT_PATH: $DEV_NOTES_ROOT/<project>/reviews/<timestamp>/devops-review.md""",
    toolsets=['file', 'search_files', 'terminal']
)
```

> The exact prompt text printed by the binary is authoritative — copy it
> verbatim when driving the pipeline manually. The shape above is the contract.

## Phase 2 — Aggregate + classify

After all four reviewers finish, run `review-aggregator` (Rust binary or let
`run-pipeline plan` invoke it):

```bash
review-aggregator --dev-notes --project <project> [--dev-notes-root <root>]
```

- Auto-discovers the **latest** `reviews/<timestamp>/` directory
- Parses every `### [CRITICAL|IMPORTANT|MINOR] Title` finding (also accepts
  table-row and bullet-list formats)
- Strips parser-metadata lines (`File:`, `Description:`, `Line:`, `Source:`)
  from finding bodies so they are not duplicated in the plan
- Deduplicates by severity + title + file
- Classifies each finding as **Do Now** (simple, single-file) or **Defer**
- Writes the plan to `plans/<timestamp>-plan.md`

## Phase 3 — Execute

Read the plan's `## 🔴 Do Now (Quick Wins)` section and apply each fix:

**Simple fixes (≤2 files, ≤20 lines):** use the agent directly.

```
read_file(path="/path/to/file")
patch(path="/path/to/file", old_string="...", new_string="...")
```

**Complex fixes (3+ files, refactors, CI changes):** dispatch a `delegate_task`
with the fix title, severity, file, and description from the plan.

The Rust `run-pipeline` Hermes executor does exactly this split
(`execute_via_claude` is only used in legacy mode). When driving the skill
manually, follow the same heuristic:

- `files <= 2 && lines <= 20` → `read_file` + `patch`
- otherwise → `delegate_task`

## Phase 4 — Verify

```bash
ci-check /path/to/project --dev-notes --project <project>
```

Runs local tests and queries GitHub Actions CI status, writing a report to
`ci-reports/<timestamp>-ci-status.md`. Only proceed to release if CI is green.

## Phase 5 — Release

```bash
run-pipeline /path/to/project release --version vX.Y.Z
# — or manually —
git tag -a vX.Y.Z -m "Phase N: description"
git push origin vX.Y.Z
# create GitHub Release via gh / API
```

The release phase builds the binary, creates the tag, and opens the GitHub
Release. It requires a GitHub token (`GITHUB_TOKEN` / `GITHUB_PAT`).

## Loop decision

The skill repeats review → execute → verify until either:

- all reviews are empty **and** CI passes → done, or
- a configured max-iteration count is reached → stop and report.

Max iterations should be configurable in the harness (a reasonable default is 5).

## Notes for harness integrators

- **No terminal required.** Load the skill (`/skill autodev` in Hermes) and let
  the agent drive everything with native tools. The Rust binaries are
  *accelerators* for the mechanical steps, not a prerequisite.
- **`--json` for programmatic use.** `run-pipeline --json` prints a
  machine-readable summary on **stdout** and routes all human log output to
  **stderr**. Parse the JSON when wrapping the binary from your own code.
- **dev-notes root.** Defaults to `~/obsidian-vault/dev-notes`; override with
  `--dev-notes-root` or the `DEV_NOTES_ROOT` env var.
