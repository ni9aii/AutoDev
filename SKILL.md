---
name: auto-dev-pipeline
description: "Автоматическая цепочка разработки: ревью → план → исполнение → CI → повторение."
version: 1.0.0
author: Hermes Agent
license: MIT
metadata:
  hermes:
    tags: [automation, devops, ci-cd, review, delegation, pipeline]
    related_skills: [subagent-driven-development, writing-plans, requesting-code-review, claude-code]
---

# Auto-Dev Pipeline

## Overview

Автоматический пайплайн разработки, который:
1. Собирает ревью текущего состояния проекта от 4 экспертов
2. Готовит план исправлений с Do/Defer классификацией
3. Исправляет через Claude Code (print mode) или Hermes (простые фиксы)
4. Проверяет CI статус
5. Создаёт релиз (тег, GitHub Release)
6. Повторяет цикл при необходимости

## When to Use

- Перед релизом — финальная проверка качества
- После завершения фазы разработки
- Когда нужно систематически улучшить кодовую базу
- Для автоматического поддержания качества в долгосрочной перспективе

## Workflow

```
┌─────────────────────────────────────────────────────────────┐
│  1. REVIEW PHASE (Parallel)                                  │
│     ├── Code Reviewer      → code-review-report.md           │
│     ├── Security Reviewer  → security-review-report.md       │
│     ├── Architecture Reviewer → architecture-review-report.md  │
│     └── DevOps Reviewer    → devops-review-report.md         │
│                                                              │
│  2. AGGREGATE PHASE                                          │
│     ├── Collect all findings                                 │
│     ├── Classify: Do Now / Defer                             │
│     └── Generate fixes-plan.md                               │
│                                                              │
│  3. EXECUTE PHASE                                            │
│     ├── Simple fixes → Hermes (read_file + patch)            │
│     ├── Complex tasks → Claude Code print mode               │
│     └── Commit after each fix                                │
│                                                              │
│  4. VERIFY PHASE                                             │
│     ├── Run full test suite                                  │
│     ├── Check CI status (GitHub Actions)                     │
│     └── Verify no regressions                                │
│                                                              │
│  5. RELEASE PHASE                                            │
│     ├── Create git tag (vX.Y.Z)                              │
│     ├── Create GitHub Release                                │
│     └── Update Obsidian vault                                │
│                                                              │
│  6. LOOP OR EXIT                                             │
│     ├── If issues remain → back to REVIEW                    │
│     ├── If all clean → EXIT success                          │
│     └── Max iterations: 5                                  │
└─────────────────────────────────────────────────────────────┘
```

## Reviewers

### Code Reviewer
- **Focus**: Style, bugs, edge cases, test coverage
- **Tools**: `file`, `search_files`
- **Output**: Список issues с severity (critical/important/minor)

### Security Reviewer
- **Focus**: Secrets, injections, auth, dependencies, CVE
- **Tools**: `file`, `terminal` (grep secrets), `web` (CVE check)
- **Output**: Security findings with CVSS where applicable

### Architecture Reviewer
- **Focus**: Coupling, SOLID, patterns, tech debt, complexity
- **Tools**: `file`, `search_files`
- **Output**: Architecture issues with refactoring suggestions

### DevOps Reviewer
- **Focus**: CI/CD, Docker, configs, tooling, build optimization
- **Tools**: `file`, `terminal`
- **Output**: DevOps issues with configuration fixes

## Quick Commands

### `/auto-dev [project-path]`
Запустить полный пайплайн на проекте.

### `/auto-dev-review [project-path]`
Только фаза ревью (без исполнения).

### `/auto-dev-plan [project-path]`
Ревью + планирование (без исполнения).

### `/auto-dev-release [project-path] [version]`
Фаза релиза — создаёт тег и GitHub Release.

## Configuration

В `~/.hermes/config.yaml`:

```yaml
quick_commands:
  auto-dev:
    description: Run full auto-dev pipeline on a project
    command: ~/.hermes/skills/autonomous-ai-agents/auto-dev-pipeline/target/release/run-pipeline '{{cwd}}' full
    args:
      cwd:
        description: Project directory
        default: .
  auto-dev-review:
    description: Run review phase only
    command: ~/.hermes/skills/autonomous-ai-agents/auto-dev-pipeline/target/release/run-pipeline '{{cwd}}' review
    args:
      cwd:
        description: Project directory
        default: .
  auto-dev-plan:
    description: Run review + plan phases
    command: ~/.hermes/skills/autonomous-ai-agents/auto-dev-pipeline/target/release/run-pipeline '{{cwd}}' plan
    args:
      cwd:
        description: Project directory
        default: .
  auto-dev-release:
    description: Create release (tag + GitHub Release)
    command: AUTO_DEV_VERSION='{{version}}' ~/.hermes/skills/autonomous-ai-agents/auto-dev-pipeline/target/release/run-pipeline '{{cwd}}' release
    args:
      cwd:
        description: Project directory
        default: .
      version:
        description: Version tag (e.g., v0.2.0)
        default: v0.1.0
```

## Exit Criteria

Пайплайн завершается успешно, когда:
- Все 4 ревьюера вернули пустой список issues
- CI статус = success
- Или оператор явно запросил остановку

## Max Iterations

По умолчанию: 5 итераций.
Настраивается через переменную окружения `AUTO_DEV_MAX_ITERATIONS`.

## Output

Все отчёты сохраняются в dev-notes репо:
- `~/dev-notes/<project>/reviews/YYYY-MM-DD-{role}-review-report.md` — отчёты ревьюеров
- `~/dev-notes/<project>/plans/YYYY-MM-DD-plan.md` — план исправлений

Проектный репо должен `.gitignore` `.hermes/` чтобы не коммитить артефакты пайплайна.

## Pitfalls

- **Context overflow**: 4 ревьюера + планирование + исполнение = много контекста. Mitigation: каждый ревьюер — отдельный subagent со свежим контекстом.
- **max_concurrent_children limit**: Hermes `delegate_task` defaults to `max_concurrent_children=3`. Launching 4 reviewers in one call fails with "Too many tasks: 4 provided, but max_concurrent_children is 3". **Fix**: Launch 3 reviewers first, then the 4th separately. Alternatively, increase `delegation.max_concurrent_children` in `~/.hermes/config.yaml`.
- **Subagent terminal loops**: Reviewer subagents with `toolsets=['terminal']` sometimes loop on identical terminal calls (50+ iterations) and hit `max_iterations`. This happens when the subagent tries to verify findings via repeated grep/terminal probes instead of reading files. **Fix**: In reviewer prompts, explicitly instruct: "Read source files with read_file, do NOT run repeated terminal grep loops." Limit terminal use to one-time directory listing.
- **Kimi Code CLI subagent loops**: REMOVED — Kimi CLI is no longer used. Use Claude Code print mode instead.
- **Kimi Code CLI недоступен**: REMOVED — no longer a dependency.
- **CI token expired / public repo without auth**: `ci-check.sh` uses GitHub API without PAT for public repos, but API rate-limiting or IP blocks can return 403/404 even for public repos. **Fix**: Always set `GITHUB_PAT` env var. If PAT unavailable, skip CI check and rely on local test verification (`git status` + local build).
- **OpenCode не работает с Kimi**: REMOVED — no longer relevant. Use Claude Code for all delegation.
- **Git sync required**: Always run `git pull --rebase origin main` before starting work. Remote may have moved ahead (especially after CI commits or parallel work), causing push rejects and rebase conflicts mid-session.
- **Review artifacts in project repo**: `.hermes/` and `reviews/` directories must NOT be committed to the project repo. Use `.gitignore` for `.hermes/`, `graphify-out/`, `*.orig`, and any review directories. All review reports and plans go to `~/dev-notes/<project>/`.
- **Partial fix traps**: A fix that addresses the symptom but not the root cause leaves latent bugs. Example: capping buffer length with `sscanf` width but leaving `strtol` without error checking. Iteration 2 reviewers must re-verify the FULL attack surface of any partially-fixed issue.
- **Fix-induced regressions**: Patches can introduce new bugs. Example: adding `vSemaphoreDelete` in deinit but calling `led_driver_clear()` AFTER the mutex is gone (use-after-free). Always check the 3 lines before and 3 lines after every patch for ordering/synchronization issues.
- **Component manager CI blockers**: External ESP-IDF components (e.g., `espressif/mdns`) may not integrate with `esp-idf-ci-action`. After 2 failed attempts, defer the component to a later phase rather than blocking the entire iteration. See `esp-idf` skill `references/component-manager-ci-pitfall.md` for the decision framework.
- **sdkconfig in git despite .gitignore**: If `sdkconfig` was committed before `.gitignore` was added, `git rm --cached sdkconfig` is required. `.gitignore` alone does NOT remove tracked files. **Verification**: Always run `git ls-files | grep sdkconfig` before reporting this issue — the file may exist locally but not be tracked.
- **Self-corrected findings in aggregator**: Subagents sometimes discover a reported issue is a false alarm and write "Removing this entry" or "Downgrading" in their report. The naive regex aggregator still includes these. **Fix**: The aggregator should skip findings where the body contains self-correction markers like "Removing", "Downgrading", "false alarm", "not present".
- **Report format inconsistency across iterations**: Subagents may use different severity formats in v1 vs v2 reports (e.g. `### [CRITICAL] Title` vs table-based `| CRITICAL | Title |`). A regex expecting `### [SEV] Title` will return 0 findings for table-based reports. **Fix**: The aggregator must support multiple input formats: Markdown headers (`### [SEV]`), table rows (`| SEV | Title |`), and bullet lists (`- [SEV] Title`). See `references/iteration-2-patterns.md` for the robust parser pattern.
- **DevOps false positives on file existence**: Reviewers may report "sdkconfig committed in git" based on `ls` output alone. Always verify with `git ls-files` or `git status --short` before acting on "file tracked" claims.
- **Graphify timing**: Run `graphify update .` (AST-only, no LLM cost) BEFORE the review phase so reviewers see the current graph. Do NOT run it between review and execute — the graph won't affect execution.
- **"Do now / defer" prioritization**: After aggregation, classify each finding as (a) Do now — low complexity, high value, no architectural dependencies; (b) Defer to next phase — requires architectural changes, new features, or cross-module refactoring. Present both lists to the user for confirmation before executing. This prevents scope creep during polish iterations.
- **clang-format version mismatch in CI**: CI "Check formatting" fails even after `clang-format -i` passes locally. Local v22 vs CI v18 (Ubuntu apt) produce different output for the same `.clang-format`. **Fix**: Make formatting check non-blocking (`--dry-run || true`) or pin clang-format version in CI via LLVM apt repo. Do NOT use `--Werror` as a hard gate unless versions are pinned.
- **Disk quota exhaustion during batch operations**: `execute_code` with Python scripts fails with `[Errno 122] Disk quota exceeded`. **Fix**: Use `curl` directly instead of Python `requests` for API calls. Prefer shell loops over Python scripts for batch operations.
- **Multi-Executor Pattern**: The pipeline supports two executors — choose based on task characteristics:
  - **Hermes (direct patch)**: Simple fixes (1-2 files, <20 lines changed). Use `read_file` + `patch` directly. Fastest, no delegation overhead.
  - **Claude Code**: CI fixes, formatting, environment-specific debugging, complex refactors, general coding. Use `claude -p "task" --allowedTools "Edit,Read,Bash" --max-turns 15 --dangerously-skip-permissions`.
  - Selection: `files <= 2 && lines <= 20` → Hermes; else → Claude Code.
  - Claude Code print mode (`-p`) is preferred — no PTY needed, no dialog handling, exits when done.
  - For analysis-only tasks (review without editing): `--allowedTools "Read"`.
  - Set `--max-turns` to prevent runaway loops (10-15 for most tasks, 25-30 for complex multi-file work).
- **Sequential execution preference**: User prefers step-by-step execution over parallel batching. When user says "Начинай последовательно с начала" / "start sequentially from the beginning", execute plan items one at a time, confirming completion of each before moving to the next.
- **Dual planning — Hermes + Obsidian**: User wants plans saved to both `.hermes/plans/` (canonical, session-scoped) AND `~/Documents/Obsidian Vault/dev-notes/<project>/plans/` (long-term, git-tracked). Always ask if Obsidian copy is needed. If user says "нужна копия в plans", copy to Obsidian and commit dev-notes repo.
- **GitHub API via Python vs curl**: Bash `curl` with JSON payloads fails silently (exit code 2) when the body contains nested quotes, backslashes, or markdown backticks. **Fix**: Use Python `urllib.request` for all GitHub API POST operations. See `references/phase-2-closeout-ci-debug-2026-05-10.md` for the complete pattern.
- **No Release phase**: REMOVED — Release Phase added (Phase 5: tag + GitHub Release + Obsidian update).
- **No Do/Defer classification**: REMOVED — Do/Defer classification added to aggregator.
- **No report format standardization**: REMOVED — Classification and effort fields added to all reviewer templates.
- **Release recreation conflict**: When a release is created manually via API and a workflow later tries `gh release create` for the same tag, the workflow fails with HTTP 422. **Fix**: Either (a) let the workflow own all releases, (b) delete the manual release before re-dispatching the workflow, or (c) use `gh release upload --clobber` instead of `gh release create`. See `references/phase-2-closeout-ci-debug-2026-05-10.md` for the deletion + re-dispatch pattern.

## References

| File | Purpose |
|------|---------|
| `references/hermes-delegate-task-integration.md` | How to integrate review phase with Hermes `delegate_task` subagents |
| `references/session-2026-05-09-pitfalls.md` | Real-world pitfalls from fresnel-beacon session (subagent loops, rebase conflicts, artifact leaks, model misconfig) |
| `references/iteration-2-patterns.md` | Partial fix traps, fix-induced regressions, by-design persistence, devops gotchas from Iteration 2 reviews |
| `references/session-2026-05-10-fresnel-beacon-phase2.md` | fresnel-beacon Phase 2 auto-dev pipeline session notes (3+1 reviewer dispatch, CI formatting fix, sequential execution, dual planning) |
| `references/session-2026-05-10-fresnel-beacon-phase2.md` | fresnel-beacon Phase 2 auto-dev pipeline session notes (3+1 reviewer dispatch, self-corrected findings, CI without PAT) |
| `references/git-sync-checklist.md` | Pre/post-work git sync steps to avoid rebase conflicts |
| `references/phase-2-closeout-ci-debug-2026-05-10.md` | Phase 2 close-out: 5 sequential CI failures → fixes (action pinning, errno.h, Docker permissions, WPA2 password, release conflict) |
| `references/2026-05-11-analysis-and-improvements.md` | Gap analysis and improvement proposals (Multi-Executor, Release Phase, Do/Defer, Phase-based workflow, report standardization) |
| `references/2026-05-11-refactoring.md` | Refactoring session notes: Kimi CLI removal, Claude Code as primary executor, Do/Defer classification, Release Phase, Rust rewrite |

## Scripts

| Script | Purpose | Language |
|--------|---------|----------|
| `src/bin/run_pipeline.rs` | Entry point — orchestrates all phases | Rust |
| `src/bin/review_aggregator.rs` | Aggregates review findings into prioritized fix plan | Rust |
| `src/bin/ci_check.rs` | Checks GitHub Actions CI status via API | Rust |

Build: `cargo build --release`
Binaries: `target/release/run-pipeline`, `target/release/review-aggregator`, `target/release/ci-check`

**Note (2026-05-11)**: Scripts rewritten from bash/python to Rust. Old scripts in `scripts/` are deprecated. Rust binaries are not yet compiled — requires `cargo` (not installed). Once compiled, update `~/.hermes/config.yaml` quick_commands to point to `target/release/` binaries.

## Templates

| Template | Purpose |
|----------|---------|
| `templates/review-prompts/code.md` | Prompt for Code Reviewer subagent |
| `templates/review-prompts/security.md` | Prompt for Security Reviewer subagent |
| `templates/review-prompts/architecture.md` | Prompt for Architecture Reviewer subagent |
| `templates/review-prompts/devops.md` | Prompt for DevOps Reviewer subagent |
| `templates/plan-template.md` | Template for generated fix plan |

## Executor Model

> **Claude Code is the primary executor** for all coding tasks.
> Use print mode (`-p`) for non-interactive execution.

- **Hermes (оркестратор)**: planning, aggregation, decision-making, simple patches
- **Claude Code (исполнитель)**: all coding tasks — fixes, refactors, tests, CI debugging

Claude Code is invoked via print mode:
```bash
claude -p "task description" \
  --allowedTools "Read,Edit,Bash" \
  --max-turns 15 \
  --dangerously-skip-permissions
```

For analysis-only (review without editing):
```bash
claude -p "analyze code for issues" --allowedTools "Read" --max-turns 10
```

## Remember

```
4 reviewers in parallel
Aggregate → classify (Do Now / Defer) → plan
Simple fixes → Hermes (read_file + patch)
Complex tasks → Claude Code print mode (claude -p)
Verify CI after each iteration
Release: tag + GitHub Release + Obsidian update
Max 5 iterations
Operator can stop anytime
Save reports to dev-notes repo (NOT project .hermes/)
Project .gitignore must ignore .hermes/ graphify-out/ *.orig
Git sync before work: git pull --rebase origin main
```
