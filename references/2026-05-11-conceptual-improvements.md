# Auto-Dev Pipeline — Conceptual Improvements (2026-05-11)

## Multi-Executor Pattern

Three executors, selected by task characteristics:

| Executor | When to use | How |
|----------|-------------|-----|
| **Hermes (direct)** | Simple fix (1-2 files, <20 lines) | `read_file` + `patch` |
| **Claude Code** | CI fixes, formatting, env-specific, complex refactors | `claude -p "task" --allowedTools "Edit,Read,Bash"` |
| **Kimi CLI** | General coding by spec, tests, simple fixes | `kimi -p "task" --print --no-interactive` |

Selection criteria:
- `files <= 2 && lines <= 20` → Hermes
- `ci_related || formatting || env_specific || complex_refactor` → Claude Code
- else → Kimi CLI

User confirmed: "Claude Code лучше справится со спецификой и экономит токены".

## Release Phase (Phase 5)

Add after Verify:
1. Create git tag (`git tag -a vX.Y.Z -m "..."`)
2. Generate release notes from CHANGELOG
3. Create GitHub Release via API
4. Attach firmware binary (if CI artifact exists)
5. Update Obsidian vault status

## Do/Defer Classification

After aggregation, split findings into:
- **Do Now**: Quick wins — low complexity, high value, no dependencies
- **Defer**: Architectural changes, cross-module refactors, new features

Present both lists to user for confirmation before executing.

## Phase-Based Workflow

Each phase has scope, acceptance criteria, and review strategy.

## Report Format Standardization

Enforce strict markdown schema or use JSON for reviewer output.

## Cross-Review Validation

After collecting all reviews:
1. Detect duplicates (same file + same issue)
2. Detect contradictions (one says "fix", another says "by design")
3. Present conflicts to user for resolution

## Pipeline Metrics

Track per-run: iterations, findings by severity/reviewer, executor usage, duration.
