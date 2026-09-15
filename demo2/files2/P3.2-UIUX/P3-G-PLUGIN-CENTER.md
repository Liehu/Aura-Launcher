# P3-G — Plugin Center

## Objective

提供独立的插件管理 surface，避免 Launcher 承担插件管理职责。

## Input

- P2.4 Plugin Control Plane
- Plugin SDK/package metadata APIs
- P3-A
- P3 IA

## Locate First

搜索：

- registry/control plane
- existing store/settings UI
- metadata models

## Allowed Changes

- Plugin Center views/components
- Installed / Store / Updates / Disabled
- detail view
- install/update/uninstall/enable/disable UI
- permission/trust display
- diagnostics entry

所有 mutation 必须通过已有 Control Plane。

## Forbidden Changes

不得修改：

- Control Plane semantics
- trust model
- capability rules
- package format
- protocol
- direct runtime invocation

## UI

```text
┌──────────────┬────────────────────┬──────────────────────┐
│ Categories   │ Plugin List        │ Detail               │
│              │                    │ Name / Icon          │
│ Installed    │ GitHub             │ Description          │
│ Store        │ ...                │ Version              │
│ Updates      │                    │ Author               │
│ Disabled     │                    │ Status               │
│              │                    │ Trust / Permissions  │
│              │                    │ Actions              │
└──────────────┴────────────────────┴──────────────────────┘
```

## Detail

至少展示：

- name
- icon
- description
- version
- author
- status
- trust
- permissions
- capabilities
- actions

## Fixtures

- healthy enabled
- disabled
- quarantined
- update available
- invalid package

## Tests

- install
- enable
- disable
- configure
- uninstall
- quarantine
- recovery
- errors

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- Installed
- Store
- Detail
- Disabled
- Quarantined
- install/update error

## Reject

- Plugin Center 逻辑复制 Control Plane
- UI 直接启动 plugin runtime
- Launcher 中塞入管理侧栏

## Final Report

```text
Task: P3-G
Status:

Changed Files:
- ...

Control Plane APIs Reused:
- ...

Views:
- ...

Tests:
- ...

Screenshots:
- ...

Trust/Capability Authority:
- ...

Direct Runtime Invocation:
- none

Issues / Follow-ups:
- ...

Final Verdict:
```
