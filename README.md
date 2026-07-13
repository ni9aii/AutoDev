# AutoDev Pipeline

[![CI](https://github.com/ni9aii/AutoDev/actions/workflows/ci.yml/badge.svg)](https://github.com/ni9aii/AutoDev/actions/workflows/ci.yml)

Automated development pipeline: review → plan → execute → verify → release.

## Architecture (MVP)

**Hermes Agent (orchestrator) + delegate_task subagents (executors)**

All tasks run inside Hermes Agent. No external harnesses required.
Rust binaries are optional accelerators.

| Component | Role | Implementation |
|-----------|------|----------------|
| Hermes skill `autodev` | Orchestration, decision-making | `~/.hermes/skills/autonomous-ai-agents/autodev/` |
| `delegate_task` | Reviewers, complex fixes | Hermes native |
| `patch` / `read_file` | Simple fixes (≤2 files, ≤20 lines) | Hermes native |
| `review-aggregator` | Finding aggregation, plan generation | Rust binary |
| `ci-check` | CI status + local tests | Rust binary |
| `run-pipeline` | Phase orchestration (legacy + Hermes mode) | Rust binary |

## Quick Start

### Hermes Mode (MVP)

```bash
# Full pipeline with Hermes delegate_task
run-pipeline /path/to/project full --hermes-mode --project myproject

# Review only
run-pipeline /path/to/project review --hermes-mode --project myproject

# Review + plan
run-pipeline /path/to/project plan --hermes-mode --project myproject

# Release
run-pipeline /path/to/project release --version v0.2.0
```

### Legacy Mode (Claude Code CLI)

```bash
# Requires `claude` CLI installed
run-pipeline /path/to/project full
```

## Hermes Mode Workflow

### Phase 1 — Review

`run-pipeline --hermes-mode` prints delegate_task instructions for 4 parallel reviewers:

```python
delegate_task(
    goal="Code Reviewer: check logic, style, idioms, performance",
    context="""
    PROJECT_PATH: /path/to/project
    OUTPUT_PATH: $DEV_NOTES_ROOT/myproject/reviews/20260606_143022/code-review.md
    """,
    toolsets=['file', 'search_files', 'terminal']
)
```

Reports saved to `$DEV_NOTES_ROOT/<project>/reviews/<timestamp>/`.

### Phase 2 — Aggregate

```bash
review-aggregator --dev-notes --project myproject
```

Auto-discovers latest reviews, generates plan at `$DEV_NOTES_ROOT/<project>/plans/<timestamp>-plan.md`.

### Phase 3 — Execute

Hermes mode prints per-fix instructions:
- Simple fixes → `read_file` + `patch`
- Complex fixes → `delegate_task`

### Phase 4 — Verify

```bash
ci-check /path/to/project --dev-notes --project myproject
```

Saves report to `$DEV_NOTES_ROOT/<project>/ci-reports/<timestamp>-ci-status.md`.

### Phase 5 — Release

```bash
run-pipeline /path/to/project release --version v0.2.0
```

## Modes & Authentication

The pipeline has two execution modes:

| Mode | Flag | Executors | Requires |
|------|------|-----------|----------|
| Hermes (MVP, default going forward) | `--hermes-mode` | `delegate_task` / `read_file`+`patch` | Hermes Agent only |
| Legacy | _(none)_ | shells out to `claude -p` CLI | Claude Code CLI, authenticated |

**Legacy mode requires an authenticated Claude Code CLI.** Before any
`claude -p` call, `run-pipeline` runs a pre-flight auth check
(`claude -p "reply with the single word: OK" --max-turns 1`). If the CLI is
missing or its session is expired/unauthenticated, the pipeline fails fast
with a clear message instead of silently producing empty reviews:

```
[auto-dev] ERROR Claude Code CLI is installed but NOT authenticated.
[auto-dev] ERROR Re-authenticate with: claude (interactive login)
[auto-dev] ERROR Or use --hermes-mode for delegate_task-based execution (no Claude CLI needed).
```

> **Workaround when Claude Code auth is unavailable:** use `--hermes-mode`.
> It performs the entire pipeline (review → aggregate → execute → verify)
> inside Hermes Agent and never invokes the `claude` binary, so it works
> regardless of Claude Code's auth state. See issue #1.

## Directory Layout (dev-notes)

`$DEV_NOTES_ROOT` defaults to `~/obsidian-vault/dev-notes` (override via
`--dev-notes-root` or the `DEV_NOTES_ROOT` env var):

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

## Installation

```bash
# Clone
git clone https://github.com/ni9aii/AutoDev.git
cd AutoDev

# Build Rust binaries (optional but recommended)
cargo build --release

# Install to PATH
cargo install --path .
```

## Requirements

- Rust 1.70+ (for binaries)
- Hermes Agent (for orchestration)
- GitHub PAT (for CI checks and releases)

## Environment Variables

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` / `GITHUB_PAT` | GitHub API authentication |
| `AUTO_DEV_VERSION` | Fallback version for release phase |
| `DEV_NOTES_ROOT` | Root for `--dev-notes` paths (default: `~/obsidian-vault/dev-notes`) |

## Project Structure

```
.
├── src/
│   ├── lib.rs                  # Shared modules (log, git, markdown, test_runner)
│   └── bin/
│       ├── run_pipeline.rs     # Main pipeline entry point
│       ├── ci_check.rs         # CI status checker
│       └── review_aggregator.rs # Review aggregation + plan generation
├── .github/workflows/
│   └── ci.yml                  # CI configuration (Arch Linux)
├── Cargo.toml
├── README.md
├── CHANGELOG.md
└── LICENSE
```

## License

MIT License — see [LICENSE](LICENSE)
