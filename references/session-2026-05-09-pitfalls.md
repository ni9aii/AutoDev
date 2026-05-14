# Session Pitfalls: Auto-Dev Pipeline on fresnel-beacon

## Pitfall 1: Kimi Code CLI Subagent Loop

**Symptom:** `delegate_task` with `toolsets=['terminal']` to run `kimi -p` causes 50+ identical `terminal` calls, subagent never completes.

**Root cause:** Kimi Code CLI is interactive by default. The subagent sees output, thinks it needs to respond, loops.

**Fix:** Use `kimi -p "task" --print --no-interactive` for non-interactive mode. Even better: use direct `file` + `patch` in Hermes for simple fixes. Reserve Kimi CLI for complex multi-file refactors only.

**Lesson:** Always prefer direct file editing over recursive CLI delegation when the fix is a single file change.

---

## Pitfall 2: Git Rebase Conflicts Mid-Session

**Symptom:** After 4 local commits, `git push origin main` rejected. `git pull --rebase` produced conflicts in 3 files (beacon_math.h, led_driver.c, ci.yml).

**Root cause:** Remote had moved ahead (CI commits or parallel work). Local commits were based on old HEAD.

**Fix sequence:**
1. `git pull --rebase origin main` (not `git pull` — creates merge commits)
2. Resolve each conflict file individually
3. `git add <file>` after each resolution
4. `GIT_EDITOR=true git rebase --continue` (skip editor for commit messages)
5. Repeat until clean, then `git push origin main`

**Prevention:** Always run `git pull --rebase origin main` BEFORE starting work.

---

## Pitfall 3: Review Artifacts Leaking into Project Repo

**Symptom:** `.hermes/plans/auto-dev/` and `reviews/` directories created inside project repo, risk of accidental commit.

**Fix:** Two-layer defense:
1. **Output path:** Save all reports to `~/dev-notes/<project>/reviews/` and `~/dev-notes/<project>/plans/` (separate repo)
2. **.gitignore:** Project repo must ignore `.hermes/`, `graphify-out/`, `*.orig`, and any review directories

**Verification:** `git status` should show no untracked review artifacts.

---

## Pitfall 4: Model Misconfiguration (Kimi-weak)

**Symptom:** Initially configured `[models.kimi-weak]` with `glm-4.5-air` in `~/.kimi/config.toml`. User corrected: Kimi Code subscription only provides `kimi-for-coding` (K2.6).

**Fix:** Removed `kimi-weak` section. Updated skill to state explicitly: **All agents use `kimi-for-coding` (K2.6) via Kimi Code subscription.** Split is by role (Hermes=planning, Kimi CLI=execution), not by model.

**Lesson:** Verify available models before configuring model-based splits. When only one model is available, split by role.

---

## Pitfall 5: OpenCode Rejection

**Symptom:** Suggested OpenCode as alternative CLI agent. User rejected: OpenCode has hardcoded model whitelist, does not support `kimi-for-coding`.

**Fix:** Use Kimi Code CLI exclusively. Document in skill: "User uses Kimi Code CLI (not OpenCode) for delegation."

---

## Pitfall 6: Review Reports Empty (Parser Mismatch)

**Symptom:** `review-aggregator.py` found 0 issues because subagent reports did not follow expected markdown structure (no `## Issues` section with bullet points).

**Fix:** Subagent prompts must explicitly specify output format. Example:
```
## Issues
- [CRITICAL] Description — file:line
- [IMPORTANT] Description — file:line
- [MINOR] Description — file:line
```

**Lesson:** Subagent output format must be machine-parseable. Include format example in prompt template.

---

## Summary: Updated Workflow

1. **Pre-flight:** `git pull --rebase origin main`
2. **Review:** 4 parallel `delegate_task` subagents → `~/dev-notes/<project>/reviews/`
3. **Aggregate:** Parse reports, generate plan → `~/dev-notes/<project>/plans/`
4. **Execute:** Direct `file` + `patch` for simple fixes; `kimi -p --print --no-interactive` for complex refactors only
5. **Verify:** Run tests, check CI
6. **Commit:** One logical change per commit, push immediately after each fix group
7. **Loop:** If issues remain, back to step 2 (max 5 iterations)
