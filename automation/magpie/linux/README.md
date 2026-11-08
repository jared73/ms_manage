# Magpie on Linux / NAS

This directory contains the Linux-friendly runner used on a NAS or Linux server. It reuses the same prompt files, `check-gate.ps1`, `release.ps1`, and GitHub API scripts as the Windows version, because the container includes PowerShell 7.

## Container contents

- `node:24-bookworm`
- PowerShell 7.4.6 Linux x64
- `@opencode/cli@2.2.6`
- git / curl (already present in the Node image)

## Deploy layout on the NAS

```text
/volume1/docker/ms-manage-magpie/
├── image/
│   ├── Dockerfile
│   └── powershell-7.4.6-linux-x64.tar.gz
├── docker-compose.yml
├── .env                  # GITHUB_TOKEN / GITEA_TOKEN / MAGPIE_MODEL
├── auth.json             # opencode auth export, chmod 600
├── opencode-data/        # opencode auth db and sessions
├── repo/                 # clone of ms_manage on main
└── logs/                 # cron logs
```

## Build

```bash
cd /volume1/docker/ms-manage-magpie/image
curl -x http://127.0.0.1:7890 -L -o powershell-7.4.6-linux-x64.tar.gz \
  https://github.com/PowerShell/PowerShell/releases/download/v7.4.6/powershell-7.4.6-linux-x64.tar.gz
docker build -t ms-manage-magpie-runner:latest .
```

## Import opencode credentials

On Windows:

```powershell
opencode auth export > opencode-auth.json
```

Copy `opencode-auth.json` to the NAS as `auth.json` and run:

```bash
docker run --rm \
  -v /volume1/docker/ms-manage-magpie/opencode-data:/root/.local/share/opencode \
  -v /volume1/docker/ms-manage-magpie/auth.json:/auth.json:ro \
  ms-manage-magpie-runner:latest \
  opencode auth import /auth.json
```

## Start

```bash
cd /volume1/docker/ms-manage-magpie
docker compose up -d
```

## Run one loop manually

```bash
docker exec ms-manage-magpie-runner \
  bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh nightly
```

## Cron

```cron
*/30 * * * * docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh issues >> /volume1/docker/ms-manage-magpie/logs/cron.log 2>&1
0 * * * *    docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh community >> /volume1/docker/ms-manage-magpie/logs/cron.log 2>&1
10 3 * * *   docker exec ms-manage-magpie-runner bash /workspace/ms_manage/automation/magpie/linux/run-loop.sh nightly >> /volume1/docker/ms-manage-magpie/logs/cron.log 2>&1
```

> On this NAS the host user is in the `docker` group, so `docker exec` works without sudo.

## Notes

- The container uses `network_mode: host`, so it can reach Gitea at `127.0.0.1:9100` and Clash at `127.0.0.1:7890`.
- Rust/Tauri Windows builds are not run in this container. `ci.yml` and `release.yml` on GitHub Actions handle them.
- `MAGPIE_GATE_QUICK=1` makes `check-gate.ps1` skip `cargo-*` steps locally.
- Keep `auth.json` and `.env` at `chmod 600`; never commit them.
