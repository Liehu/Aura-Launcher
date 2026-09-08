# ADR-0006: Capability runtime enforcement (planned)

Status: Proposed (v0.1.x — must exist before capability-gated features ship)
Date: 2026-09-03

## Decision

Capabilities will move from declaration-only to a three-stage model:

```text
Manifest (Declared)
  → Host policy (Granted, per trust level)
  → Runtime enforcement (Broker checks every capability call: ALLOW/DENY)
```

Concretely: the plugin RPC gains capability-gated methods (e.g.
`network.request`, `clipboard.read`); the Host checks the granted set on each
call and answers DENY without invoking anything. A declaration in
`plugin.json` alone grants nothing at runtime.

## Rationale

Today capabilities are metadata: a plugin that "declares no network" can still
spawn its own sockets inside its own process. Runtime enforcement only becomes
possible once capabilities map to RPC methods the Host mediates. The advice
review requires this be on record before any capability-using feature is
built — writing the ADR now prevents "declaration implies grant" from
crystallizing into the protocol.

## Consequences

- Plugin protocol needs a version bump when the first gated method lands
  (INV-009)
- Trust levels (docs/SECURITY.md) decide the default granted set
- Until then, the effective capability model is "isolation only": process
  boundary + confinement + job object (ADR-0005)
