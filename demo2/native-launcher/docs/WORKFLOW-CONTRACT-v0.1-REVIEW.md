# WORKFLOW-CONTRACT-v0.1 — Design Review（Native Launcher 内部评审）

**Date:** 2026-09-05
**Reviewed:** `demo2/files2/WORKFLOW-CONTRACT-v0.1.md` 草案（27 节）
**对照:** ADR-0014 / ACTION-CONTRACT v0.1 / INV-049/050 / MVP4.0 冻结清单

---

## 评审结论：**APPROVE WITH REVISIONS — 有条件通过**

> **状态更新（2026-09-05）**：R1–R4/m1–m5 已经 review 25 收敛确认并全部吸收，`WORKFLOW-CONTRACT-v0.1.md` 定稿 **FROZEN**（ADR-0015）。收敛时的两处修正：①契约层 FailureClass 保留 8 类，`UnknownActionType` 归一化为 `InvalidInput`（reason 保留）；②`ActionInvocation.action` 类型为 `WorkflowAction`（Reference|Inline 双态），否则 Inline 步骤无法承载。INV-049/050 转生效，新增 INV-052~057（映射避免与 049/050 重复）。

核心架构判断全部正确：Workflow = Orchestration、Resolver = Authorization + Applicability + Freshness、Engine = Effect Gateway、PluginBroker = Effect Executor。线性 Step v0.1、at-least-once 语义、不实现 rollback、Confirmation 不可持久化——这些取舍全部同意。以下 4 个修订必须在冻结前完成（前两个是草案未覆盖的**真实缺口**，不是措辞问题）。

---

## R1（必须修）ActionReference 的解析来源未定义

§2.5 要求执行前重新 resolve，但没有回答：**Resolver 从哪里找到 reference 指向的 ActionDescriptor？**

当前架构中 Command/Descriptor 只存在于 popup session 的活跃结果集里，没有持久化 Command Registry。Workflow 的生命周期显然长于一次 popup——`wf.organize-downloads` 定义后，被引用的 command 很可能不在任何当前结果集中。

**修订要求（冻结进契约）：**

```text
Resolution Source（v0.1 冻结为二选一，按序尝试）：
1. 当前 Popup Session 结果集（同会话内命中）
2. 对 provider_id 发起一次 fresh query，按 (provider_id, command.id) 过滤
两者都未命中 → 新增失败类别 CommandNotFound → 默认 Stop
```

禁止的做法：Workflow 定义时缓存 descriptor（违反 INV-049）、Host 为 workflow 建全局命令注册表（新增持久化子系统，v0.1 不值）。

## R2（必须修）system.* 步骤无法用 ActionReference 表达

§14 示例 Step 2 是 `system.copy`，但 ActionReference 是 `{provider_id, command_id, action_id}` 三元组——**system effect 不属于任何 provider/command**，这个步骤按现模型无法构造。

**修订要求：Step 的 action 字段改为双态：**

```rust
enum WorkflowAction {
    /// 引用已发布的 Command/Action（重查询解析）
    Reference(ActionReference),
    /// 内联 Host-owned descriptor（system.*；仍是不可信输入，每次重解析）
    Inline(ActionDescriptor),
}
```

Inline 与 Reference 同样走 `resolve_descriptor_for` / resolver，不产生第二执行通道；inline 的 `requires` 依旧受 capability 单调性约束（workflow 自身无 capability authority，§16 一致）。

## R3（必须修）Failure Taxonomy 缺两类，且分类来源需绑定到现有错误模型

§11 六类漏掉了 MVP4.0 已存在的：

- **PluginCrashed**（`PluginError::Crashed`）→ 默认 Stop
- **PluginUnavailable**（spawn 失败 / 插件未安装）→ 默认 Retry（有界）后 Stop

同时要求冻结**分类映射表**（分类不能靠 Workflow 猜）：`PluginError::{Timeout→Timeout, ActionFailed→BusinessError, Malformed/Crashed→ProtocolViolation, Spawn→PluginUnavailable}` + resolver 的 `{CapabilityDenied, InvalidInput, UnknownActionType→InvalidInput}` + `context_is_stale→StaleContext`。

## R4（必须修）Confirmation 在无头运行中未定义"谁来确认"

§15 规定 `WaitingForConfirmation`，但 Workflow run 可能由非 UI 路径触发（未来 Workflow/AI），popup 可能已关闭——无人能按第二次 Enter。

**修订要求（v0.1 最小语义）：** Resolution 返回 ConfirmationRequired 时，run 进入 `Paused(Reason::ConfirmationRequired)` 并停止推进；恢复确认只允许来自发起该 run 的 UI 会话（与 MVP3.2 的二次 Enter 同源）。不支持无人值守自动确认——这与 INV-048（外部 confirmed 状态不可信）一致，且避免实现 confirmation service。

---

## 次要修订（冻结时一并处理）

| # | 项 | 修订 |
|---|---|---|
| m1 | Run 级状态机含 `Retrying` | 删除 run 级 Retrying：重试是 **Step 级**状态，Run 保持 Running。状态机少一个迁移面 |
| m2 | `ActionInvocation.workflow_run_id` | 移除。ActionInvocation 是可复用对象（AI/MCP 共享，review 21 §11），run 归属由 `StepRun.invocation_id` 反向关联；否则该类型被 Workflow 绑架 |
| m3 | WF-001~015 清单 | 不全部是 invariant：WF-001~006/009~013 升格 INV-052~（编号顺延）；WF-007/008/014/015 是已有 INV（041/027/037/048）的复述，标注引用即可，避免双份漂移 |
| m4 | CAT-WF-011 persist/reload | 持久化介质是实现细节：v0.1 冻结为"WorkflowRun 可序列化"，不承诺 SQLite |
| m5 | §2.3 observed_context_generation 字段名 | 与 §5 StepRun 的 `resolved_context_generation` 统一为一个术语（建议 `resolved_context_generation`） |

## 确认项（设计正确，冻结）

线性 Step（§21）、at-least-once（§19）、不实现 rollback（§20）、Confirmation 不持久化（§15）、Workflow 无 capability authority（§16）、Plugin action 与 system action 同一 Engine（§17）、三层 id 分离（§18）、`Resolving` 可重入（§8）、默认失败矩阵（§14，含 R3 增补后共 8 类）。

---

## 冻结路径

1. 按本评审修订草案 → Status: **FROZEN v0.1**（ADR-0015 Workflow Orchestration）。
2. INV 映射：R1/R2/R3 的规则 + WF-002/003/004/005/006 → INV-052~057；INV-049/050 从"预定"转"生效"。
3. 实现任务切分（冻结后再拆）：WF 核心类型（domain）→ runner 状态机（core 或新 crate `launcher-workflow`）→ 失败分类映射 → calculator-plus 参考工作流（Reference + Inline 混合三步）→ CAT-WF-001~012。
