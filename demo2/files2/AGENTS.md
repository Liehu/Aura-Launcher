# AGENTS.md — Native Launcher

## Mission

Build a native, keyboard-first Windows launcher with strict low-memory behavior.

## Read First

Before modifying code, read:

1. `README.md`
2. `docs/01-design-spec-v0.1.md`
3. `docs/02-agentic-coding-development-spec-v0.1.md`
4. `docs/03-test-plan-v0.1.md`
5. `docs/04-mvp-scope-v0.1.md`
6. relevant ADRs

## Architecture Red Lines

DO NOT:

- add Electron
- add CEF
- add WebView/WebView2 for launcher UI
- embed Python/Node runtime into Core
- let plugins access UI internals
- add unbounded global caches
- block UI thread on IO/CPU work
- silently change public contracts
- make architecture changes without ADR

## Implementation Rules

- Keep changes small and localized.
- Prefer existing abstractions over parallel abstractions.
- Every new public API needs tests.
- Every performance-sensitive change needs a benchmark or rationale.
- Every concurrency change needs a concurrency test or explanation.
- `unsafe` requires explicit justification and review.

## Required Validation

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Task Workflow

```text
Inspect -> Plan -> Implement -> Test -> Review -> Report
```

Never skip Inspect or Test.

## Task Scope

Do not fix unrelated problems during a task unless they block the task. Record unrelated findings separately.

## Memory Rules

Treat memory as a first-class budget.

- no unbounded collections
- no permanent plugin runtimes unless explicitly designed
- prefer bounded caches
- measure before optimizing
- compare against baseline for performance PRs

## Output Format for Agent Completion

Every coding task completion must report:

```text
Summary
Changed files
Tests run
Benchmark / memory impact
Architecture impact
Known risks
```
