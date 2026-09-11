# Aura-Launcher 技术栈与界面现状梳理（改造前基线）

> 生成于 2026-09-12 UI Refresh 交付 · 基线 commit：`4025841`（main 分支，改造前最后一个提交）
> 本文是「参考 rubick 完善 UI」改造的知识基础与回滚对照锚点。

## 1. 项目定位

Native Launcher 1.0 —— 键盘优先、原生渲染的 Windows 启动器。**Rust + Slint（软件渲染器），无 Electron/CEF/WebView**。
索引器与插件运行时是独立边界；插件进程按需拉起、空闲自动退出。

## 2. 技术栈清单

| 层 | 技术 | 说明 |
|---|---|---|
| 语言/工具链 | Rust 2021 edition（workspace） | `cargo build/test/clippy/fmt` |
| UI 框架 | Slint 1.17.1（backend-winit + renderer-software） | 声明式 `.slint` 文件；`system-tray` 特性 |
| Windows 互操作 | windows 0.58（Gdi/Shell/Registry/COM…） | 热键、托盘、前台管理、图标提取 |
| 存储 | rusqlite 0.32（bundled SQLite） | index.db / favorites.db / plugins.db 等，WAL 模式 |
| 配置 | TOML（toml 0.8） | `%APPDATA%\NativeLauncher\config.toml` |
| 搜索/索引 | 自研 crates（launcher-search / launcher-indexer） | 模糊匹配在 dev profile 也保持 opt-level 2（热路径） |
| 插件 | stdio JSON-RPC 外部进程（launcher-plugin-api/host） | Manifest v2，Contract v0.1 frozen |
| 发布打包 | scripts/package.py → zip + make_msix.py → msix | artifacts/dist/ |

## 3. Workspace 结构（18 crates + 9 apps）

- `crates/launcher-ui` —— **本次 UI 改造对象**：`.slint` 声明界面 + 只读 viewmodel（架构契约禁止 UI 触达执行器）
- `crates/launcher-core|search|indexer|config|hotkey|context|action|workflow|ai|mcp|plugin-*` —— 领域与运行时
- `apps/launcher-app` —— 主程序（main.rs 108KB：组装、托盘、热键、snapshot/keyboard-walkthrough 验证设施）
- `apps/launcher-indexer-service|launcher-bench|launcher-plugin-cli|example-*|calculator-*` —— 服务与示例

## 4. UI 层现状（改造前）

`crates/launcher-ui/ui/` 13 个 slint 文件 + 6 个原语：

| 文件 | 职责 |
|---|---|
| theme.slint | 唯一视觉令牌源（色板/圆角/间距/字号/行高/窗口尺寸），明暗双主题（`light-theme` bool），动画时长 `anim-ms`（快照模式钉 0） |
| app.slint | 主弹窗：渐进披露（空查询只显示搜索框）、内容驱动高度、主搜索/Action/Workflow/Editor/Tool 五表面互斥切换 |
| search-box.slint | 搜索输入（原为 std LineEdit 包装） |
| result-list.slint / result-row.slint | 结果列表/行（原：44px 行、20px 图标、标题左+副标题右 320px） |
| action-panel / workflow-runtime / tool-surface / graph-editor | Action 面板、工作流运行时、工具会话、编辑器表面 |
| management.slint | 独立管理窗口（780×520，左 1/7 菜单 + 右 6/7 详情，5 个 tab） |
| primitives/ | Divider / FocusRing / SelectedMarker / ShortcutBadge / StateSymbol / ThemedButton |

### 视觉契约约束（改造时必须遵守，已验证未破坏）

- VISUAL-DESIGN-SPEC v0.1/v0.2：组件**必须**用 theme.slint 令牌，禁止散落 hex 字面量
- UI-CONTRACT v0.2：UI 层只渲染 host 提供的模型，无执行器语义（`tests/ui_contract.rs` 源码级扫描强制）
- 窗口高度内容驱动：`padding*2 + search + rows*row-main + context + status`（改令牌即自动适配）

## 5. 基线构建与截图（改造前可复现起点）

- 基线构建：`cargo build -p launcher-ui -p launcher-app` ✅（dev profile，Slint 1.17.1）
- 基线测试：`cargo test -p launcher-ui` ✅ 6/6
- 基线截图：`LAUNCHER_SNAPSHOT_DIR=<dir>` 一次产出 15 个冻结场景 × 明/暗两套 BMP（VR-001..VR-015，640×420 逻辑尺寸）
  - 改造前留存：`demo2/native-launcher/.openclaw-baseline/baseline-before-dark/` 与 `baseline-before-light/`（PNG 副本）
- 已知基线缺陷（本次修复，见 Batch 0）：VR 场景从不设置查询文本，而渐进披露要求 `has-query` 才渲染结果 → **改造前 VR-002（Main.Results）与 VR-001（Main.Empty）字节级相同**（MD5 均为 `173A52FA…`）

## 6. 参照项目 rubick（D:\GitProjects\rubick-master，只读）

Vue 3 + Electron 26 + ant-design-vue 3.2.14 + less。本次提炼的核心 UI 特征：

1. 搜索区：60px 高、左 logo 32px 圆形、40px 无边框输入、右侧 More 操作
2. 结果行：70px 高、图标 + 标题/描述两行 meta 结构、底部 1px 分隔线、选中行 hover 色
3. 关键词高亮（匹配段红色加粗）、空查询显示插件历史网格
4. 设置页：ant-design 行式设置项 + 分组卡片
5. 截断策略：描述超 80 字符取 63 + … + 末 14

**只借鉴布局与交互观感，未复制任何源码/图标/资源**（许可与版权隔离）。
