# Adding a new harness

Supporting a new agent harness is intentionally small: one overlay file plus
installer wiring. The canonical skill content lives in `SKILL.core.md`;
harness-specific differences live in `harnesses/<name>.overlay`.

## Steps

1. **Create the overlay** `harnesses/<name>.overlay`. It declares only what
   differs for this harness (tool names, invocation syntax, paths). Look at
   `harnesses/hermes.overlay` and `harnesses/claude-code.overlay` as
   templates — most harnesses need only a few lines.
2. **Register it in `tools/gen.sh`** so rendering produces
   `skills/<name>/SKILL.md` and the references symlink.
3. **Register it in `install.sh`**: add the harness to `HARNESSES=(...)` and
   its default install directory in `install_dir()`.
4. **Test locally**:
   ```bash
   bash tools/gen.sh
   AUTODEV_INSTALL_ROOT=/tmp/test ./install.sh --harness <name>
   AUTODEV_INSTALL_ROOT=/tmp/test ./install.sh --harness <name> --check
   ```
5. **CI covers it automatically**: the gen-check job verifies every rendered
   surface against the source, and the installer checks iterate all
   registered harnesses.
6. **Docs**: add a row to the install tables in `README.md` and
   `docs/installation.md`.

## Rules

- Never edit rendered `SKILL.md` files — they are generated; CI fails on
  drift (`bash tools/gen.sh && git diff --exit-code`).
- No Python anywhere — scripts are bash, code is Rust.
- Every change ships with tests/CI coverage and docs updates in the same PR.
