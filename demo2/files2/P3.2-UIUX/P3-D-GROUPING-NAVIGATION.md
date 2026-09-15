# P3-D — Result Grouping + Keyboard Navigation

## Objective

建立清晰的语义分组与确定性的键盘遍历。

## Input

- P3-B
- P3-C

## Locate First

寻找 result list/group rendering 与 keyboard event routing；复用既有 IDs/ranking。

## Allowed Changes

- grouping presentation
- headers
- cursor
- keyboard events
- selection
- scrolling

## Forbidden Changes

不得修改：

- ranking weights
- search provider
- plugin runtime
- action execution

## Group Model

```rust
enum ResultGroupKind {
    Applications,
    Commands,
    Files,
    Folders,
    Plugins,
    Web,
    Other,
}
```

## Navigation

- header 不可选择
- Up / Down
- Home / End（若框架支持）
- PageUp / PageDown 可选
- selected row 必须始终可见
- empty group 自动隐藏
- 只有一个 group 时可隐藏 header

## Fixtures

- one group
- all groups
- empty group
- 1000 results
- disabled/unavailable result
- rapid Up/Down

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- multi-group
- one-group
- selected near bottom
- long list

## Reject Conditions

- header 抢走 focus
- selected row 不明显
- keyboard traversal 不确定
- grouping 修改 ranking semantics

## Final Report

```text
Task: P3-D
Status:

Changed Files:
- ...

Group Model:
- ...

Keyboard Model:
- ...

Tests:
- ...

Screenshots:
- ...

Rejected Conditions Checked:
- ...

Issues / Follow-ups:
- ...

Final Verdict:
```
