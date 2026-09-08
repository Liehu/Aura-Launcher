# SECURITY.md

## Principles

- least privilege
- process isolation
- explicit capabilities
- fail closed
- untrusted plugin input
- no arbitrary plugin control over launcher UI

## Plugin Capabilities

Examples:

```text
filesystem.read
filesystem.write
clipboard.read
clipboard.write
network
shell.execute
process.spawn
notifications
ui.render
```

Capabilities must be explicit in the manifest and checked by the host.

## High-Risk Operations

These must require explicit Action handling:

- shell execution
- process spawn
- arbitrary file write
- network access

## Failure Isolation

A plugin failure must not terminate Core or UI.
