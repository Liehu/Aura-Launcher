# P3-H — Settings + System Tray

## Objective

建立统一 Settings IA，并补齐 System Tray 管理入口。

## Input

- P3-A
- Settings IA
- existing tray
- config APIs

## Locate First

搜索：

- settings router
- tray/icon lifecycle
- existing config services

## Allowed Changes

### Settings

- General
- Search
- Appearance
- Hotkeys
- Sources
- Plugins
- Advanced
- About

### Tray

- Open
- Plugins
- Settings
- Reindex
- Pause Hotkey
- Check Updates
- About
- Exit

只做 presentation/router wiring 与已有 service 调用。

## Forbidden Changes

不得修改：

- settings schema redesign（除非严格必要并记录 ADR）
- ranking
- plugin protocol/control-plane semantics
- catalog/index implementation

## API Constraints

Tray 必须调用已有 service：

- Reindex → existing index service
- Exit → existing graceful shutdown
- Plugins → Plugin Center navigation
- Settings → Settings navigation

不得复制 service。

## Tests

Tray：

- left click
- menu
- Plugins
- Settings
- Reindex
- Pause/Resume
- Exit

Settings：

- navigation
- load
- save
- cancel
- back
- invalid value

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- tray menu
- settings top-level
- each major settings category
- Plugins navigation进入 Plugin Center，而不是复制一份插件管理 UI

## Mandatory Report

验证：

- shutdown path
- reindex path
- no duplicate service

## Final Report

```text
Task: P3-H
Status:

Changed Files:
- ...

Settings IA:
- ...

Tray:
- ...

Existing Services Reused:
- ...

Tests:
- ...

Screenshots:
- ...

Shutdown Path:
- ...

Reindex Path:
- ...

Duplicate Services:
- none / found

Issues / Follow-ups:
- ...

Final Verdict:
```
