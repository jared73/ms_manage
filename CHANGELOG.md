# Changelog

All notable changes to MS Manage are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/).

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