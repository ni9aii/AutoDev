#!/usr/bin/env bash
# Generate the project-structure block for README.md and SKILL.core.md.
#
# Why: hand-written structure trees drift (dogfood 20260824_161227: ~40% of
# findings were docs drift). This script renders the tree from the real
# filesystem into a marked block; CI fails when the committed block is stale
# (same contract as tools/gen.sh skill surfaces).
#
# Usage: bash tools/gen-structure.sh
# Check: git diff --exit-code -- README.md SKILL.core.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Render an ls-style tree for one directory. Depth-limited; excludes build
# artifacts. Files first (alphabetically), then subdirectories (recursed).
render_dir() {
  local dir="$1" prefix="$2" depth="$3" entry base
  # files in this directory
  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    base="${entry##*/}"
    echo "${prefix}├── ${base}"
  done < <(find "$dir" -maxdepth 1 -type f ! -name '*.d' | sort)
  # stop descending at max depth
  [ "$depth" -le 0 ] && return 0
  # subdirectories
  while IFS= read -r entry; do
    [ -n "$entry" ] || continue
    base="${entry##*/}"
    case "$base" in target|.git|advisory-db) continue ;; esac
    echo "${prefix}├── ${base}/"
    render_dir "${entry}" "${prefix}│   " "$((depth - 1))"
  done < <(find "$dir" -maxdepth 1 -type d ! -path "$dir" | sort)
}

render_tree() {
  echo '```text'
  echo '.'
  render_dir src "" 2
  render_dir tests "" 1
  render_dir tools "" 0
  render_dir .github "" 1
  render_dir skills "" 2
  render_dir harnesses "" 0
  render_dir references "" 0
  for f in Cargo.toml SKILL.core.md README.md AGENTS.md install.sh renovate.json; do
    [ -f "$f" ] && echo "├── $f"
  done
  echo '```'
}

update_block() {
  local file="$1"
  local tmp
  tmp="$(mktemp)"
  if ! grep -q 'STRUCTURE:BEGIN' "$file"; then
    echo "ERROR: $file has no <!-- STRUCTURE:BEGIN --> marker" >&2
    exit 1
  fi
  awk -v repl="$(render_tree)" '
    BEGIN { skip=0 }
    /<!-- STRUCTURE:BEGIN -->/ { print; print repl; skip=1; next }
    /<!-- STRUCTURE:END -->/   { skip=0 }
    !skip { print }
  ' "$file" > "$tmp"
  mv "$tmp" "$file"
}

update_block README.md
update_block SKILL.core.md
echo "structure blocks updated: README.md SKILL.core.md"
