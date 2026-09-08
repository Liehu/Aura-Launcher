# ADR-0003: Indexer process boundary

Status: Accepted (v0.1)
Date: 2026-09-03

## Decision

Indexer logic lives in `launcher-indexer` (SQLite + bounded scan) and can run
two ways: embedded in `launcher-app` (current default) or as the standalone
`launcher-indexer-service` process speaking newline-delimited JSON-RPC
(status / rebuild / search / shutdown) over stdio.

## Rationale

- Embedded mode keeps idle memory minimal (no second resident process) while
  the index is small/medium; verified at 214k files.
- The standalone binary exists and is exercised, so moving to a separate
  resident process later is an operational change, not a rewrite.

## Consequences

- In embedded mode, a crash in the indexer crate could affect the app; bounded
  scans + prepared statements are the mitigation until the process split.
- Moving the index out-of-process later requires the named-pipe transport
  (see ADR-0001) and a new ADR.

## Re-evaluation triggers

- Index ≥ 1M files or rebuild > user-acceptable startup cost;
- requirement for SYSTEM-permission USN/MFT watching (Phase 2) — that part
  MUST run as a separate service per design spec.
