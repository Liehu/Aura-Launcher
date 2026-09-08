# ADR-0001: IPC / Plugin protocol

Status: Accepted (v0.1)
Date: 2026-09-03

## Decision

Newline-delimited JSON-RPC-style messages (`launcher-ipc` Request/Response)
over stdin/stdout for external plugin processes, and over stdio for the
indexer service process in the MVP.

## Rationale

- Flow Launcher / Wox use stdio JSON-RPC successfully for process plugins.
- stdio framing is trivial to test and has no named-pipe security surface in
  the MVP; a named-pipe transport can be added behind `launcher-ipc` later
  without changing the message model.
- Messages are versioned by `api_version` in the plugin manifest (currently
  "0.1").

## Consequences

- `launcher-ipc` owns the message schema and result sanitization
  (`MAX_PLUGIN_RESULTS` truncation).
- Timeout and crash handling live in `launcher-plugin-host`, never in Core.
