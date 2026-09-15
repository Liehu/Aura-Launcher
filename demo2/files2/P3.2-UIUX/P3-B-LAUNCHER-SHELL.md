# P3-B — Launcher Shell

## Objective

实现稳定、键盘优先的 Launcher Shell：搜索框、结果 viewport、选择态、footer hints、focus 与窗口行为。

## Input

- P3-A
- P3 Launcher Shell specification
- existing search/read APIs

## Locate First

寻找当前 launcher view/window 与 search result rendering。

## Allowed Changes

- layout
- search input
- result viewport
- selection
- footer hints
- focus
- sizing/positioning
- UI adapter

## Forbidden Changes

不得修改：

- search backend/ranking
- catalog discovery
- file indexing
- plugin runtime/control-plane
- ActionResolver implementation
- Plugin Center
- Settings IA

## Conceptual Read-only Interface

```rust
trait SearchPresenter {
    fn submit_query(&mut self, query: &str);
    fn results(&self) -> &[SearchResultViewModel];
}
```

必须适配现有架构，不得创建第二套 search engine。

## UI Layout

```text
┌──────────────────────────────────────────────┐
│ Search input                                 │
├──────────────────────────────────────────────┤
│ Result 1                                     │
│ Result 2                                     │
│ Result 3                                     │
│ ...                                          │
├──────────────────────────────────────────────┤
│ Enter Open   → Actions   F1 Preview   Esc    │
└──────────────────────────────────────────────┘
```

LauncherSearch 不得放入 Plugin Store、Settings sidebar、management panel。

## Tests

- empty
- normal query
- one result
- 100+ results
- selection
- Enter
- Esc
- focus restoration

## Fixtures

- 1 application
- 2 builtin commands
- 2 plugins
- 3 files
- 1 web result

## Commands

```text
cargo fmt --check
cargo test --all
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- 1440×900 populated
- compact-height
- empty
- selected result
- footer hints
- long title/subtitle

## Acceptance

- 搜索框视觉权重最高
- keyboard-first
- selection 清晰
- 无管理 UI
- 窗口尺寸适应内容
- 不产生第二套 search pipeline

## Final Report

```text
Task: P3-B
Status:

Changed Files:
- ...

UI Structure:
- Search:
- Results:
- Footer:
- Focus:

Tests:
- ...

Screenshots:
- ...

Forbidden Boundary Check:
- ...

Issues / Follow-ups:
- ...

Final Verdict:
```
