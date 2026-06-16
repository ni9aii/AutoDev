#!/usr/bin/env bash
# Install the autodev Claude Code skill
set -euo pipefail

SKILL_DIR="$HOME/.claude/skills/autodev"

mkdir -p "$SKILL_DIR"
cp "$(dirname "$0")/SKILL.md" "$SKILL_DIR/SKILL.md"

echo "Installed: $SKILL_DIR/SKILL.md"
echo ""
echo "Usage in Claude Code: /autodev [review|plan|execute|full] <project-name> [project-path]"
echo ""
echo "Make sure to set DEV_NOTES_ROOT in your shell profile:"
echo "  export DEV_NOTES_ROOT=~/obsidian-vault/dev-notes"
echo ""
echo "And ensure autodev binaries are on PATH:"
echo "  cd $HOME/code/AutoDev && cargo build --release"
echo "  export PATH=\"\$PATH:\$HOME/code/AutoDev/target/release\""
