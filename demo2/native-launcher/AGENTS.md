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
Performance Impact:
  Memory: unchanged | <delta>
  Startup: unchanged | <delta>
  Search: unchanged | <delta>
  Benchmark: before ... after ... delta ...   (run `launcher-bench check`)
  或 N/A + Reason: <changed code is not on any hot/IO/allocation/render path>
  禁止只写 "benchmark 没明显变化"；必须 either measured or reasoned no-impact
Architecture impact
Known risks
```

## Plugin Contract 门禁（ADR-0007/0009）

- 任何修改 `launcher-ipc` / `launcher-plugin-api` / `launcher-plugin-host` 的改动：`apps/calculator-plugin` E2E（Canonical Reference Plugin）+ Plugin Contract Test Kit（`apps/example-testplugins/tests/contract.rs`）未全部通过时，不得宣布完成。
- Plugin Contract v0.1 已冻结（2026-09-04）：breaking 变更必须 bump 到 v0.2 并新增 ADR；additive 变更走 v0.1.x。
- 插件作者 API 边界（ADR-0010）：插件只依赖 `launcher-plugin-api`（Rust）或 `plugins/python/launcher_plugin.py`（Python SDK）；SDK/示例/文档不得依赖或 re-export `launcher-plugin-host` / `launcher-core` 等内部 crate 类型。
- `runtime.type → 启动命令` 的映射只允许存在于 `launcher-plugin-host::runtime::resolve_launch`；新增运行时（node/wasm）只改这一处并补 LaunchPlan 测试。
- Runtime dispatch 红线（ADR-0010/0011）：`runtime.type` 的 match/dispatch 只允许出现在 `launcher-plugin-host::runtime::resolve_launch`；`PluginHandle::spawn` 及任何上层编排代码禁止出现 runtime 分支。
- Plugin Contract v0.1 已冻结且协议扩展期结束（review 14）：下一步是 COMMAND-CONTRACT-v0.1 / ACTION-CONTRACT-v0.1（`docs/` 下 Proposed 状态），实现前必须先过设计评审 + 新 ADR。
- COMMAND-CONTRACT-v0.1 / ACTION-CONTRACT-v0.1 已修订至 **Revision 2**（review 15 语义全部吸收：Action Resolution 安全边界、requires⊆capabilities、CapabilityDenied 归 Action Execution Result 而非 RPC error、provider_id Host 权威等）。两契约已经 ADR-0011 冻结（v0.1）；calculator-plus 为 Canonical Reference（只实现 system.* action，`plugin.*` 留给独立 ADR）。resolver 的新增 action type 只改 `launcher_domain::resolve_descriptor` 并补映射测试。
- Domain 门禁（ADR-0011，review 17 §17）：修改 `launcher-domain` / `launcher-action` / COMMAND- 或 ACTION-CONTRACT 语义 → 必须跑 calculator-plus conformance（`apps/calculator-plus/tests/`：mvp3_acceptance + domain_contract_kit CAT-001~011）；修改协议/runtime → calculator conformance（Plugin Contract Test Kit）。参考插件分工：calculator = Protocol conformance，calculator-plus = Domain conformance。
