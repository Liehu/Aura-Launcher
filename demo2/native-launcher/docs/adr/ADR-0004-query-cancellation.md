# ADR-0004: Query cancellation / supersession

Status: Accepted (v0.1.x)
Date: 2026-09-03

## Decision

Every keystroke starts a new **generation** via `launcher_core::SearchSession`
(atomic counter). The UI only accepts search results whose generation equals
the newest one; stale results are dropped at delivery time.

## Rationale

Typing "ch→chr→chro→chrome" can produce out-of-order completions; without
supersession the UI can flip back to an older query's results. True
cancellation of in-flight queries is unnecessary at current cost (a full
fan-out over all providers is microseconds, plugin queries have their own
timeout), so we supersede instead of cancel — simpler and race-free.

## Implementation

- `SearchSession::begin() -> u64` on the UI thread when a query is dispatched
- `SearchSession::is_current(query_id)` checked (a) after the search returns,
  (b) before posting to the event loop, (c) inside the UI closure
- Invariant INV-012; unit test `search_session_supersedes_stale_queries`

## Consequences

- Wasted work for superseded queries is bounded and negligible today
- If a provider ever becomes expensive (network plugins), generation-aware
  cancellation must be added at the provider level; supersession semantics
  at the UI stay unchanged.
