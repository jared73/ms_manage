# Dist-Zip Rebuild — Design

Date: 2026-06-28
Status: Approved
Topic: Rebuild the green (portable) ZIP package using the existing `scripts/build-zip.ps1`.

## Goal

Re-run the existing PowerShell build script to produce a fresh
`dist-zip/MS-Manage-v0.1.0-win-x64.zip` from the currently-built
`ms_manage.exe`, without changing the script or the package contents.

## Non-Goals

- Not changing `scripts/build-zip.ps1`.
- Not bumping the version (stays `0.1.0`).
- Not adding/removing files inside the zip.
- Not signing the zip or computing checksums.
- Not building a new release (caller is expected to have already built
  `src-tauri/target/release/ms_manage.exe`).

## Inputs

| Path                                            | Required | Notes                          |
| ----------------------------------------------- | -------- | ------------------------------ |
| `src-tauri/target/release/ms_manage.exe`        | yes      | Source binary (~7 MB).         |
| `README.md` (project root)                      | optional | Copied when present.           |
| `e2e/` (project root)                           | optional | Recursively copied when present. |
| `scripts/QUICKSTART.template.txt`               | optional | Fallback inline text if missing. |

The script aborts with `Write-Error` if `ms_manage.exe` is missing.
`$ErrorActionPreference = 'Stop'` is set, so any failure exits the
script with a non-zero status.

## Output

| Path                                                       | Type   |
| ---------------------------------------------------------- | ------ |
| `dist-zip/MS-Manage-v0.1.0-win-x64.zip`                    | deliverable |
| `dist-zip/MS-Manage-v0.1.0-win-x64/` (staging temp dir)    | transient — lives in `$env:TEMP`, removed by script |

The zip is created via
`[System.IO.Compression.ZipFile]::CreateFromDirectory($stage, $zipPath, Optimal, $false)`
with compression level `Optimal` and `includeBaseDirectory = false`.

The final zip contains four entries (matching the original layout):

```
MS-Manage-v0.1.0-win-x64.zip
├── ms_manage.exe
├── README.md
├── e2e/
│   └── … (recursively copied)
└── QUICKSTART.txt
```

## Invocation

```bash
powershell -ExecutionPolicy Bypass -File scripts/build-zip.ps1
```

On success the script prints:

```
Generated: <absolute path to .zip>
Size: <MB>
```

## Verification

1. The script exits with status `0` and prints both `Generated:` and
   `Size:` lines.
2. `dist-zip/MS-Manage-v0.1.0-win-x64.zip` exists and is a valid ZIP
   (Windows Explorer opens it without error).
3. The zip contains exactly: `ms_manage.exe`, `README.md`, `e2e/` (if
   present in source), `QUICKSTART.txt`.

## Failure Modes

| Condition                              | Behavior                                    |
| -------------------------------------- | ------------------------------------------- |
| `ms_manage.exe` missing                | `Write-Error` + `exit 1`.                   |
| `dist-zip/` missing                    | Auto-created via `New-Item`.                |
| Old zip present                        | Removed by `Remove-Item -Force` before rebuild. |
| `README.md` / `e2e/` / template missing| Each copy is guarded by `Test-Path`; script still runs. |
| Any uncaught exception                 | `$ErrorActionPreference = 'Stop'` aborts script. |

## Risks

- PowerShell here-strings were a known source of mojibake; the script
  already handles this by reading `QUICKSTART.txt` from a UTF-8
  template file rather than embedding it inline. No change.
- `e2e/` is currently empty in this repo; `Copy-Item -Recurse` will
  create an empty `e2e/` directory entry inside the zip. This matches
  the prior behavior and is acceptable.

## Rollback

- Re-run the script to regenerate.
- `git checkout -- dist-zip/` removes any artifacts the script emitted
  (the only file it produces inside `dist-zip/` is the `.zip` itself;
  the staging area lives under `$env:TEMP` and is auto-cleaned).
