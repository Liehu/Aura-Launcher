# P3-A — UI State Architecture

## Objective

冻结并实现 UI 状态机，使 Launcher、Plugin Context、Preview、Context Menu、Plugin Center、Settings 等状态明确隔离。

## Input

- P3 UI/UX Design Specification v1.0
- 当前 UI/window/router/state 实现
- P2.4 backend/platform foundations

## Locate First

搜索实际代码：

- `Launcher`
- `Window`
- `View`
- `State`
- `Settings`
- `Plugin`
- `tauri`
- `egui`
- `iced`
- `slint`

不得假设 UI crate/path。

## Allowed Changes

- UI state enum/state machine
- navigation/router
- launcher window lifecycle
- Escape/back navigation
- UI-only tests
- 必要的最小 adapter

## Forbidden Changes

不得修改：

- `launcher-action/**`
- application discovery/catalog/index
- plugin protocol/control-plane
- DB schema
- plugin runtime/execution
- search ranking

## Required State Model

可复用等价现有类型；不得重复创建 domain model：

```rust
enum UiState {
    Closed,
    Launcher(LauncherState),
    PluginContext(PluginContextState),
    Preview(PreviewState),
    ContextMenu(ContextMenuState),
    PluginCenter(PluginCenterState),
    Settings(SettingsState),
}
```

## API Constraints

- navigation 必须显式且确定性
- UI 不得直接执行 command
- Back/Escape 必须返回安全的 parent state
- presentation state 与 domain state 分离
- UI 不拥有 trust/capability authority

## Tests

必须覆盖：

1. Closed → Launcher
2. Launcher → PluginContext → Esc → Launcher
3. Launcher → PluginCenter → Back
4. Launcher → Settings → Back
5. Launcher → Preview → Esc
6. 非法 transition 被拒绝或规范化

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Test Fixture

- query: `git`
- plugin: `GitHub`
- 一个可 Preview 的 application
- 一个 Settings entry

## Screenshot Acceptance

必须提供：

- Launcher
- Plugin Context
- Plugin Center
- Settings
- Esc/Back 返回 Launcher
- LauncherSearch 中不得出现管理侧栏/设置/插件商店

## Final Report

```text
Task: P3-A
Status: PASS | PARTIAL | BLOCKED

Changed Files:
- ...

Architecture:
- UI state:
- Navigation:
- Reused existing APIs:

Tests:
- cargo fmt --check:
- cargo test --all:
- cargo clippy ...:

Fixtures:
- ...

Screenshots:
- ...

Forbidden Boundary Check:
- launcher-action bypass:
- plugin runtime bypass:
- catalog/index modification:
- DB/schema modification:

Issues / Follow-ups:
- ...

Final Verdict:
- PASS / PARTIAL / BLOCKED
```
