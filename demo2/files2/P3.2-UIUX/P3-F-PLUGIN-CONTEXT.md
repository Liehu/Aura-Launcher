# P3-F — Plugin Context

## Objective

选中插件结果后进入 contextual search mode，而不是打开第二套 launcher UI。

## Input

- P3-A
- P3-B
- plugin search/result API
- P3-C

## Locate First

搜索：

- plugin result handling
- plugin query APIs
- launcher navigation
- plugin runtime adapters

必须复用 Plugin Control Plane。

## Allowed Changes

- PluginContext state
- header/back
- query state
- result adapter
- navigation

## Forbidden Changes

不得修改：

- new runtime
- protocol
- trust/capability semantics
- installation/lifecycle
- direct plugin execution

## State

```rust
struct PluginContextState {
    plugin_id: PluginId,
    query: String,
    parent_query: String,
    selected_result: Option<ResultId>,
}
```

## Flow

```text
Launcher query
    ↓
select plugin
    ↓
PluginContext
    ↓
plugin query
    ↓
select result
    ↓
ActionResolver
    ↓
Esc
    ↓
Launcher
```

## Fixture

GitHub plugin：

- Repositories
- Issues
- Pull Requests
- Settings

## Tests

- enter
- query isolation
- selection
- action dispatch
- Esc back
- repeated enter/exit
- plugin failure does not corrupt launcher

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- plugin context entry
- plugin query
- selected result
- plugin error
- Esc back

## Reject

插件不得变成：

- unrelated app
- second window
- Plugin Center/management UI

## Final Report

```text
Task: P3-F
Status:

Changed Files:
- ...

PluginContext:
- state:
- navigation:
- query isolation:

Tests:
- ...

Screenshots:
- ...

Runtime/Protocol Changes:
- none expected

Issues / Follow-ups:
- ...

Final Verdict:
```
