# 151 — P2.7 Batch 18：D 线 AI Approval（D01/D02/D03/D04/D05）

> 日期：2026-09-10。范围：P2.7 第十八批（`P2.7 开发设计规范` §19-§21/
> §23，150 号交接优先级 1）。输入基线：history/150（835 tests）。

## 交付

- `crates/launcher-ai/src/approval.rs`（新，**D01/D03/D05/D04**）：
  - **D01 模型**：`ApprovalRequest`（goal + 完整 plan + 创建/过期时间）、
    `Decision::{Approve, ApproveEdited(PlanDocument), Reject}`、
    `ApprovalGate`（run-scoped：一次一个 pending，新请求取代旧请求）；
  - **D03 安全边界（§21 冻结红线）**：approve 只授权**这一个 plan 一次**
    ——request 单次使用（重放 = AlreadyDecided）、未知 id = Unknown、
    决策在 OS/执行边界之前；无 `Approve → 永久授权` 通路；
  - **D05 过期/取消**：默认 TTL 5 分钟；过期/显式 cancel 后请求不可回答
    （fail-closed）；`pending_request(now)` 供宿主查询存活请求；
  - **D04 计划编辑**：`ApproveEdited` 携带替换 PlanDocument——先过 B02
    结构校验再生效，编辑后的 plan 无新增权威、走正常执行链。
- **§23 接线（agent_loop.rs）**：新增 `ApprovalSink` trait + 默认
  `NoApprovalSink`（fail-closed：无人应答 = 取消，绝不自动执行）；
  `run_agent` 在 pipeline 产出 proposal 后、任何执行前调用
  `request_approval_if_required`——需要审批的 plan 阻塞等待宿主决定；
  Reject/过期/无应答 → Cancelled（零步执行），Approve/ApproveEdited →
  以生效 steps 继续。
- **D02 审批 UI（launcher-app/agent_service.rs）**：`UiApprovalSink`
  复用 Workflow Runtime Surface 展示待审 plan（`!` 标记需审批步），
  Enter（confirm 按钮）= Approve、Esc/超时 = 拒绝；`on_workflow_confirm`
  同时路由 workflow 与 agent 两条确认通道；agent 线程阻塞等待，150ms
  轮询 cancel/TTL。
- 测试 7 条：需审批才出请求、单次使用、过期/取消 fail-closed、编辑计划
  校验后生效/非法编辑拒绝、reject 零执行、无 sink 阻断执行、
  approve/reject 路由执行。

## Gate 结果

- `cargo test --workspace`：**842 passed / 0 failed**（835 → 842，+7）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（更新后优先级）

1. P2.7 E 线（Memory/Privacy：E01-E03 Session/Run Memory/偏好——
   AgentSessionStore 已备；E06 注入防御 prompt.rs 已有 sanitize 基础）
2. P2.7 F 线（Product UX：AI 搜索面/会话面）
3. P2.7 G/H（QA：G04 Approval Security Tests 本批已含核心用例；
   Release：MSIX 签名等外部证书）
4. Pinyin 完整拼音表（优化项）
