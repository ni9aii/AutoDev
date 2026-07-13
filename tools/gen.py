#!/usr/bin/env python3
"""Render AutoDev skill surfaces from a single canonical core + per-harness overlays.

Why: AutoDev targets several agent harnesses (Hermes, Claude Code, ...). Each
needs its own SKILL.md (different frontmatter, invocation, tool names). Keeping
N hand-written copies drifts. Instead we keep ONE workflow body (SKILL.core.md)
and a small <harness>.yaml overlay, and render every surface here.

Outputs:
  SKILL.core.md + harnesses/generic.yaml     -> ./SKILL.md                (repo root, generic)
  SKILL.core.md + harnesses/hermes.yaml      -> ./skills/hermes/SKILL.md
  SKILL.core.md + harnesses/claude-code.yaml -> ./skills/claude-code/SKILL.md

Each rendered skill gets a self-contained `references/` copy alongside it, so an
agent loading the skill in isolation still has the deep-dive guides.

Run:  python3 tools/gen.py
Check: git diff --exit-code   # fails if a committed surface drifted from source
"""
from __future__ import annotations

import os
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORE = ROOT / "SKILL.core.md"
HARNESS_DIR = ROOT / "harnesses"
REFERENCES = ROOT / "references"

# Map overlay file -> output SKILL.md path (relative to repo root)
TARGETS = {
    "generic.yaml": Path("SKILL.md"),
    "hermes.yaml": Path("skills/hermes/SKILL.md"),
    "claude-code.yaml": Path("skills/claude-code/SKILL.md"),
}

PLACEHOLDERS = ("FRONTMATTER", "INVOKE", "REVIEWERS", "EXECUTE", "INSTALL_PATH_HINT")


def parse_overlay(path: Path) -> dict[str, str]:
    """Minimal YAML parser for our flat `key: |\\n  block` overlays.

    Supports: top-level `key: value` and `key: |` block scalars (indentation
    significant). No nesting, no flow style — sufficient for overlays.
    """
    text = path.read_text(encoding="utf-8")
    data: dict[str, str] = {}
    key = None
    buf: list[str] = []
    base_indent = None

    def flush() -> None:
        if key is not None:
            # strip trailing blank line, keep internal formatting
            while buf and buf[-1].strip() == "":
                buf.pop()
            data[key] = "\n".join(buf)

    for raw in text.splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            if key is not None and base_indent is not None:
                buf.append(raw[base_indent:] if len(raw) >= base_indent else "")
            continue
        m = re.match(r"^(\S.*?):\s*(\||$)?", raw)
        if m and not raw.startswith(" "):
            flush()
            key = m.group(1)
            buf = []
            base_indent = None
            if m.group(2) == "|":
                base_indent = None  # set on first block line
                continue
            # inline value
            data[key] = (m.group(3) if False else "").strip()
            # (m.group(3) unused; inline values not used in overlays)
            key = None
        else:
            if base_indent is None:
                base_indent = len(raw) - len(raw.lstrip())
            buf.append(raw[base_indent:])
    flush()
    return data


def render(core: str, overlay: dict[str, str]) -> str:
    out = core
    for ph in PLACEHOLDERS:
        token = "{{%s}}" % ph
        if token in out:
            out = out.replace(token, overlay.get(ph.lower(), "").strip())
    # drop any unfilled placeholder (defensive)
    out = re.sub(r"\{\{[A-Z_]+\}\}", "", out)
    return out.strip() + "\n"


def copy_references(dest_skill: Path) -> None:
    if not REFERENCES.is_dir():
        return
    dest_ref = dest_skill.parent / "references"
    # Root skill already lives next to references/ — nothing to copy.
    if dest_ref.resolve() == REFERENCES.resolve():
        return
    dest_ref.mkdir(parents=True, exist_ok=True)
    for f in REFERENCES.glob("*.md"):
        if (dest_ref / f.name).resolve() == f.resolve():
            continue
        shutil.copyfile(f, dest_ref / f.name)


def main() -> int:
    if not CORE.exists():
        print("ERROR: %s missing" % CORE, file=sys.stderr)
        return 1
    core = CORE.read_text(encoding="utf-8")

    for name, rel_target in TARGETS.items():
        overlay_path = HARNESS_DIR / name
        if not overlay_path.exists():
            print("WARN: overlay %s missing, skipping" % overlay_path)
            continue
        overlay = parse_overlay(overlay_path)
        rendered = render(core, overlay)
        target = ROOT / rel_target
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(rendered, encoding="utf-8")
        copy_references(target)
        print("rendered: %s" % rel_target)

    print("done")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
