#!/usr/bin/env bash
# Install the AutoDev skill into your agent harness — one command.
#
# Usage:
#   ./install.sh                 # auto-detect harness, install there
#   ./install.sh --harness H     # force harness (hermes | claude-code)
#   ./install.sh --list          # show supported harnesses
#   ./install.sh --check         # verify install, no changes
#   ./install.sh --update        # re-install latest from a checkout
#   ./install.sh --remote [TAG]  # one-line install: fetch release tarball,
#                                # then install (no checkout needed):
#   curl -fsSL https://raw.githubusercontent.com/ni9aii/AutoDev/vX.Y.Z/install.sh | bash -s -- --remote [TAG]
#   ./install.sh --uninstall     # remove the installed skill dir
#
# No Python required. Prerequisites: bash, git, rsync, and a symlink-capable
# checkout (git core.symlinks=true; on Windows use WSL or enable symlinks).
# Re-renders the skill from SKILL.core.md + harnesses/*.overlay
# via tools/gen.sh, then copies the right skills/<h>/SKILL.md (+ references/)
# into the harness's skill directory. In the repo, skills/<h>/references is a
# SYMLINK to the shared root references/ (see tools/gen.sh); install.sh
# dereferences it (-aL) so each installed skill dir is self-contained with
# real files.
#
# Remote mode (--remote): downloads the release tarball
# https://github.com/ni9aii/AutoDev/archive/refs/tags/<TAG>.tar.gz
# (default TAG: latest GitHub release), unpacks it to a temp dir and runs the
# bundled copy of this script from there — full reuse of the local logic.
set -euo pipefail

REPO="ni9aii/AutoDev"
SELF_VERSION_MARKER="# AUTODEV_SKILL_VERSION"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GEN="$ROOT/tools/gen.sh"

# Base directory for installs. Override with AUTODEV_INSTALL_ROOT (used by CI
# and for custom install locations); defaults to $HOME.
INSTALL_ROOT="${AUTODEV_INSTALL_ROOT:-$HOME}"

HARNESSES=( hermes claude-code )

# harness -> default install directory (under INSTALL_ROOT)
install_dir() {
  case "$1" in
    hermes)      echo "$INSTALL_ROOT/.hermes/skills/autonomous-ai-agents/autodev" ;;
    claude-code) echo "$INSTALL_ROOT/.claude/skills/autodev" ;;
    *) echo "" ;;
  esac
}

detect_harness() {
  if [ -d "$INSTALL_ROOT/.hermes/skills" ] || command -v hermes >/dev/null 2>&1; then
    echo hermes; return
  fi
  if [ -d "$INSTALL_ROOT/.claude/skills" ] || command -v claude >/dev/null 2>&1; then
    echo claude-code; return
  fi
  echo ""
}

usage() {
  cat <<'EOF'
Usage: ./install.sh [--harness H | --list | --check | --update | --uninstall | --remote [TAG]]

  (no args)        auto-detect harness and install
  --harness H      install for harness H (hermes | claude-code)
  --list           list supported harnesses
  --check          verify install without changing anything
  --update         re-install (overwrite existing copy with current source)
  --uninstall      remove the installed skill directory
  --remote [TAG]   one-line install without a checkout: download release
                   tarball vTAG (default: latest release), then install.
                   Requires curl and tar.

  Env: AUTODEV_INSTALL_ROOT overrides the base dir (default $HOME).
       e.g. AUTODEV_INSTALL_ROOT=/tmp/test ./install.sh --harness claude-code

  One-line install:
    curl -fsSL https://raw.githubusercontent.com/ni9aii/AutoDev/vX.Y.Z/install.sh \
      | bash -s -- --remote [TAG]
EOF
}

list_harnesses() {
  echo "Supported harnesses:"
  for h in "${HARNESSES[@]}"; do
    printf "  - %s  ->  %s\n" "$h" "$(install_dir "$h")"
  done
}

# ---- remote mode: fetch release tarball and re-exec local logic ----
latest_tag() {
  # Resolve the latest published release tag. curl + grep only (no jq dep).
  local url="https://api.github.com/repos/${REPO}/releases/latest"
  curl -fsSL "$url" | grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4
}

run_remote() {
  local tag="${1:-}"
  command -v curl >/dev/null 2>&1 || { echo "ERROR: --remote requires curl." >&2; exit 1; }
  command -v tar  >/dev/null 2>&1 || { echo "ERROR: --remote requires tar." >&2; exit 1; }

  if [ -z "$tag" ]; then
    echo "Resolving latest release..."
    tag="$(latest_tag)"
    if [ -z "$tag" ]; then
      echo "ERROR: could not resolve the latest release tag." >&2
      echo "Pass an explicit tag: --remote vX.Y.Z" >&2
      exit 1
    fi
  fi
  case "$tag" in
    v[0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "ERROR: invalid tag '$tag' (expected vX.Y.Z)." >&2; exit 1 ;;
  esac

  local url="https://github.com/${REPO}/archive/refs/tags/${tag}.tar.gz"
  local tmp; tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  echo "Downloading ${url}"
  curl -fsSL "$url" -o "$tmp/src.tar.gz"

  # Integrity check: if a <tarball>.sha256 asset exists next to the tarball,
  # verify it. GitHub archives do not ship checksums by default, so absence is
  # not fatal — but it is called out loudly so users know the download was NOT
  # verified.
  if curl -fsSL "${url}.sha256" -o "$tmp/src.tar.gz.sha256" 2>/dev/null; then
    command -v sha256sum >/dev/null 2>&1 || {
      echo "ERROR: sha256sum is required to verify --remote downloads." >&2
      exit 1
    }
    echo "Verifying sha256 checksum..."
    # Accept both '<digest>  <file>' and bare '<digest>' checksum files by
    # comparing first fields directly.
    expected="$(awk 'NR==1{print $1}' "$tmp/src.tar.gz.sha256")"
    actual="$(sha256sum "$tmp/src.tar.gz" | awk '{print $1}')"
    if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
      echo "ERROR: sha256 verification FAILED for ${tag} tarball." >&2
      echo "The downloaded file does not match the published checksum — do not use it." >&2
      exit 1
    fi
    echo "sha256 OK."
  else
    echo "WARNING: no .sha256 checksum published for ${tag}; download NOT verified." >&2
  fi

  mkdir -p "$tmp/src"
  tar -xzf "$tmp/src.tar.gz" -C "$tmp/src" --strip-components=1

  echo "Installing AutoDev ${tag} (remote mode)..."
  # ${arr[@]+...}: safe expansion under `set -u` when the array is empty.
  exec bash "$tmp/src/install.sh" ${MODE_FLAGS[@]+"${MODE_FLAGS[@]}"}
}

# ---- arg parse ----
MODE=install
HARNESS=""
REMOTE_TAG=""
MODE_FLAGS=()
while [ $# -gt 0 ]; do
  case "$1" in
    --harness) HARNESS="${2:-}"; shift 2 ;;
    --list)    MODE=list; shift ;;
    --check)   MODE=check; shift ;;
    --update)  MODE=install; MODE_FLAGS=(--update); shift ;;
    --uninstall) MODE=uninstall; shift ;;
    --remote)
      shift
      REMOTE_TAG="${1:-}"
      # `set -e` kills a bare `[ ... ] && shift` when the test fails (the
      # documented no-TAG form `install.sh --remote`), so use an explicit if.
      if [ $# -gt 0 ]; then shift; fi
      MODE=remote ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Forward the resolved --harness decision to the inner (tarball) script so a
# user's `--remote vX.Y.Z --harness claude-code` survives the exec even when
# the tarball ships an older installer. Only forward what the user actually
# asked for; otherwise the inner script auto-detects.
if [ "$MODE" = "remote" ] && [ -n "$HARNESS" ]; then
  case "${MODE_FLAGS[*]}" in
    *"--harness"*) ;;               # already forwarded verbatim below
    *) MODE_FLAGS=("${MODE_FLAGS[@]+"${MODE_FLAGS[@]}"}" --harness "$HARNESS") ;;
  esac
fi

if [ "$MODE" = "list" ]; then list_harnesses; exit 0; fi

if [ "$MODE" = "remote" ]; then
  run_remote "$REMOTE_TAG"   # never returns (exec)
fi

if [ -z "$HARNESS" ]; then
  HARNESS="$(detect_harness)"
  if [ -z "$HARNESS" ]; then
    echo "Could not auto-detect a harness." >&2
    echo "Supported: ${HARNESSES[*]}" >&2
    echo "Re-run with --harness H (e.g. ./install.sh --harness claude-code)" >&2
    exit 1
  fi
  echo "Auto-detected harness: $HARNESS"
fi

case "$HARNESS" in
  hermes|claude-code) ;;
  *) echo "Unsupported harness: $HARNESS" >&2; list_harnesses >&2; exit 1 ;;
esac

SRC="$ROOT/skills/$HARNESS/SKILL.md"
DST="$(install_dir "$HARNESS")"

if [ ! -f "$SRC" ]; then
  echo "ERROR: rendered skill missing: $SRC" >&2
  echo "Run: bash tools/gen.sh" >&2
  exit 1
fi

if [ "$MODE" = "check" ]; then
  if [ -f "$DST/SKILL.md" ]; then
    echo "OK: $DST/SKILL.md exists"
    exit 0
  else
    echo "MISSING: $DST/SKILL.md (run ./install.sh --harness $HARNESS to install)"
    exit 1
  fi
fi

if [ "$MODE" = "uninstall" ]; then
  if [ -d "$DST" ]; then
    rm -rf "$DST"
    echo "Removed: $DST"
  else
    echo "Nothing to uninstall: $DST does not exist."
  fi
  exit 0
fi

# re-render from source so the installed copy is never stale
echo "Rendering skill surfaces..."
bash "$GEN"

REFS_SRC="$ROOT/skills/$HARNESS/references"
# Fail loudly BEFORE any partial install if the references source did not
# resolve to a directory of .md files. On checkouts without symlink support
# (e.g. git core.symlinks=false on Windows), skills/<h>/references materializes
# as a plain TEXT file containing "../../references" — silently skipping it
# would produce an incomplete skill.
if [ ! -d "$REFS_SRC" ] || ! ls "$REFS_SRC"/*.md >/dev/null 2>&1; then
  echo "ERROR: $REFS_SRC did not resolve to a directory of .md files." >&2
  echo "This usually means your checkout materialized reference symlinks as plain" >&2
  echo "text files (git core.symlinks=false on Windows). Enable symlink support" >&2
  echo "(git config --global core.symlinks true + a filesystem that supports links," >&2
  echo "or WSL) and re-clone/re-checkout, then re-run install." >&2
  exit 1
fi

command -v rsync >/dev/null 2>&1 || {
  echo "ERROR: rsync is required by install.sh but was not found in PATH." >&2
  echo "Install rsync (on Windows/Git-Bash it is not present by default; use WSL" >&2
  echo "or install rsync for MSYS2/Git-Bash), then re-run install." >&2
  exit 1
}

echo "Installing into: $DST"
mkdir -p "$DST"
cp -f "$SRC" "$DST/SKILL.md"
if [ -d "$REFS_SRC" ]; then
  # Mirror, don't accumulate (Fix 7): the installed references/ must exactly
  # equal the source set — rsync --delete removes orphan copies left by
  # renamed/deleted sources.
  #
  # Symlink layout: tools/gen.sh makes skills/<h>/references a SYMLINK to
  # ../../references. Installed skill dirs must be self-contained, so we
  # dereference with -L: rsync -aL copies the real files the link points at,
  # not the link itself. (Equivalent to rsync from "$ROOT/references/" since
  # both per-harness links point there; using the per-harness path keeps the
  # install driven by what that harness's surface declares.)
  mkdir -p "$DST/references"
  rsync --delete -aL "$ROOT/skills/$HARNESS/references/" "$DST/references/"
  # Guard: installed references must be REAL files, never a symlink —
  # otherwise the install is not self-contained.
  if [ -L "$DST/references" ]; then
    echo "ERROR: $DST/references is a symlink; expected dereferenced real files." >&2
    exit 1
  fi
  if ! find "$DST/references" -maxdepth 1 -name '*.md' -type f | grep -q .; then
    echo "ERROR: $DST/references contains no real .md files." >&2
    exit 1
  fi
fi

# Stamp installed version (C5 update path): taken from Cargo.toml by gen.sh —
# see below. Falls back to unknown if the marker line was not produced.
INSTALLED_VERSION="unknown"
VERSION_LINE="$(grep -m1 '^version = "' "$ROOT/Cargo.toml" 2>/dev/null || true)"
if [ -n "$VERSION_LINE" ]; then
  INSTALLED_VERSION="$(printf '%s' "$VERSION_LINE" | cut -d'"' -f2)"
fi
printf '%s %s\n' "$SELF_VERSION_MARKER" "$INSTALLED_VERSION" > "$DST/.autodev-version"

echo ""
echo "Installed AutoDev skill for $HARNESS (version $INSTALLED_VERSION)."
echo "Re-run ./install.sh (or --update) after pulling new releases to refresh."
if [ "$HARNESS" = "hermes" ]; then
  echo "Load it with: /skill autodev   (or /autodev if mapped as a quick command)"
  echo "Note: if you use the git-sync plugin, it manages ~/.hermes — re-sync after install."
elif [ "$HARNESS" = "claude-code" ]; then
  echo "Load it in Claude Code with: /autodev [review|plan|execute|full] <project-name>"
fi
