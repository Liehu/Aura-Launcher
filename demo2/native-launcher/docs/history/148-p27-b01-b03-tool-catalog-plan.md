# 148 — P2.7 Batch 17：B01–B03（Tool Catalog 投影 + Plan Schema + Plan Validator）

> 日期：2026-09-09。范围：P2.7 第十七批（`P2.7 开发设计规范` §13/§14/§39，
> 147 号交接优先级 1）。输入基线：history/147（824 tests）。

## 交付

- `crates/launcher-ai/src/tool_catalog.rs`（新，**B01**）：统一 Tool
  Catalog 投影——actions + workflows 收敛为一个 planner-facing 目录：
  - 稳定 `ref` 键：`provider|command|action`（`|` 分隔，provider id 中的
    `:` 不冲突）/ `workflow|<def id>`——与宿主 TurnExecutor 的解析 key
    完全一致（147 号接线所用 ref 格式从此有了单一出处）；
  - 投影继续丢权（§13：visibility ≠ grant——无 disabled/confirmation/
    授权态）；BTreeMap 按 ref 排序 = 确定性，与 provider 查询顺序无关；
  - `render_json` 有界（默认 200 entries / 16k chars），截断按整条丢弃，
    绝不产出残缺 JSON。
- `crates/launcher-ai/src/plan.rs`（新，**B02**）：版本化 PlanDocument
  （schema_version=1；未知版本 fail-closed，与 MCP profile 同策略）——
  计划跨 LLM turn 存在的宿主编制（D04 计划编辑/恢复的本体）。
  from_proposal（从已验证 AgentProposal 派生）/ to_json / from_json
  （加载即重验：唯一 step_id、有界 id、非空 ref、≤64 步）。
- `crates/launcher-ai/src/plan_validator.rs`（新，**B03**）：对目录的
  fail-closed 校验——每个 step 的 ref 必须存在于 Tool Catalog，否则
  UnknownRef；`workflow|` 前缀与条目 kind 交叉核对（KindMismatch）；
  input 必须是 object 或 null（InvalidInput）。首个失败即返回（文档序）。
- `apps/launcher-app/src/agent_service.rs`：catalog_json 改走 B01 投影
  （actions + workflows 目录统一渲染），删除 ad-hoc JSON 拼装——147 号
  的 ref 格式与 B01 正式合流。
- 测试 8 条：ref 稳定/查得、投影有界且确定性、JSON 往返、未知版本
  fail-closed、编辑后结构校验、UnknownRef/KindMismatch/InvalidInput。

## Gate 结果

- `cargo test --workspace`：**832 passed / 0 failed**（824 → 832，+8）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（更新后优先级）

1. P2.9 Windows Adapter（逐 Adapter 接真实 Win32）
2. P2.6 E02–E05 Slint 画布 VIEW
3. P2.7 D–H（D 线 Approval UI/交互式 clarify §18、E 线 Memory/Privacy、
   F 线 Product UX、G 线 QA、H 线 Release）
4. MSIX 签名（等外部证书）；Pinyin 完整拼音表（优化项）
