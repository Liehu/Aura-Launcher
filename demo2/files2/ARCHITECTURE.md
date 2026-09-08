# ARCHITECTURE.md

Canonical high-level architecture is defined in `docs/01-design-spec-v0.1.md`.

## Dependency Direction

```text
launcher-domain
    ^
    |
launcher-search / launcher-context / launcher-action
    ^
    |
launcher-core / launcher-ipc
    ^
    |
launcher-ui / launcher-indexer / launcher-plugin-host
```

The exact dependency graph may become stricter; it must never become cyclic.

## Ownership

| Area | Owner crate |
|---|---|
| Domain types | launcher-domain |
| Search | launcher-search |
| Context | launcher-context |
| Effects/actions | launcher-action |
| IPC | launcher-ipc |
| UI | launcher-ui |
| Indexing | launcher-indexer |
| Plugin process boundary | launcher-plugin-host |
| Core orchestration | launcher-core |

## Architecture Change Policy

If a change alters process boundaries, public protocols, plugin model, DB schema strategy, or UI technology, create/update an ADR first.
