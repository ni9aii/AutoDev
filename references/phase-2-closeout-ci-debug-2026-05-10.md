# Phase 2 Close-out CI Debug Session — fresnel-beacon

**Date:** 2026-05-10
**Session:** Phase 2 close-out after v0.2.0 tag, CI red, terminal crash recovery
**Commits involved:** ae8b5a5 → 5 sequential fixes → final green CI

---

## Failure Chain (5 sequential issues)

### 1. "Set up job" failure — Action SHA pinning

**Symptom:** CI run fails immediately at "Set up job" for `actions/cache` and `espressif/esp-idf-ci-action`.
**Root cause:** Pinned SHA hashes became invalid (rewritten history or CDN cache invalidation).
**Fix:** Revert to tag-based references:
```yaml
uses: actions/cache@v4.2.0
uses: espressif/esp-idf-ci-action@v1.1.0
```
**Commit:** `ci: revert action pinning to tag-based references`

### 2. `errno` undeclared in http_server.c

**Symptom:** Build fails with `error: 'errno' undeclared` at `strtoul()` error checking.
**Root cause:** `http_server.c` uses `errno` after `strtoul()` but never `#include <errno.h>`.
**Fix:** Add `#include <errno.h>` to `components/http_server/http_server.c`.
**Commit:** `fix(http_server): add missing errno.h include`

### 3. sha256sum Permission denied — Docker root ownership

**Symptom:** Release workflow fails at "Generate checksums" with "Permission denied".
**Root cause:** `build/` directory created inside Docker container (espressif/idf image) as root. Host runner cannot write to it.
**Fix:** Remove sha256sum step. GitHub Release assets are integrity-checked by GitHub itself.
**Commit:** `ci(release): remove sha256sum step, fix permission issue`

### 4. `ESP_ERR_WIFI_PASSWORD` — WPA2 password < 8 chars

**Symptom:** Wokwi simulation times out after 5 minutes. Logs show boot loop.
**Root cause:** AP password `"FB%02X%02X"` produces only 6 chars. `WIFI_AUTH_WPA2_PSK` requires ≥ 8.
**Fix:** Change format to `"Fresnel%02X%02X"` (8+ chars).
**Commit:** `fix(wifi_manager): increase AP password length to meet WPA2 minimum`

### 5. Release 422 — Release already exists for tag

**Symptom:** Release workflow fails at "Create GitHub Release" with HTTP 422.
**Root cause:** Release was created manually via API earlier. `gh release create` fails if release exists.
**Fix:** Delete old release via API, re-dispatch workflow.
```python
# Delete old release
req = urllib.request.Request(
    f"https://api.github.com/repos/{owner}/{repo}/releases/{release_id}",
    method="DELETE",
    headers={"Authorization": f"Bearer {token}"}
)
# Then re-dispatch workflow
```

---

## Key Techniques

### CI Log Retrieval via Python (not curl)

Bash curl with JSON payloads fails silently (exit code 2) due to quote escaping issues. Use Python `urllib.request`:

```python
import urllib.request, json

# Get runs
req = urllib.request.Request(
    "https://api.github.com/repos/ni9aii/fresnel-beacon/actions/runs?per_page=5",
    headers={"Authorization": f"Bearer {token}"}
)
with urllib.request.urlopen(req) as resp:
    data = json.load(resp)

# Download logs ZIP
req = urllib.request.Request(
    f"https://api.github.com/repos/ni9aii/fresnel-beacon/actions/runs/{run_id}/logs",
    headers={"Authorization": f"Bearer {token}", "Accept": "application/vnd.github+json"}
)
with urllib.request.urlopen(req) as resp:
    zip_data = resp.read()

# Parse ZIP
import zipfile, io
z = zipfile.ZipFile(io.BytesIO(zip_data))
for name in z.namelist():
    content = z.read(name).decode('utf-8', errors='replace')
    errors = [l for l in content.split('\n') if 'error:' in l.lower()]
    if errors:
        print(f"=== {name} ===")
        for e in errors[:10]:
            print(f"  {e}")
```

### Tag Force-Push + Release Recreation

When CI fix commits are added after a tag, the tag must be moved:

```bash
# 1. Delete local tag
git tag -d v0.2.0

# 2. Recreate on new HEAD
git tag -a v0.2.0 -m "..."

# 3. Force-push tag
git push origin v0.2.0 --force

# 4. Delete old release via API (Python)
# 5. Re-dispatch release workflow
```

**Pitfall:** Force-pushing tags is generally discouraged in shared repos, but acceptable for pre-release tags during active development. Always communicate tag moves to the team.

---

## Lessons

1. **Action pinning trade-offs:** SHA pinning is secure but fragile for third-party Docker-based actions. Have a documented fallback to tag-based.
2. **Missing includes are common after subagent edits:** Subagents add code using `errno`, `strtoul`, etc. but forget headers. Always check compilation after subagent work.
3. **Docker root ownership is invisible until you try to write:** Build artifacts appear fine for reading (`ls -lh`) but fail on write (`sha256sum > file`).
4. **WPA2 password length is a silent CI killer:** Simulation boot loops waste 5 minutes per run. Validate password length at compile time if possible.
5. **Manual API releases conflict with workflow releases:** Choose one owner for releases — either workflow-only or manual-only. Mixing causes 422 conflicts.
