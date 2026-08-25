# Installation

The fastest path is the one-command installer (no checkout needed):

```bash
curl -fsSL https://raw.githubusercontent.com/ni9aii/AutoDev/main/install.sh | bash -s -- --remote
```

Pin a specific release instead of the latest:

```bash
curl -fsSL https://raw.githubusercontent.com/ni9aii/AutoDev/main/install.sh | bash -s -- --remote v0.9.0
```

`--remote` downloads the release tarball from GitHub, unpacks it to a temp
directory and runs the bundled installer — full reuse of the local logic.

## From a checkout

```bash
git clone https://github.com/ni9aii/AutoDev && cd AutoDev
./install.sh                 # auto-detects your harness, installs there
./install.sh --harness hermes      # or force a specific harness
./install.sh --harness claude-code
./install.sh --list          # show supported harnesses + install paths
./install.sh --check         # verify an install without changing anything
./install.sh --update        # re-install from current source (explicit alias)
./install.sh --uninstall     # remove the installed skill directory
```

## Flags and environment

| Flag | Effect |
|------|--------|
| `--harness H` | Install for `hermes` or `claude-code` |
| `--list` | Show supported harnesses and install paths |
| `--check` | Verify install; exit 0 present / 1 missing |
| `--update` | Re-install, overwriting the existing copy |
| `--uninstall` | Remove the installed skill directory |
| `--remote [TAG]` | One-line install from release tarball (default: latest) |

| Env var | Effect |
|---------|--------|
| `AUTODEV_INSTALL_ROOT` | Base dir for installs (default `$HOME`) |

## What gets installed

The installer re-renders the skill surfaces from `SKILL.core.md` +
overlays (`tools/gen.sh`) and copies the right `skills/<harness>/SKILL.md`
with a dereferenced, self-contained copy of `references/` into your
harness's skill directory. A version stamp `.autodev-version` is written so
you can always tell which release you have:

| Harness     | Install path                                    |
|-------------|--------------------------------------------------|
| Hermes      | `~/.hermes/skills/autonomous-ai-agents/autodev`  |
| Claude Code | `~/.claude/skills/autodev`                       |

## Updating and uninstalling

Re-run the same one-liner (or `install.sh --update` from a checkout) after a
new release — the install mirrors the source set (`rsync --delete`), so
renamed/removed reference files do not accumulate. `--uninstall` removes the
directory entirely.

Note for Hermes users with the git-sync plugin: it manages `~/.hermes` —
re-sync after installing or updating.

## Manual install (alternative)

If you prefer not to run scripts, copy the rendered skill by hand:

```bash
# Hermes
cp skills/hermes/SKILL.md ~/.hermes/skills/autonomous-ai-agents/autodev/SKILL.md
rsync -aL skills/hermes/references/ ~/.hermes/skills/autonomous-ai-agents/autodev/references/

# Claude Code
cp skills/claude-code/SKILL.md ~/.claude/skills/autodev/SKILL.md
rsync -aL skills/claude-code/references/ ~/.claude/skills/autodev/references/
```

## Requirements

bash, git, rsync; `curl` + `tar` for `--remote`. A symlink-capable checkout
(git `core.symlinks=true`; on Windows use WSL) when installing from source.
No Python anywhere in this project.
