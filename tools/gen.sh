#!/usr/bin/env bash
# Render AutoDev skill surfaces from one canonical core + per-harness overlays.
#
# Why: AutoDev targets several agent harnesses (Hermes, Claude Code, ...). Each
# needs its own SKILL.md (different frontmatter, invocation, tool names). Keeping
# N hand-written copies drifts. Instead we keep ONE workflow body (SKILL.core.md)
# and a small <harness>.overlay file, and render every surface here — in pure
# bash, no Python, so the Rust project stays Python-free.
#
# Overlay format: sections delimited by @@KEY@@ markers, e.g.
#   @@FRONTMATTER@@
#   <yaml/text>
#   @@INVOKE@@
#   <text>
#   ...
#
# Outputs:
#   harnesses/generic.overlay     -> ./SKILL.md                  (repo root, generic)
#   harnesses/hermes.overlay      -> ./skills/hermes/SKILL.md
#   harnesses/claude-code.overlay -> ./skills/claude-code/SKILL.md
#
# Each rendered skill gets a self-contained references/ copy alongside it.
#
# Run:  bash tools/gen.sh
# Check: git diff --exit-code   # fails if a committed surface drifted from source
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="$ROOT/SKILL.core.md"
HARNESS_DIR="$ROOT/harnesses"
REFERENCES="$ROOT/references"

# overlay file -> output SKILL.md (relative to repo root)
TARGETS=( "generic.overlay:SKILL.md"
           "hermes.overlay:skills/hermes/SKILL.md"
           "claude-code.overlay:skills/claude-code/SKILL.md" )

# Extract the block between @@KEY@@ and the next @@ marker (or EOF).
extract() {
  local file="$1" key="$2"
  awk -v k="@@${key}@@" '
    $0 == k { f=1; next }
    f && /^@@[A-Z_]+@@$/ { f=0 }
    f { print }
  ' "$file"
}

render() {
  local core="$1" overlay="$2"
  local out
  out="$(cat "$core")"
  for key in FRONTMATTER INVOKE REVIEWERS EXECUTE INSTALL_PATH_HINT; do
    local val
    val="$(extract "$overlay" "$key")"
    # strip a single leading/trailing blank line for cleanliness
    val="$(printf '%s' "$val" | sed -e '1{/^$/d}' -e '${/^$/d}')"
    out="${out//\{\{$key\}\}/$val}"
  done
  # drop any unfilled placeholder (defensive)
  out="$(printf '%s' "$out" | sed -E 's/\{\{[A-Z_]+\}\}//g')"
  printf '%s\n' "$out"
}

copy_references() {
  local dest_skill="$1"
  local dest_ref
  dest_ref="$(dirname "$dest_skill")/references"
  [ -d "$REFERENCES" ] || return 0
  # Root skill already lives next to references/ — nothing to copy.
  [ "$(realpath "$dest_ref")" = "$(realpath "$REFERENCES")" ] && return 0
  mkdir -p "$dest_ref"
  for f in "$REFERENCES"/*.md; do
    [ -e "$f" ] || continue
    cp -f "$f" "$dest_ref/"
  done
}

[ -f "$CORE" ] || { echo "ERROR: $CORE missing" >&2; exit 1; }

for entry in "${TARGETS[@]}"; do
  overlay_name="${entry%%:*}"
  rel_target="${entry#*:}"
  overlay="$HARNESS_DIR/$overlay_name"
  [ -f "$overlay" ] || { echo "WARN: $overlay missing, skip" >&2; continue; }
  rendered="$(render "$CORE" "$overlay")"
  target="$ROOT/$rel_target"
  mkdir -p "$(dirname "$target")"
  printf '%s' "$rendered" > "$target"
  copy_references "$target"
  echo "rendered: $rel_target"
done

echo "done"
