# ADR-0002: UI and Core merged in one process for MVP

Status: Accepted (v0.1)
Date: 2026-09-03

## Decision

`launcher-app` merges the Slint UI and Core in one process for v0.1, while
keeping the module boundary: the app depends on `launcher-core` and
`launcher-ui` crates only; Core never depends on the UI.

## Rationale

Allowed explicitly by design spec 3.1 ("MVP 可以将 UI 与 Core 暂时合并") to
shorten the validation cycle for the first vertical slice.

## Consequences

- Idle memory target is measured on `launcher-app` as a whole.
- Splitting to `launcher-ui.exe` + `launcher-core.exe` with a real IPC
  transport requires a new ADR (and a named-pipe transport in
  `launcher-ipc`).
