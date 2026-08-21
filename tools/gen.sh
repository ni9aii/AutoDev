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

  # Keys actually used by the core file (single source of truth — the loop
  # below can no longer drift from what SKILL.core.md contains).
  local core_keys
  core_keys="$(printf '%s' "$out" | grep -o '{{[A-Z_]*}}' | tr -d '{}' | sort -u)"

  # Keys provided by this overlay.
  local overlay_keys
  overlay_keys="$(grep -o '^@@[A-Z_]*@@$' "$overlay" | tr -d '@' | sort -u)"

  # Strict contract: every core key must exist in the overlay and vice versa.
  local missing extra k
  missing="$(comm -23 <(printf '%s\n' "$core_keys") <(printf '%s\n' "$overlay_keys"))"
  extra="$(comm -13 <(printf '%s\n' "$core_keys") <(printf '%s\n' "$overlay_keys"))"
  if [ -n "$missing" ]; then
    echo "ERROR: $overlay is missing overlay keys used by core: $(echo "$missing" | tr '\n' ' ')" >&2
    exit 1
  fi
  if [ -n "$extra" ]; then
    echo "ERROR: $overlay defines keys the core never uses: $(echo "$extra" | tr '\n' ' ')" >&2
    exit 1
  fi

  while IFS= read -r k; do
    [ -n "$k" ] || continue
    local val
    val="$(extract "$overlay" "$k")"
    # strip a single leading/trailing blank line for cleanliness
    val="$(printf '%s' "$val" | sed -e '1{/^$/d}' -e '${/^$/d}')"
    out="${out//\{\{$k\}\}/$val}"
  done <<< "$core_keys"

  # Strict contract: no placeholder may survive substitution. Fail loudly
  # instead of stripping silently — a typo'd key or missing overlay section
  # used to render as an empty section that passed every gate.
  if printf '%s' "$out" | grep -q '{{[A-Z_]*}}'; then
    echo "ERROR: unfilled placeholders remain after substitution:" >&2
    printf '%s' "$out" | grep -o '{{[A-Z_]*}}' | sort -u >&2
    exit 1
  fi
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
  [ -f "$overlay" ] || { echo "ERROR: $overlay missing (every target surface requires its overlay — silent skips used to leave stale SKILL.md files that still passed the drift check)" >&2; exit 1; }
  rendered="$(render "$CORE" "$overlay")"
  target="$ROOT/$rel_target"
  mkdir -p "$(dirname "$target")"
  printf '%s' "$rendered" > "$target"
  copy_references "$target"
  echo "rendered: $rel_target"
done

echo "done"
