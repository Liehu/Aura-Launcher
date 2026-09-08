# ADR-0017: UI Behavior Contract（UI-CONTRACT-v0.1 冻结）

日期：2026-09-05　状态：Accepted　来源：review 25（UX 架构）+ review 29（内部评审 R1–R5）+ review 30（收敛）

## Decision

1. **UI-CONTRACT-v0.1 冻结**（`docs/UI-CONTRACT-v0.1.md`，20 节）：三个顶层 UI Mode（Main / Action / Workflow），Confirmation 为 Action 子状态（ConfirmationPending）；AI/Plugin/Context 是内容来源，不是 Mode。
2. **核心节 = State Transition Matrix（§8）**：Slint 实现是把已定义状态机投影出来，而非边写 UI 边决定产品行为。
3. **R1 修复转正**：primary 身份由 host 计算（`Command::primary_action()` = first Ready → `ActionPresentation.is_primary`），UI 禁止按位置推导。
4. **R2 冻结**：Context 变更在 v0.1 不自主变更当前 Action 选择；stale → 拒绝执行 + 非阻断提示；重解析属于后续 invocation 生命周期。
5. **R5 冻结**：每个有意义状态至少一个非颜色表达通道（INV-065 / §16）。
6. **Esc = Cancel**：Action 面板 Esc 关闭并取消 pending confirmation（已实现，`panel-closed` → 清除 pending；launcher dismiss 同样清除）。
7. **新增 INV-062~065**（Workflow UI 只表现状态 / MCP 无独立执行路径 / Context 变更经 Presentation 消费 / 失败呈现消费分类结果）；其余 UI-INV 与现有 INV 重叠，引用不重复。
8. **唯一未实现 mode**：Workflow Runtime Surface（P0-UI-007）；Workflow Builder / AI Chat / MCP UI / Capability Management UI 明确不做。

## Consequences

- Slint 实现成为状态机投影；UI 回归 = 状态迁移矩阵测试（UC-001~010）。
- Plugin/Workflow/AI/MCP 的后续演进不再影响 UI 契约——它们只是内容来源。

---

## Addendum（2026-09-05，UI-CONTRACT 两版本合并）

外部 GPT 版与仓库 canonical 版合并完成（对比分析见 `docs/UI-CONTRACT-COMPARISON.md`）：

1. **吸收 A1–A7**：Main 细分状态（Idle/Searching/Results/Empty/Error）、Workflow.Cancelled、焦点保持规则（Presentation 更新不得无条件重置焦点；Running 不抢焦点）、execution_id 列入 Stable IDs、Enter→Effect P95 ≤ 50ms 新 SLO + 五状态内存记录、UI-ACC-001~050 细粒度验收集（替换 UC-001~010）、显式"不定义"清单。
2. **D1 决议**：Result Row 单击 = **选中并执行 primary**（Windows launcher 惯例），修正外部版"仅选择"表述。
3. **保留 B1–B5**：禁止迁移清单 / Workflow surface 位置语义 / Confirmation status-line 绑定 / 实现状态标注 / INV-062~065 链接。
4. 验收现状：UI-ACC-050 项中 ✅ 39 / ◐ 2 / ⏳ 9（⏳ 集中于 Workflow Runtime Surface 与诊断呈现 = P0-UI-007/008）。
5. canonical 唯一权威：`docs/UI-CONTRACT-v0.1.md`；外部文件标 SUPERSEDED。
