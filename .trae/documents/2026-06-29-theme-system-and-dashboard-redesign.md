# 主题系统与主界面重设计 Plan

## 摘要

为 MS Manage 增加 4 套可切换主题（暗紫 / 午夜蓝 / 护眼绿 / 明亮浅色），改造全部 7 个页面的视觉风格。主题切换通过 **CSS 变量 + Tailwind 配置** 实现，所有页面零侵入式适配；主题选择 UI 放在 Settings 页，状态走 zustand store + Rust 后端持久化。

## 现状分析

### 已有基础
- `AppSettings.theme` 字段已存在（[src-tauri/src/cmd_settings.rs:10](file:///d:/workspace/73/ms_manage/src-tauri/src/cmd_settings.rs#L10)、[src-tauri/src/storage/mod.rs:91](file:///d:/workspace/73/ms_manage/src-tauri/src/storage/mod.rs#L91)），默认 `"dark"`
- IPC 契约 [src/lib/ipc.ts:101](file:///d:/workspace/73/ms_manage/src/lib/ipc.ts#L101) 已声明 `theme: "dark" | "light" | "system"`，需扩展枚举
- zustand store [src/stores/app.ts](file:///d:/workspace/73/ms_manage/src/stores/app.ts) 已有 settings 缓存模式可参考

### 当前问题
1. **颜色硬编码**：[tailwind.config.js](file:///d:/workspace/73/ms_manage/tailwind.config.js) 中 `bg / fg / accent / ok / warn / err` 全是固定 hex
2. **透明度写死**：全项目 32 处 `white/x` 透明度（深色专用），浅色主题下不可见
3. **Settings 无主题入口**：[src/pages/Settings.tsx](file:///d:\workspace\73\ms_manage/src/pages/Settings.tsx) 只暴露了 language 和 logs_max_entries
4. **Dashboard 简陋**：纯统计卡片堆叠，无 hero 区域

## Proposed Changes

### 阶段 1：主题系统基础设施（核心）

#### 1.1 扩展主题枚举
**文件**：[src-tauri/src/storage/mod.rs](file:///d:\workspace\73\ms_manage/src-tauri/src/storage/mod.rs)（第 91、142 行）、[src-tauri/src/cmd_settings.rs](file:///d:\workspace\73\ms_manage/src-tauri/src/cmd_settings.rs)（第 10、34 行）
- Rust 默认值从 `"dark"` 改为 `"purple"`（暗紫，向后兼容：旧配置文件 `"dark"` 视为 `"purple"`）
- 新增 4 个有效值：`purple` | `midnight` | `forest` | `light`
- 在 `load_settings` 中做一次 fallback：读到 `dark` / `system` / 未知值 → 映射到 `purple`

**文件**：[src/lib/ipc.ts](file:///d:\workspace\73\ms_manage/src/lib/ipc.ts)（第 101 行）
- `theme` 类型改为 `"purple" | "midnight" | "forest" | "light"`

#### 1.2 Tailwind 改用 CSS 变量
**文件**：[tailwind.config.js](file:///d:\workspace\73\ms_manage/tailwind.config.js)
- 所有颜色值改为 `var(--bg)` / `var(--bg-subtle)` / `var(--bg-elev)` / `var(--fg)` / `var(--fg-muted)` / `var(--accent)` / `var(--accent-hover)` / `var(--ok)` / `var(--warn)` / `var(--err)` / `var(--border)` / `var(--overlay)`（替代 `white/x`）
- 保留 `bg.DEFAULT` 等现有键名，仅替换 value，业务代码无需改动

#### 1.3 定义 4 套主题 CSS 变量
**文件**：[src/styles/global.css](file:///d:\workspace\73\ms_manage/src/styles/global.css)
- 在 `:root` 中用 `[data-theme="xxx"]` 选择器定义 4 套色板（每套包含 12 个变量）
- 暗紫（`purple`）：保留现有色值 `#0f1115 / #161922 / #1c2030 / #e6e8ef / #9aa0b0 / #7c5cff / #6a4cf0 / #22c55e / #f59e0b / #ef4444`，border=`rgba(255,255,255,0.08)`，overlay=`rgba(0,0,0,0.5)`
- 午夜蓝（`midnight`）：bg `#0a1628 / #0f2240 / #16315a`，fg `#dbeafe / #93c5fd`，accent `#38bdf8 / #0ea5e9`，border=`rgba(125,211,252,0.12)`
- 护眼绿（`forest`）：bg `#0f1f15 / #16291e / #1c3526`，fg `#dcfce7 / #86efac`，accent `#34d399 / #10b981`，border=`rgba(134,239,172,0.12)`
- 明亮浅色（`light`）：bg `#f8fafc / #ffffff / #f1f5f9`，fg `#0f172a / #64748b`，accent `#7c3aed / #6d28d9`，ok/warn/err 保持，border=`rgba(15,23,42,0.1)`，overlay=`rgba(15,23,42,0.4)`
- 滚动条颜色也用 CSS 变量：`--scrollbar-thumb` / `--scrollbar-thumb-hover`

#### 1.4 替换 `white/x` 透明度
**文件**：9 个使用 `white/x` 的文件（[Layout.tsx](file:///d:\workspace\73\ms_manage/src/components/Layout.tsx)、[Section.tsx](file:///d:\workspace\73\ms_manage/src/components/Section.tsx)、[Modal.tsx](file:///d:\workspace\73\ms_manage/src/components/Modal.tsx)、[global.css](file:///d:\workspace\73\ms_manage/src/styles/global.css)、[Dashboard.tsx](file:///d:\workspace\73\ms_manage/src/pages/Dashboard.tsx)、[Servers.tsx](file:///d:\workspace\73\ms_manage/src/pages/Servers.tsx)、[Skills.tsx](file:///d:\workspace\73\ms_manage/src/pages/Skills.tsx)、[Logs.tsx](file:///d:\workspace\73\ms_manage/src/pages/Logs.tsx)、[Settings.tsx](file:///d:\workspace\73\ms_manage/src/pages/Settings.tsx)）
- 全局搜索 `white/5` → `border-border`，`white/10` → `border-border`，`white/[0.02]` → `bg-overlay-soft`，`black/50` → `bg-overlay`
- 在 Tailwind 配置新增 `overlay` 与 `overlay-soft` 颜色键（也指向 CSS 变量），保证浅色主题有正确覆盖效果

#### 1.5 主题状态管理
**文件**：[src/stores/app.ts](file:///d:\workspace\73\ms_manage/src/stores/app.ts)
- 新增 `theme: ThemeName`（默认 `"purple"`，启动时从 settings 加载）
- 新增 `setTheme(t)`：写入 `document.documentElement.dataset.theme = t` + 调用 `updateSettings({ ...settings, theme: t })` + 更新 store
- 在 `App.tsx` 启动时同步 `document.documentElement.dataset.theme = theme`

### 阶段 2：Settings 页新增主题选择 UI

**文件**：[src/pages/Settings.tsx](file:///d:\workspace\73\ms_manage/src/pages/Settings.tsx)（General 卡片下方插入新卡片）
- 新增「主题」卡片，4 个色块按钮（每个按钮显示主题主色 + 名称）
- 当前选中主题加 `ring-2 ring-accent` 边框
- 点击立即应用（不需要点保存按钮），toast 提示「主题已切换」
- 走 `useApp.setTheme`，同时更新 `settings.theme` 以保证「保存」按钮能持久化

**文件**：[src/i18n/zh.ts](file:///d:\workspace\73\ms_manage/src/i18n/zh.ts)、[src/i18n/en.ts](file:///d:\workspace\73\ms_manage/src/i18n/en.ts)
- `settings` 节新增 `theme: "主题"` / `"Theme"` 已存在（zh.ts:64, en.ts:16）✓
- 新增 `settings.theme_purple: "暗紫"` / `theme_midnight: "午夜蓝"` / `theme_forest: "护眼绿"` / `theme_light: "明亮浅色"`
- 新增 `settings.theme_switched: "主题已切换"`

### 阶段 3：Dashboard 主界面重设计

**文件**：[src/pages/Dashboard.tsx](file:///d:\workspace\73\ms_manage/src/pages/Dashboard.tsx)
保留现有功能，新增视觉元素：

1. **Hero 横幅**：顶部欢迎区改为渐变背景卡片（`bg-gradient-to-r from-bg-subtle to-bg-elev`），左侧标题 + 副标题，右侧显示代理状态徽章 + 版本号
2. **统计卡片升级**：4 个卡片改为带左侧色条（accent / ok / warn / fg-muted），hover 时微动效 `transition-transform hover:-translate-y-0.5`
3. **快速操作区**：原 "Quick start" 改为 3 个大按钮卡片（添加 MCP / 添加 Skill / 客户端接入），每个带图标和简短描述
4. **已暴露工具区**：保持折叠卡片样式，色块标签改为 `bg-accent/15`
5. 底部版本信息移到 Hero 横幅右侧，避免重复

### 阶段 4：其他 6 个页面微调

**文件**：[Servers.tsx](file:///d:\workspace\73\ms_manage/src/pages/Servers.tsx)、[Skills.tsx](file:///d:\workspace\73\ms_manage/src/pages/Skills.tsx)、[Connections.tsx](file:///d:\workspace\73\ms_manage/src/pages/Connections.tsx)、[Market.tsx](file:///d:\workspace\73\ms_manage/src/pages/Market.tsx)、[Logs.tsx](file:///d:\workspace\73\ms_manage/src/pages/Logs.tsx)、[Settings.tsx](file:///d:\workspace\73\ms_manage/src/pages/Settings.tsx)

统一改造：
- 顶部统一增加 Section 标题样式（部分页面已有，保持一致）
- 表格 hover 行用 `hover:bg-overlay-soft` 替代 `hover:bg-white/[0.02]`
- 边框统一 `border-border`
- 卡片统一 `bg-bg-subtle`

Layout.tsx 侧边栏：
- logo 区 AI 方块保持 accent 色
- 底部版本号样式保留

## Assumptions & Decisions

1. **向后兼容**：旧 settings.json 中 `theme: "dark"` / `"light"` / `"system"` 在加载时映射到 `purple` / `light` / `purple`，避免老用户配置报错
2. **不引入 CSS-in-JS**：纯 CSS 变量 + Tailwind，零运行时开销，构建产物不变大
3. **不引入 next-themes 等三方库**：项目已有 zustand + localStorage，自管足够
4. **主题切换是即时生效**：所有组件已用 Tailwind 语义色（`bg-bg` / `text-fg` / `bg-accent`），改 `data-theme` 即触发重渲染
5. **Rust 后端仅做持久化**：不解析主题值，只透传字符串
6. **不修改 Tailwind 颜色键名**：保留 `bg.DEFAULT` / `bg.subtle` / `bg.elev` / `fg.DEFAULT` / `fg.muted` / `accent.DEFAULT` 等，业务代码零改动
7. **明亮浅色主题**滚动条、overlay、shadow 全部单独定义，保证浅色背景下对比度足够

## Verification Steps

### 类型检查
```powershell
npm run build
```
应 0 error，0 warning。

### Lint
```powershell
npm run lint
```

### 手动验证清单
1. 启动 `npm run tauri:dev`
2. 进入 设置 → 主题，依次点击 4 个色块：
   - 暗紫：bg `#0f1115`，accent `#7c5cff`（与现状一致）
   - 午夜蓝：bg `#0a1628`，accent `#38bdf8`
   - 护眼绿：bg `#0f1f15`，accent `#34d399`
   - 明亮浅色：bg `#f8fafc`，accent `#7c3aed`
3. 切换后检查：
   - 侧边栏 / Dashboard / Servers / Skills / Connections / Market / Logs / Settings 8 个页面色彩协调
   - 所有边框可见（无白色透明边在浅色下消失）
   - 所有 hover 态可见
   - Modal 弹出时遮罩正常
   - 滚动条颜色匹配主题
4. 重启应用 → 主题持久化生效
5. 修改 `~/.ms_manage/settings.json` 把 theme 改回 `"dark"` → 重启应自动映射到 purple，不报错
6. Dashboard 新 Hero 区在 4 个主题下都清晰可读
7. 统计卡片 hover 微动效正常

### 预期风险
- **风险 1**：浅色主题下 lucide 图标默认 `currentColor`，应自动适应，无需额外处理
- **风险 2**：Monaco 编辑器（Skills 页）有自己的主题，需在 Skills.tsx 中根据当前 theme 切换 `theme="vs-dark"` / `theme="light"` — 列入实施时处理
- **风险 3**：Market 页面可能有内联硬编码色，实施时全局搜索 `#` 开头的内联色值确认
