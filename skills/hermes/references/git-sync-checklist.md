# Git Sync Checklist for Auto-Dev Pipeline

Run before starting any work on a project to avoid mid-session rebase conflicts.

## Pre-Work Steps

```bash
cd <project-dir>
git status                    # Check for uncommitted changes
git stash                     # If needed, stash them
git pull --rebase origin main # Sync with remote
```

## Post-Work Steps

```bash
git add -u
git commit -m "fix: description"
git push origin main          # Should fast-forward if sync was done
```

## If Push Rejected (Rebase with Conflicts)

```bash
git pull --rebase origin main
# For EACH conflicted file:
#   1. Edit file, resolve <<<<<<< markers, pick correct version
#   2. git add <file>
#   3. GIT_EDITOR=true git rebase --continue  (skip editor, keep original msg)
# Repeat until rebase completes, then push
```

**Real example from fresnel-beacon session:**
- 3 files conflicted: `beacon_math.h`, `led_driver.c`, `ci.yml`
- Resolution: `read_file` each conflict → `patch` with resolved version → `git add` → `GIT_EDITOR=true git rebase --continue`
- Took 3 iterations of `rebase --continue` before clean
- **Key:** Do NOT use `git rebase --continue` without `GIT_EDITOR=true` — it opens editor and blocks automation

## Common Conflict Sources

- CI commits on GitHub (workflow files, version bumps)
- Parallel agent work on same files
- `.gitignore` changes between local and remote

## Prevention

- Always pull before starting
- Commit early and often (one logical change per commit)
- Use `git config pull.rebase true` globally
- Keep `.gitignore` updated: `.hermes/`, `graphify-out/`, `*.orig`, build artifacts
