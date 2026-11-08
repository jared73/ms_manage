# Magpie on Linux / NAS

This directory contains the Linux/NAS runner for the same Magpie loops. The container includes:

- `node:24-bookworm` (Node + git + curl)
- PowerShell 7.4.6 Linux x64 (so the existing `.ps1` tools work)
- `@opencode/cli@2.2.6`

The Windows-native `install-scheduled-tasks.ps1` is not used on NAS. Instead, the container runs `scheduler.mjs`, which invokes `run-loop.sh` for the three loops.

## Deploy layout

```text
/volume1/docker/ms-manage-magpie/
├── image/
│   ├── Dockerfile
│   └── powershell-7.4.6-linux-x64.tar.gz
├── docker-compose.yml
├── .env                    # MAGPIE_MODEL only
├── .git-credentials        # Gitea/GitHub credentials, chmod 600
├── scheduler.mjs
├── opencode-home/          # opencode auth + sessions, chmod 700
├── repo/                   # clone of ms_manage on main
└── logs/
```

## Build the image

```bash
cd /volume1/docker/ms-manage-magpie/image
# GitHub release download; use the proxy your NAS already has.
curl -L --fail --retry 2 -o powershell-7.4.6-linux-x64.tar.gz \
  "https://gh-proxy.com/https://github.com/PowerShell/PowerShell/releases/download/v7.4.6/powershell-7.4.6-linux-x64.tar.gz"
docker build --network host -t ms-manage-magpie-runner:latest .
```

If `gh-proxy.com` is unavailable, use the official URL with the NAS Clash proxy:

```bash
curl -x http://127.0.0.1:7890 -L --fail -o powershell-7.4.6-linux-x64.tar.gz \
  https://github.com/PowerShell/PowerShell/releases/download/v7.4.6/powershell-7.4.6-linux-x64.tar.gz
```

## Import opencode credentials

On Windows:

```powershell
opencode auth export > opencode-auth.json
```

Copy it to the NAS, then import it into the dedicated home directory:

```bash
docker run --rm --user 1000:10 \
  -e HOME=/workspace/home \
  -v /volume1/docker/ms-manage-magpie/opencode-home:/workspace/home \
  -v /volume1/docker/ms-manage-magpie/auth.json:/auth.json:ro \
  ms-manage-magpie-runner:latest \
  opencode auth import /auth.json
```

After import, delete `auth.json`.

## Start

```bash
cd /volume1/docker/ms-manage-magpie
docker compose up -d
```

Verify:

```bash
docker ps --filter name=ms-manage-magpie-runner
docker logs --tail 30 ms-manage-magpie-runner
docker exec ms-manage-magpie-runner opencode auth list
```

## Manual loop

```bash
docker exec ms-manage-magpie-runner \
  bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh nightly
```

`run-loop.sh` enforces:

- current branch must be `main`;
- clean working tree;
- a global `flock` lock (`automation/magpie/state/run.lock`);
- `MAGPIE_GATE_QUICK=1`, so `check-gate.ps1` skips `cargo-*` steps (Rust/Tauri Windows build is covered by GitHub Actions).

## Scheduling

`scheduler.mjs` starts three loops:

| loop | schedule |
|---|---|
| `issues` | every 30 minutes |
| `community` | every 60 minutes |
| `nightly` | daily at 03:10 Asia/Shanghai |

The schedule lives in the container, not in host cron, because this NAS user cannot write `/var/spool/cron`. If you prefer host cron, entries can still call:

```cron
*/30 * * * * docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh issues
0 * * * *    docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh community
10 3 * * *   docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh nightly
```

## Credentials

- `opencode-home/` holds the imported opencode provider auth.
- `.git-credentials` holds the Gitea/GitHub HTTPS credentials and is mounted read-only at `/workspace/.git-credentials`.
- `GITHUB_TOKEN` is intentionally **not** set as a container environment variable: opencode treats `GITHUB_TOKEN` as a GitHub Copilot auth signal and then hides the stored provider credentials. The PowerShell scripts get GitHub credentials from `git credential fill`, which uses the mounted credential store.
- `.env` only needs `MAGPIE_MODEL`.
