# Changelog

All notable changes to MS Manage are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/).

## [1.5.0] - 2026-07-31

> Major UI polish release. Ships a new **Global Memory** module, the **Connections** (client onboarding) page, the **Market** page, the **Doctor** self-check, and the **Standalone CLI** (`@ms-manage/cli`) for npx-based onboarding. Six rounds of UI/security/performance audit fixes have been folded in since 0.3.0.

### Added
- **Global Memory module** (`/memories` page + `cmd_memories` Tauri commands + `storage/memories.rs`)
  - Edit one master Markdown copy and sync it to each AI tool's real config path (`AGENTS.md` / `CLAUDE.md` etc.) via simple file copy.
  - **Sync state badges** (disabled / target missing / in sync / drift) computed live on every list call.
  - **Drift recovery**: when the target file changes externally, the card surfaces a `Pull` (back to master) / `Push` (overwrite target) choice — single-direction by default (master is authoritative), but visible at all times.
  - **First-enable safe reconcile**: when enabling and the target file already exists with non-empty content different from the master, the external content is pulled into the master first (as long as the master is still the initial template), so existing user configs are not overwritten.
  - **Preset auto-upgrade**: builtin presets (e.g. `opencode`) auto-import the existing target file's content and auto-enable on first run, so the app takes over the live config rather than stomping it with a template.
  - 5 Rust unit tests covering `sync_to_target` / `pull_from_target` / `enable_with_reconcile` / `sync_state` / `upgrade_preset_from_disk`.
- **Client onboarding page** (`/connections`) — paste-ready mcpServers JSON, copy-token flow, dual mode (proxy / individual), connection guide for Claude Desktop / Cursor / Cline / Continue.
- **Market page** (`/market`) — browse and one-click install community MCP servers and skills.
- **Doctor self-check** (`Settings → 环境体检`) — one-click environment health probe (binary presence, ports, paths, MCP reachability).
- **Standalone CLI** (`packages/cli/`) — `@ms-manage/cli` published as a small (~100-line) stdio↔HTTP forwarder for `npx`-based onboarding from clients that don't have MS Manage installed locally.
- **`Toggle` component** (`src/components/Toggle.tsx`) — unified 3 scattered `role="switch"` implementations (Settings autostart, Skills target, ServerDetailDrawer tool) into one component with consistent size/disabled/aria semantics. Guaranteed contrast across all 4 themes.
- **`i18n schema.ts`** — compile-time parity check between `zh.ts` and `en.ts`. Adding a key to one language without the other now fails `tsc --noEmit`.
- Settings theme swatch reads CSS variables instead of hardcoded hex.
- Design tokens: `onAccent` / `surfaceMuted` colors; `2xs` / `3xs` fontSize steps; `transitionDuration` / `transitionTimingFunction` defaults; `.surface-muted` / `.surface-accent-soft` / `.icon-xs/sm/md/lg` utilities.

### Performance
- **`servers_cache`** — `RwLock<Option<Vec<StoredServer>>>` cache for `load_servers()`. `tools/list` hot path no longer reads/parses `servers.json` on every request.
- **SSE `MAX_SESSIONS = 256` + 5-minute stale cleanup** — local processes can no longer exhaust the `SessionRegistry`. Each session has `mpsc::channel(64)` backpressure (was 256).
- **`settings_cache`** — `settings.json` warm-loaded at startup so the first request doesn't pay the cold-read cost.
- **token cache atomic** — `cached_token: Mutex<(String, u64)>` collapses the old `(Mutex<String>, AtomicU64>)` pair, eliminating the race window where `update_settings` and `regenerate_token` could leave generation ahead of value.
- **`proxy_starting` atomic guard** — prevents the duplicate proxy-launch race that two near-simultaneous clicks could trigger.
- **Dashboard `visibility`-based polling pause** — the dashboard stops polling while the window is hidden, saving wakeups.

### Safety
- **settings.json corruption recovery** — hand-written string scanner tries to extract `mcp_token` from a corrupt JSON before falling back to regenerating. Avoids the previous failure mode where all clients went 401 after a half-written settings.json was silently replaced.
- **LogRecord size cap** — `message` and `detail` each capped at 4KB via `floor_char_boundary`. Source field whitelisted to known names; unknown sources degrade to `"system"`.
- **`let _ = ...` → `log_event("error", source, ...)`** — failures in `cmd_install`, `cmd_skills`, `cmd_projects`, `cmd_settings` now surface in the GUI Logs page (previously only `eprintln!` to stderr).
- **timing-safe token compare** — hand-rolled `ct_eq(&[u8], &[u8])` for `bearer_ok` and `query_ok`. No new crate dependency.
- **Content-Length strict parse** — rejects malformed Content-Length rather than falling back to permissive behavior.
- **`drain_stderr` truncation + binary sniff** — prevents huge/binary stderr output from one misbehaving child from clogging logs.
- **`atomic_write` tmp file uses UUID** — eliminates the collision window when two writes raced on the same temp filename.
- **`fetch_tools` accepts both string and integer IDs** — forward-compatible with upstream ID changes.
- **Skill frontmatter via `serde_yaml`** — replaces the manual string parser that previously could miss fields on edge-case YAML.
- **`start_one` no longer overwrites `Error` variant** — child process error state preserved across restart attempts.
- **`httpConfig` narrowed types** — fewer ways for callers to construct an invalid HTTP server config.
- **`validate_skill_path` rejects `SystemRoot` env** — paths like `%WINDIR%\system32\…` are no longer valid skill targets.
- **TOCTOU and tar-path-traversal fixes** — tightened the install probe and the tar extraction paths.
- **`market cache` marked dead** — stale cache no longer served after upstream revalidation.

### UX
- **Modal focus trap** — distinguishes "focus outside dialog" from "focus at the real first/last focusable element"; Shift+Tab from the first input now jumps to the last.
- **`ServerDetailDrawer` uses `<Modal size="lg">`** — gains focus trap, Esc-to-close, `aria-modal` for free.
- **Modal Esc stack-aware** — pressing Esc with stacked modals closes the top one first.
- **Long-op progress** — install / sync / pull show non-blocking progress + cancel paths.
- **Async buttons disabled while in flight** — no more double-submit.
- **Form validation** — required fields validated client-side before round-tripping.
- **Token-regen double-confirm** — destructive token rotate requires two confirmations.
- **`ProjectsModal` flattened** — single-level dialog instead of nested.
- **Connections chevron + loading/error/reload+retry** — empty/loading/error states with retry button.
- **`useLogs` error/retry/skeleton + confirm-clear** — log page has full state coverage.
- **Skills search no-match empty state** — distinct UI from "no skills at all".
- **Server table horizontal scroll** — wide transports no longer break the layout.
- **Section header flex-wrap** — Logs toolbar (7 controls) no longer collapses at 960×640.
- **Microcopy unified** — `MCP 服务` / `代理` (zh) and `MCP server` / `Proxy` (en) consistently. JSON field names keep literal `server`.
- **All emoji indicators replaced with lucide icons** — cross-platform rendering consistency.
- **Light theme contrast pass** — meets WCAG AA on text/background pairs across all 4 themes.
- **Dashboard skeleton/loading state + dedup** — no flicker on first paint, no duplicate stats.
- **`StatusBadge role="status"`** — screen readers announce state changes.
- **Language persistence** — `settings.language` written and applied on reload; no more flash of default language.

### Accessibility
- `aria-label` added to all Servers row icon-only buttons (start / stop / test / edit / export / delete / view).
- `aria-label` added to Skills card action buttons (eye / pencil / trash).
- `aria-label` added to Logs search input, level and source selects.
- ServerDetailDrawer via `<Modal>`: gains `role="dialog"` + `aria-modal="true"`.
- Focus rings on all interactive elements, tab order verified per page.

### Internal
- **`buildMcpServers` + `copyAndDownload` extracted** — single helper functions used across code paths.
- **`needs_install` moved to backend `detect_pkg`** — single source of truth for install-required detection.
- **5 rounds of code-review cleanup** — language persistence, error handling, concurrency locks, dead code.

### Compatibility
- No breaking API changes (Tauri commands, IPC types, `settings.json` / `servers.json` / `projects.json` schema unchanged).
- Smooth upgrade from any 0.x version.
- Distribution channels: NSIS `.exe` installer (`AI Manage_<ver>_x64-setup.exe`) and portable `.zip`.

## [0.3.0] - 2026-07-26

### Added
- **`Toggle` component** (`src/components/Toggle.tsx`) — unified 3 scattered `role="switch"` implementations (Settings autostart, Skills target, ServerDetailDrawer tool) into one component with consistent size/disabled/aria semantics. Guaranteed contrast (`bg-fg-muted/30` + `border-border`) across all 4 themes.
- **`i18n schema.ts`** — compile-time parity check between `zh.ts` and `en.ts`. Adding a key to one language without the other now fails `tsc --noEmit`.
- **Settings theme swatch** reads CSS variables (`getComputedStyle`) instead of hardcoded hex — single source of truth, color tokens sync automatically.
- New i18n keys: `skills.card_toggle/edit/delete`, `servers.row_view/edit/delete/start/stop/copy`, `settings.lang_en`, `connections.proxy_running/stopped`, `market.load_failed/install_failed`, `common.copied`, `logs.level_debug/info/warn/error`.

### Performance
- **`servers_cache`** — Settings-style `RwLock<Option<Vec<StoredServer>>>` cache for `load_servers()`. `tools/list` hot path no longer reads and parses `servers.json` on every request.
- **SSE `MAX_SESSIONS = 256` + 5-minute stale cleanup** — local processes can no longer exhaust the `SessionRegistry`. Each session has `mpsc::channel(64)` backpressure (was 256).
- **token cache atomic** — `cached_token: Mutex<(String, u64)>` collapses the old `(Mutex<String>, AtomicU64>)` pair, eliminating the race window where `update_settings` and `regenerate_token` could leave generation ahead of value.

### Safety
- **settings.json corruption recovery** — hand-written string scanner (`src-tauri/src/storage/mod.rs`) tries to extract `mcp_token` from a corrupt JSON before falling back to regenerating. Avoids the previous failure mode where all clients went 401 after a half-written settings.json was silently replaced.
- **LogRecord size cap** — `message` and `detail` each capped at 4KB via `floor_char_boundary`. Source field whitelisted to known names; unknown sources degrade to `"system"`.
- **`let _ = ...` → `log_event("error", source, ...)`** — failures in `cmd_install`, `cmd_skills`, `cmd_projects`, `cmd_settings` now surface in the GUI Logs page (previously only `eprintln!` to stderr).
- **timing-safe token compare** — hand-rolled `ct_eq(&[u8], &[u8])` for `bearer_ok` and `query_ok` in `connect.rs`. No new crate dependency.

### UX
- **`Section` header** uses `flex-wrap gap-3` — Logs page toolbar (7 controls) no longer collapses to a vertical column at 960×640.
- **`Logs` search box** uses `flex-1 min-w-[140px] max-w-[260px]` — absorbs remaining row width; controls now fit one row at typical window sizes.
- **`Modal` focus trap** correctly distinguishes "focus outside dialog" from "focus at the real first/last focusable element" — Shift+Tab from the first input now jumps to the last instead of escaping the dialog.
- **`ServerDetailDrawer` uses `<Modal size="lg">`** wrapper — gains focus trap, Esc-to-close, `aria-modal` for free.
- **Microcopy unified** — user-facing strings now consistently use `MCP 服务` / `代理` (zh) and `MCP server` / `Proxy` (en). JSON field names (`mcpServers`, `server_id`) keep the literal `server`.
- **Settings theme card `✓`** → lucide `<Check size={12} aria-hidden />`.
- **Connections `✓`** → lucide `<Check />` + `aria-live="polite"`.
- **Connections `(未生成)`** → `t("settings.token_not_generated")`.
- **All emoji indicators** (✅ ❌ ⚠ 🔒 ✏ 🗑) replaced with lucide icons for cross-platform rendering consistency.

### Accessibility
- `aria-label` added to all Servers row icon-only buttons (start / stop / test / edit / export / delete / view).
- `aria-label` added to Skills card action buttons (eye / pencil / trash).
- `aria-label` added to Logs search input, level and source selects.
- ServerDetailDrawer via `<Modal>`: gains `role="dialog"` + `aria-modal="true"`.

### Design tokens
- **`tailwind.config.js`**: `onAccent` / `surfaceMuted` colors; `2xs` / `3xs` fontSize steps; `transitionDuration` (100/150/300ms) + `transitionTimingFunction` defaults.
- **`global.css`**: per-theme `--fg-on-accent` and `--surface-muted` CSS variables. `midnight` and `forest` themes now have theme-coherent muted surfaces (no more slate-gray bleed in blue/green themes).
- New utility classes: `.surface-muted`, `.surface-accent-soft`, `.icon-xs/sm/md/lg`.
- Icon size grep migration: Settings + Skills major buttons use `icon-sm` / `icon-md` className instead of inline `size={N}`.

### Changed
- `Settings` `auto_launch` toggle uses the new `Toggle` component.
- `Skills` `target.enabled` toggle uses `Toggle`.
- `ServerDetailDrawer` tool enable switches use `Toggle`.

### Compatibility
- No breaking API changes (Tauri commands, IPC types, settings.json / servers.json / projects.json schema unchanged).
- Smooth upgrade from any 0.2.x version.

## [0.2.0] - 2026-07-XX

Prior round-7 polish + round-6 security/performance audit fixes. See git history for detail.