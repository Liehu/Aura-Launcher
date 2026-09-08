# ADR-0005: Plugin executable confinement + process-tree isolation

Status: Accepted (v0.1.x)
Date: 2026-09-03

## Decision

1. **Executable path confinement**: a plugin manifest's `executable` must be a
   relative path without `..`/absolute/UNC components, and (when the file
   exists) its canonicalized path must stay inside the canonicalized plugin
   directory. Enforced by `launcher_plugin_host::resolve_executable` before
   spawn (INV-013).
2. **Process-tree isolation**: each plugin process is assigned to a Windows
   Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; killing/locking-up/
   dropping a plugin terminates the whole tree (INV-014).
3. **Child environment**: `current_dir` is the plugin directory; environment is
   inherited from the host (MVP simplification, see re-evaluation); stderr is
   discarded.

## Rationale

A local user plugin is untrusted code (docs/SECURITY.md trust model). Without
confinement, a manifest could point at `..\..\anything.exe` or a UNC share;
without a job object, `plugin.exe -> python.exe -> child.exe` trees leave
orphans after a timeout kill.

## Known limitations / re-evaluation triggers

- **Spawn-to-assign race**: the job is assigned right after `spawn()`; a plugin
  could in theory spawn a child in that window. Hardening path: create the
  process suspended (`CREATE_SUSPENDED`) via `CreateProcessW`, assign, then
  resume — required before shipping any code-execution-capable plugin tier.
- Environment inheritance leaks host env vars; switch to a minimal allowlist
  when network/privileged plugins arrive.
- Symlink/junction inside the plugin directory pointing outward is caught by
  canonicalize at spawn time; re-resolving per spawn is intentional (cheap).
