# P3-C — Search Result Contract + Ranking

## Objective

建立统一、确定性的 SearchResult 与 ranking，使应用/内置命令不会被插件、文件等低价值结果淹没。

## Input

- P3 SearchResult specification
- CatalogProvider
- existing search implementation
- plugin metadata
- file index

## Locate First

搜索：

- `SearchResult`
- `Search`
- `CatalogProvider`
- ranking/sort
- plugin adapters

不得创建第二套搜索管线。

## Allowed Changes

- unified DTO/view contract
- ResultKind / ResultSource（缺失时）
- ranking signals
- scoring
- priorities
- favorite/usage/recency
- fixtures/tests

## Forbidden Changes

不得修改：

- discovery/catalog reconciliation
- file indexing
- plugin protocol/control-plane
- action execution
- DB migration（除非已有 schema 明确支持 ranking signals；否则采用内存/default signals 并记录 ADR）

## Conceptual Contract

```rust
struct SearchResult {
    id: ResultId,
    title: String,
    subtitle: Option<String>,
    icon: IconRef,
    source: ResultSource,
    kind: ResultKind,
    score: f64,
    ranking: RankingSignals,
    actions: Vec<ActionDescriptor>,
    metadata: ResultMetadata,
}
```

## Ranking Signals

- exact_match
- prefix_match
- fuzzy_match
- usage_score
- recency
- source_priority
- builtin_priority
- favorite

要求：

- deterministic
- tunable
- weights 集中管理
- 同一输入得到稳定顺序

## Fixtures

### F1 — notepad

包含：

- Notepad.exe
- Notepad++
- README.txt
- web result
- unrelated plugin

预期：exact/builtin application 优先。

### F2 — git

包含：

- git.exe
- GitHub builtin/plugin
- repository result
- git file

预期：高价值 application/command 优先，低价值 plugin 不得刷屏。

### F3 — exact filename

当 query 明显是文件名时，精确文件可超过无关 application。

### F4 — tie

相同 query/signals 多次运行，结果顺序必须一致。

## Commands

```text
cargo test --all
cargo test --all ranking
cargo clippy --all-targets --all-features
```

## Screenshot Acceptance

- `notepad`
- `git`

首项必须符合 ranking expectation；插件不得因为“可搜索”而天然置顶。

## Acceptance

- 统一 SearchResult
- deterministic scoring
- weights 单点配置
- 不同步重新 discovery
- 不执行 action
- 不破坏 P2.4 catalog/search boundary

## Final Report

```text
Task: P3-C
Status:

Changed Files:
- ...

Search Contract:
- ...

Ranking Formula:
- ...

Weights:
- ...

Fixtures:
- F1:
- F2:
- F3:
- F4:

Tests:
- ...

Screenshots:
- ...

Architecture Boundary:
- synchronous rediscovery:
- execution:
- catalog ownership:

Issues / Follow-ups:
- ...

Final Verdict:
```
