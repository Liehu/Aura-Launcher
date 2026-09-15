# P3-I — Visual System + Accessibility

## Objective

在 IA 与 interaction 稳定后建立统一视觉系统。

## Input

- P3-B
- P3-D
- P3-F
- P3-G
- P3-H
- existing visual/theme conventions

## Locate First

必须确认真实 UI framework/theme/token system，并优先复用已有 components。

## Allowed Changes

- design tokens
- typography
- spacing
- icons
- row heights
- radius
- border/shadow
- hover
- selected
- focus
- disabled
- error
- loading
- restrained animation
- accessibility

## Forbidden Changes

不得修改：

- IA
- ranking
- state machine
- action semantics
- plugin lifecycle/control-plane

## Required States

- Loading
- Empty
- Error
- Disabled
- Unavailable
- Recovering
- Selected
- Focused

## Accessibility

至少验证：

- keyboard-only
- visible focus
- high contrast
- forced colors（框架支持时）
- semantic labels（框架支持时）
- focus 不得仅靠颜色表达

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- normal
- compact
- high contrast / accessibility state
- loading
- empty
- error
- disabled
- selected/focused

## Visual Reject Conditions

- gratuitous gradients
- oversized cards
- excessive animation
- visual noise
- 为了视觉效果破坏 keyboard flow

## Final Report

```text
Task: P3-I
Status:

Changed Files:
- ...

Visual Tokens:
- ...

Components Updated:
- ...

Accessibility:
- keyboard-only:
- focus:
- contrast:
- semantic labels:

Tests:
- ...

Screenshots:
- ...

Regression:
- ...

Issues / Follow-ups:
- ...

Final Verdict:
```
