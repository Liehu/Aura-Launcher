# ADR-0011: Command & Action Contract v0.1 冻结与 MVP3.0 实现

日期：2026-09-04　状态：Accepted　来源：review 15-mvp3-0.1（Revision 2 修订）+ review 16-mvp3-0.2（有条件通过 → 语义修补后批准）

## Context

COMMAND-CONTRACT / ACTION-CONTRACT 两份 Proposed 设计稿经两轮评审（15、16）后，语义边界已钉死：信任边界（untrusted descriptor → resolver → trusted resolved action）、身份三层（command.id / action.id / action.type）、capability 单调性、eligibility vs permission。评审批准进入实现。

## Decision

1. **两份契约冻结为 v0.1**（文档 Status 已更新）；后续 breaking → v0.2，additive → v0.1.x。
2. **Domain 层实现**（`launcher-domain`）：
   - `ActionDescriptor{id, title?, type, input, requires[]}`——插件 wire 层的不可信 action 声明。
   - `resolve_descriptor(d, granted) -> Result<Action, DescriptorError{UnknownActionType|InvalidInput|CapabilityDenied}>`——纯函数 resolver，UI 无关（INV-034 方向），AI/Workflow 可复用。`system.*` 五种类型映射现有 `ActionKind`；`plugin.*` v0.1 reserved/unsupported，等同未知类型处置。
3. **Host 集成**（`launcher-plugin-host::query`）：结果条目的 `actions[]` 支持 legacy 字符串与 descriptor 对象两种形态；逐项解析，畸形/被拒 action 单独丢弃 + WARN（INV-031 fault containment），item 永不连坐；`provider_id` 由 Host 生成（INV-029）。wire 形态扩展属于 Plugin Contract v0.1.x additive（结果条目 actions 元素从 string 扩展为 string|object，向后兼容）。
4. **Invariant 落档**：INV-026/027/029/031 措辞按 review 16 强化（monotonicity、blast radius、source provenance）；新增 INV-033（engine 只接受 ResolvedAction）与 INV-034（resolver UI-independent）。
5. **参考插件**：`apps/calculator-plus`（Canonical，system.* only）——primary Copy / secondary Insert / Open History + 故意注入的未知 `plugin.*` secondary，验收 `tests/mvp3_acceptance.rs`（评审 §24 七条中的 1/2/3/4/6/7 条；⑤"unknown primary disabled/fallback"由 resolver 单测覆盖 UnknownActionType 路径）。

## 明确不做（⏸ 维持）

UI Schema、`plugin.*` action RPC（需 v0.1.x additive ADR + 第二参考插件）、Store、Context supply to plugins、Node SDK、WASM、AI/MCP/Workflow。

## Consequences

- UI Schema、Workflow、AI Agent、MCP Tool 将来都只是同一模型上的不同 **Command Producer**，执行安全边界（INV-026/033）无需重设计。
- `requires_context` 语义已冻结但 v0.1 不实现判定（context supply ⏸）；字段保留在契约中。
