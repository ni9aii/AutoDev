# Hermes delegate_task Integration for Auto-Dev Pipeline

## Problem

The pipeline needs to dispatch reviewers and execute fixes. Real implementation requires:
1. Dispatching 4 subagents via Hermes `delegate_task` for review
2. Executing fixes via Claude Code print mode (or Hermes direct patch for simple fixes)

## Integration Pattern

### Phase 1: Review (Parallel Subagents)

Each reviewer is a separate `delegate_task` call with the review prompt loaded from `templates/review-prompts/{role}.md`:

```python
# Code Reviewer
delegate_task(
    goal="Perform code review on the project",
    context="""
    PROJECT_PATH: /path/to/project
    REVIEW_PROMPT: (full content of templates/review-prompts/code.md)
    
    Read the source files, analyze for bugs/style/edge cases/tests.
    Output findings in the format specified in the prompt.
    Save report to: ~/dev-notes/<project>/reviews/YYYY-MM-DD-code-review-report.md
    """,
    toolsets=['file', 'search_files']
)

# Security Reviewer (parallel)
delegate_task(
    goal="Perform security review",
    context="... security.md prompt ...",
    toolsets=['file', 'terminal']
)

# Architecture Reviewer (parallel)
delegate_task(
    goal="Perform architecture review",
    context="... architecture.md prompt ...",
    toolsets=['file', 'search_files']
)

# DevOps Reviewer (parallel)
delegate_task(
    goal="Perform DevOps review",
    context="... devops.md prompt ...",
    toolsets=['file', 'terminal']
)
```

**Note:** Launch 3 reviewers first, then the 4th separately (max_concurrent_children=3).

### Phase 2: Aggregate + Classify

After all 4 subagents complete, run `review-aggregator`:

```bash
review-aggregator \
    --input-dir ~/dev-notes/<project>/reviews/ \
    --output ~/dev-notes/<project>/plans/YYYY-MM-DD-plan.md
```

The aggregator:
- Parses all review reports
- Classifies findings as "Do Now" or "Defer"
- Generates prioritized fix plan

### Phase 3: Execute (Claude Code or Hermes)

**For simple fixes (1-2 files, <20 lines):**
Use Hermes directly:
```
read_file(path="/path/to/file")
patch(path="/path/to/file", old_string="...", new_string="...")
```

**For complex tasks (3+ files, refactors, CI fixes):**
Use Claude Code print mode:
```bash
claude -p "Fix buffer overflow in JSON parser at components/http_server/http_server.c:42. Add input validation." \
  --allowedTools "Read,Edit,Bash" \
  --max-turns 15 \
  --dangerously-skip-permissions
```

**For analysis-only (review without editing):**
```bash
claude -p "Review all changes in src/ for security issues. Output findings as JSON." \
  --allowedTools "Read" \
  --max-turns 10
```

**Selection criteria:**
- `files <= 2 && lines <= 20` → Hermes direct patch
- Everything else → Claude Code print mode

### Phase 4: Verify CI

```bash
ci-check /path/to/project
```

### Phase 5: Release

```bash
# Create tag
git tag -a vX.Y.Z -m "Phase N: description"
git push origin vX.Y.Z

# Create GitHub Release (via API or gh CLI)
# Update Obsidian vault status
```

### Phase 6: Loop Decision

```python
if all_reviews_empty() and ci_passed:
    exit_success()
else:
    iteration += 1
    if iteration > max_iterations:
        exit_max_iterations_reached()
    else:
        goto_phase_1()
```

## Implementation Status

- ✅ SKILL.md with workflow documentation
- ✅ 4 reviewer prompt templates
- ✅ review-aggregator (Rust, with Do/Defer classification)
- ✅ ci-check (Rust, GitHub Actions API check)
- ✅ run-pipeline (Rust, orchestrates all phases)
- ✅ Claude Code as primary executor (print mode)
- ✅ Release phase (tag, GitHub Release, Obsidian update)
- ⚠️ delegate_task integration — requires Hermes subagent dispatch
- ⚠️ Rust binaries not yet compiled — requires `cargo`

## Next Steps

1. Install rustup: `sudo pacman -S rustup`
2. Build: `cargo build --release`
3. Test on fresnel-beacon
4. Update `~/.hermes/config.yaml` quick_commands

## User Preference Notes

- User prefers "строго по одному пункту" execution (one task at a time)
- User wants reports saved to dev-notes repo (~/dev-notes/<project>/reviews/ and ~/dev-notes/<project>/plans/)
- User uses Claude Code for all delegation (not Kimi CLI)
- User splits by role: Hermes = planning/decisions, Claude Code = execution
- Max iterations should be configurable (default: 5)
- Project repo must .gitignore .hermes/ and graphify-out/
- Git sync required before starting: git pull --rebase origin main
