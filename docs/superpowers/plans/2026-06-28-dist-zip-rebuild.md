# Dist-Zip Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-run `scripts/build-zip.ps1` to produce a fresh
`dist-zip/MS-Manage-v0.1.0-win-x64.zip` from the existing
`ms_manage.exe` — no script or content changes.

**Architecture:** Single-task plan. The deliverable is the side effect
of running an existing, reviewed PowerShell script. The script handles
staging, copying, zipping, and cleanup internally; we just invoke it
and verify the artifact.

**Tech Stack:** PowerShell 5+ (`System.IO.Compression.FileSystem`),
Windows, Git Bash for the verification commands.

## Global Constraints

- Do NOT modify `scripts/build-zip.ps1`.
- Version stays at `0.1.0` (matches `package.json`).
- Zip contents stay at the original 4 items: `ms_manage.exe`,
  `README.md`, `e2e/`, `QUICKSTART.txt`.
- No checksums, signing, or new build steps.

---

## File Structure

This plan touches zero source files. The only expected new artifact is:

| Path                                                | Type   | Source                                  |
| --------------------------------------------------- | ------ | --------------------------------------- |
| `dist-zip/MS-Manage-v0.1.0-win-x64.zip`             | create | output of `scripts/build-zip.ps1`       |

Files inspected (read-only) during execution:
- `scripts/build-zip.ps1`
- `scripts/QUICKSTART.template.txt`
- `src-tauri/target/release/ms_manage.exe`
- `README.md`
- `e2e/`

---

### Task 1: Run the existing build script and verify the zip

**Files:**
- Read (for context, not modified): `scripts/build-zip.ps1`
- Read (for context, not modified): `scripts/QUICKSTART.template.txt`
- Create (artifact, not source): `dist-zip/MS-Manage-v0.1.0-win-x64.zip`

**Interfaces:**
- Consumes: `src-tauri/target/release/ms_manage.exe` (must exist)
- Produces: `dist-zip/MS-Manage-v0.1.0-win-x64.zip` containing
  `ms_manage.exe`, `README.md`, `e2e/`, `QUICKSTART.txt`.

- [ ] **Step 1: Confirm the source binary exists**

Run from the repo root `D:\project\73\ms_manage`:

```bash
ls -la "src-tauri/target/release/ms_manage.exe"
```

Expected: a single line showing the file with a non-zero size
(~7 MB). If missing, abort and ask the user to run a release build
(`npm run tauri:build`) before continuing.

- [ ] **Step 2: Run the build script**

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-zip.ps1
```

Expected: script exits with status `0`, prints two lines at the end:

```
Generated: D:\project\73\ms_manage\dist-zip\MS-Manage-v0.1.0-win-x64.zip
Size: <N> MB
```

(`<N>` is a positive number, typically a single-digit MB.)

If the script prints `Not found: …ms_manage.exe -- run …` and exits
with status `1`, go back to Step 1 — the binary is not where the
script expects.

- [ ] **Step 3: Verify the zip file exists and is non-empty**

```bash
ls -la "dist-zip/MS-Manage-v0.1.0-win-x64.zip"
```

Expected: a single line with a non-zero size. If the file is missing
or `0`, something went wrong — re-read the script output from Step 2
and re-run if needed.

- [ ] **Step 4: Verify the zip contents**

```bash
unzip -l "dist-zip/MS-Manage-v0.1.0-win-x64.zip"
```

Expected: a list with at least the following entries (extra entries
from `e2e/` are fine):

```
Archive:  dist-zip/MS-Manage-v0.1.0-win-x64.zip
  Length      Date    Time    Name
---------  ---------- -----   ----
        …  …          …       ms_manage.exe
        …  …          …       README.md
        …  …          …       QUICKSTART.txt
        …  …          …       e2e/
        …  …          …       (any files under e2e/)
```

The four required entries must all be present. Order in the listing
does not matter.

- [ ] **Step 5: Commit the artifact (optional but recommended)**

The zip is a build artifact; the existing `.gitignore` should be
checked first to see if `dist-zip/` is ignored.

```bash
git check-ignore -v "dist-zip/MS-Manage-v0.1.0-win-x64.zip" || true
git status --short dist-zip/
```

Expected: if `dist-zip/` is gitignored, the status will be empty —
skip the commit, nothing to do. If the file is tracked, commit:

```bash
git add "dist-zip/MS-Manage-v0.1.0-win-x64.zip"
git commit -m "build: rebuild MS-Manage v0.1.0 win-x64 green zip"
```

- [ ] **Step 6: Done**

The zip is ready for distribution. Hand the file
`dist-zip/MS-Manage-v0.1.0-win-x64.zip` to whoever needs it.
