#!/usr/bin/env bash
# Install the AutoDev skill into your agent harness — one command.
#
# Usage:
#   ./install.sh                 # auto-detect harness, install there
#   ./install.sh --harness H     # force harness (hermes | claude-code)
#   ./install.sh --list          # show supported harnesses
#   ./install.sh --check         # verify install, no changes
#
# No Python required. Re-renders the skill from SKILL.core.md + harnesses/*.overlay
# via tools/gen.sh, then copies the right skills/<h>/SKILL.md (+ references/) into
# the harness's skill directory.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GEN="$ROOT/tools/gen.sh"

HARNESSES=( hermes claude-code )

# harness -> default install directory
install_dir() {
  case "$1" in
    hermes)      echo "$HOME/.hermes/skills/autonomous-ai-agents/autodev" ;;
    claude-code) echo "$HOME/.claude/skills/autodev" ;;
    *) echo "" ;;
  esac
}

detect_harness() {
  if [ -d "$HOME/.hermes/skills" ] || command -v hermes >/dev/null 2>&1; then
    echo hermes; return
  fi
  if [ -d "$HOME/.claude/skills" ] || command -v claude >/dev/null 2>&1; then
    echo claude-code; return
  fi
  echo ""
}

usage() {
  cat <<'EOF'
Usage: ./install.sh [--harness H | --list | --check]

  (no args)   auto-detect harness and install
  --harness H install for harness H (hermes | claude-code)
  --list      list supported harnesses
  --check     verify install without changing anything
EOF
}

list_harnesses() {
  echo "Supported harnesses:"
  for h in "${HARNESSES[@]}"; do
    printf "  - %s  ->  %s\n" "$h" "$(install_dir "$h")"
  done
}

# ---- arg parse ----
MODE=install
HARNESS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --harness) HARNESS="${2:-}"; shift 2 ;;
    --list)    MODE=list; shift ;;
    --check)   MODE=check; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

[ "$MODE" = "list" ] && { list_harnesses; exit 0; }

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
  if [ -f "$DST" ]; then
    echo "OK: $DST exists"
    exit 0
  else
    echo "MISSING: $DST (run ./install.sh --harness $HARNESS to install)"
    exit 1
  fi
fi

# re-render from source so the installed copy is never stale
echo "Rendering skill surfaces..."
bash "$GEN"

echo "Installing into: $DST"
mkdir -p "$DST"
cp -f "$SRC" "$DST/SKILL.md"
if [ -d "$ROOT/skills/$HARNESS/references" ]; then
  mkdir -p "$DST/references"
  cp -f "$ROOT/skills/$HARNESS/references"/*.md "$DST/references/" 2>/dev/null || true
fi

echo ""
echo "Installed AutoDev skill for $HARNESS."
if [ "$HARNESS" = "hermes" ]; then
  echo "Load it with: /skill autodev   (or /autodev if mapped as a quick command)"
  echo "Note: if you use the git-sync plugin, it manages ~/.hermes — re-sync after install."
elif [ "$HARNESS" = "claude-code" ]; then
  echo "Load it in Claude Code with: /autodev [review|plan|execute|full] <project-name>"
fi
