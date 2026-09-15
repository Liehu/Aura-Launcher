# P3-E — Unified Action + Context Menu

## Objective

右键菜单与键盘 Actions 统一基于现有 ActionResolver。

## Input

- P2.4 ActionProposal / ActionDescriptor
- ActionResolver
- launcher-action validate/execute
- P3 action matrix

## Locate First

搜索：

- `ActionProposal`
- `ActionDescriptor`
- `ActionResolver`
- `launcher-action`
- context menus/action lists

## Allowed Changes

- action presentation adapter
- availability mapping
- context menu UI
- keyboard action UI
- labels/icons/shortcuts
- confirmation UI

## Forbidden Changes

UI 禁止：

- direct process spawn
- shell execution
- plugin execution
- destructive filesystem operation
- 修改 ActionResolver semantics
- 修改 launcher-action contract

## Conceptual API

```rust
trait ActionDispatcher {
    fn validate(&self, action: &ActionDescriptor) -> ValidationResult;
    fn execute(&self, action: &ActionDescriptor) -> Result<Effect, ActionError>;
}
```

如现有接口不同，应适配，不得创建第二执行 authority。

## Action Matrix

| Kind | Actions |
|---|---|
| Application | Open, Run as Administrator, Open Location, Pin, Properties, Uninstall |
| File | Open, Open With, Copy Path, Open Location, Rename, Delete, Properties |
| Folder | Open, Open in Terminal, Copy Path, Properties |
| Plugin | Open, Configure, Enable, Disable, Reload, Uninstall, Permissions, Diagnostics |

## Tests

- 每种 result kind
- unavailable action disabled
- right-click 与 keyboard 使用同一 action ID
- execution 必须经 ActionResolver
- destructive action 有 confirmation
- unknown/unresolved 不可执行

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- application context menu
- file context menu
- plugin context menu
- disabled action
- confirmation dialog

## Mandatory Report Item

扫描并报告所有发现的 direct execution path。

## Final Report

```text
Task: P3-E
Status:

Changed Files:
- ...

Action Model:
- ...

Action IDs:
- ...

Tests:
- ...

Screenshots:
- ...

Direct Execution Paths Found:
- ...

Forbidden Boundary Check:
- UI direct spawn:
- UI direct plugin execution:
- ActionResolver bypass:

Issues / Follow-ups:
- ...

Final Verdict:
```
