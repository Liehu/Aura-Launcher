# P3-J — UX Integration + Regression Gate

## Objective

完成 P3-A..I 的最终集成、回归与冻结。

本 Task 不新增 capability。

## Input

- P3-A..I
- P3 UI/UX Design Specification v1.0
- P2.4 release gate

## Locate First

审查：

- all diffs
- architecture boundaries
- direct execution
- duplicate search engines
- duplicate plugin registries
- UI-side mutation

## Allowed Changes

- integration fixes
- regression fixes
- fixtures
- UX test harness
- documentation

## Forbidden Changes

不得新增：

- plugin APIs
- search capabilities
- workflow
- AI
- MCP
- automation
- P2.4 contract changes

## Required Commands

```text
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test --all
```

此外必须执行仓库 CI/docs 中已有的 UI/build/package commands。

## UX Fixture Suite

1. Launch empty
2. `notepad` ranking
3. `git` ranking
4. right-click app
5. right-click file
6. right-click plugin
7. → actions
8. F1 preview
9. plugin context
10. plugin query
11. execute plugin result
12. Esc return
13. Tray → Plugins
14. Tray → Settings
15. disable/enable fixture plugin
16. quarantined plugin
17. settings save/cancel
18. launcher close/reopen

## Screenshot Evidence

必须提供：

- empty launcher
- ranking
- grouped results
- context menu
- plugin context
- Plugin Center installed/detail
- Settings
- Tray
- loading
- error
- disabled
- quarantined

## Architecture Audit

必须满足：

- UI never bypasses ActionResolver
- UI never directly executes plugins
- UI never synchronously rediscovers apps
- UI does not own trust/capability authority
- Plugin Center 与 Settings 分离
- P2.4 frozen contracts unchanged

## PASS Criteria

只有同时满足以下条件才 PASS：

- P3-A..I acceptance criteria 全部通过
- all tests pass
- screenshots exist
- no forbidden boundary violations
- no unexplained architecture drift

## Final Report

```text
Task: P3-J
Status: PASS | PARTIAL | BLOCKED

Implemented:
- ...

Regression:
- cargo fmt --check:
- cargo clippy --all-targets --all-features:
- cargo test --all:
- repository CI/build/package commands:

UX Fixture Results:
1. Launch empty:
2. notepad ranking:
3. git ranking:
4. app context:
5. file context:
6. plugin context menu:
7. actions:
8. preview:
9. plugin context:
10. plugin query:
11. plugin action:
12. Esc return:
13. tray plugins:
14. tray settings:
15. plugin disable/enable:
16. quarantined:
17. settings save/cancel:
18. close/reopen:

Architecture Audit:
- ActionResolver bypass:
- Direct plugin execution:
- Synchronous rediscovery:
- Trust/capability ownership:
- Plugin Center / Settings separation:
- P2.4 contract changes:

Screenshots:
- ...

Known Issues:
- ...

Final Verdict:
- PASS / PARTIAL / BLOCKED
