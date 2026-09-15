# ADR-0020: Plugin Context 是 Launcher 的查询作用域（P3-F）

**状态：ACCEPTED**
**来源：P3.2-UIUX / P3-F + 规范 §10（Plugin Context）、§4（UI State Machine）**

## 背景

规范 §10 要求：用户选择插件结果后进入 `LauncherContext = PluginContext(GitHub)`，
而不是 `NewWindow(GitHub)`。此前 launcher-app 的插件结果 Enter 直接触发主
action（legacy 行为），插件没有作用域交互；P3.1 的窗口插件（window plugin）
另有独立窗口路径，与作用域无关，两者不得混淆。

## 决策

### 架构（强制形状）

```text
LauncherSearch
   │ select plugin result (Ctrl+Enter)
   ▼
LauncherShell（同一个 AppWindow，不开第二窗口）
   ├─ context-hint: ← {plugin} · Esc 返回
   ├─ search box: placeholder = "Search {plugin}…"
   ├─ in-scope results（仅该插件的输出）
   └─ ActionDescriptor → ActionResolver → launcher-action（路径不变）
```

- **UI 状态**：`UiState::PluginContext { plugin_id, parent_query, query,
  selected_result }`（launcher-ui::ui_state）。`enter_overlay` 只允许从
  `Launcher(Search)` 进入；`escape()` 永远回到 `LauncherSearch` 并恢复
  `parent_query`——Esc 是返回，不是退出 Launcher（§10.1）。
- **检索**：`Core::search_plugin_scoped(plugin_id, query, limit)` 只扇出到
  `plugin_identity()` 匹配的那一个 Provider；不参与跨源 ranking 竞争（作用域
  语义），按插件自身 score 提示稳定排序。空查询 = 该插件自己的 discovery
  catalog（不是全局 recent）。
- **生命周期边界（延续 ADR-0019）**：UI 只知道
  `PluginId / Query / SearchResult / ActionDescriptor`。链路是
  `PluginContext → PluginProvider → PluginHost → PluginLifetimePolicy`；
  `PluginLifetime/Ephemeral/Resident` 类型不出现在 launcher-ui、launcher-app、
  launcher-core（源码搜索 guard，仅允许注释提及边界）。
- **执行权限不变**：作用域内的结果仍走既有 `execute-action` →
  ActionEngine → PluginBroker；P3-F 未新增任何执行路径。

### 交互约定

- 进入：**Ctrl+Enter**（选中 plugin 来源结果时）。有意不用 Enter——Enter =
  执行主 action 是既有 UI Contract + VR 基线 + 用户习惯构成的 legacy
  constraint（见 ADR-0021 的同类决策），不为本任务机械改语义。
- 退出：**Esc**（或热键重新唤起时整体重置为 LauncherSearch）。
- 插件作用域内的错误经既有 `SearchResult.errors` → Main.Error 呈现，不污染
  Launcher 其他状态。

## Contract Gate（P3-F 验收七项）

1. PluginContext 不开第二窗口 — PASS（复用 AppWindow；`management`/tool 窗口路径未触碰）。
2. Esc 必须回 LauncherSearch — PASS（状态机单测 + slint Escape 路由）。
3. query isolation — PASS（`search_plugin_scoped_isolates_one_provider`）。
4. plugin error 不污染 Launcher — PASS（errors 走既有 Main.Error 呈现路径）。
5. ActionResolver 路径不变 — PASS（spawn_execute/execute-action 零改动）。
6. PluginLifetime 未进入 launcher-core/UI — PASS（源码搜索零类型引用）。
7. PluginContext 不变成 Plugin Center — PASS（无管理 UI 进入作用域；管理窗口独立）。

## 已知限制 / Follow-up

- 进入键是 Ctrl+Enter 而非规范 §10 字面的"select plugin → context"：等
  legacy Enter 语义被显式决策（ADR-0021 同类）后再统一。
- 作用域内仅隔离检索，不做 per-scope 历史/收藏（spec §24 属后续能力）。
- window 插件（P3.1）行为不受影响：其交互面是受管窗口，不是本作用域。
