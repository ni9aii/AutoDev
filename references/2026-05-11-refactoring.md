# Auto-Dev Pipeline Refactoring — 2026-05-11

## Decision: Remove Kimi CLI, Use Claude Code as Primary Executor

**Rationale:** Kimi CLI was prone to looping (50+ identical terminal calls) and required interactive terminal delegation. Claude Code print mode (`-p`) is more reliable — no PTY needed, no dialog handling, exits when done.

## Architecture Changes

### Before (v1)
- Kimi CLI as primary executor
- Flat plan (all findings in one list)
- 4 phases: Review → Aggregate → Execute → Verify

### After (v2)
- Claude Code print mode as primary executor
- Hermes direct patch for simple fixes (1-2 files, <20 lines)
- Do/Defer classification in aggregator
- 6 phases: Review → Aggregate → Execute → Verify → Release → Loop

## Executor Selection

| Criteria | Executor |
|----------|----------|
| 1-2 files, <20 lines, obvious patch | Hermes (read_file + patch) |
| 3+ files, refactors, CI fixes, general coding | Claude Code print mode |

## Claude Code Invocation Pattern

```bash
# Standard task
claude -p "task description" \
  --allowedTools "Read,Edit,Bash" \
  --max-turns 15 \
  --dangerously-skip-permissions

# Analysis only (review)
claude -p "analyze code for issues" \
  --allowedTools "Read" \
  --max-turns 10

# Complex multi-file work
claude -p "refactor X to use Y pattern" \
  --allowedTools "Read,Edit,Bash" \
  --max-turns 30 \
  --dangerously-skip-permissions
```

## Do/Defer Classification

**do_now:** CRITICAL/IMPORTANT + specific file + no "refactor/architecture/cross-module/redesign" in body
**defer:** Everything else

Effort estimation:
- **low:** Simple fix, 1 file, <10 lines
- **medium:** CRITICAL or security-related
- **high:** Contains "refactor", "architecture", or "redesign"

## Release Phase

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin vX.Y.Z
```

## Rust Scripts Status

Rewritten from bash/python to Rust:
- `src/bin/run_pipeline.rs` — orchestrator with 6 phases
- `src/bin/review_aggregator.rs` — aggregation + Do/Defer classification
- `src/bin/ci_check.rs` — GitHub Actions CI status check

**Status:** Code written but not compiled — requires `sudo pacman -S rustup`
