# UI-CONTRACT-v0.1 实现一致性审计（Implementation Conformance Audit）

**Date:** 2026-09-05
**Method:** 逐条对照 `docs/UI-CONTRACT-v0.1.md`（含 Addendum 1）与当前实现（`crates/launcher-ui/ui/app.slint`、`crates/launcher-ui/src/lib.rs`、`apps/launcher-app/src/main.rs`）
**图例：** ✅ 符合　◐ 部分符合　❌ 缺口（已修/待修）　⏳ 范围外（P0-UI-007/008 待实现）　N/A 不适用

---

## 审计结果总览

**154 项测试全绿前提下，逐条审计发现 1 个 ❌ 行为缺口（已当场修复）+ 4 个 ◐ 部分符合 + 1 块 ⏳ 范围外 + 2 条代码质量观察。无其他违反项。**

| 契约节 | 主题 | 结论 |
|---|---|---|
| §1 Scope/Principles | 瞬时工作空间 / 职责六词 / 禁止清单 | ✅ |
| §2 UI Modes | 3+1 mode，同窗口 | ✅ |
| §3.1 Main 状态 | Idle/Searching/Results/Empty/Error | ◐（F1） |
| §3.2 Action 状态 | Normal/ConfirmationPending | ✅ |
| §3.3 Workflow 状态 | （surface 未实现） | ⏳ |
| §3.5 State Authority | UI 不推断 | ✅ |
| §4 Presentation Models | 字段级 | ◐（F2：CommandPresentation.selected） |
| §5 Focus Model | 焦点保持 / disabled 非 focus target | ✅（含一处确认） |
| §6 Keyboard | 全部键位 | ✅（含 Ctrl+Shift+letter shortcut 派发） |
| §7 Mouse | D1 单击执行 / disabled 无效果 / step 点击不重执行 | ✅（Workflow 部分 ⏳） |
| §8 Transition Matrix | 逐行对照 | ❌→✅（F1 已修）+ ◐（F3） |
| §9 Search/Result | 增量/generation/identity | ✅ |
| §10 Context Presentation | hint 能力 | ◐（F4） |
| §11 Action Presentation | 顺序/primary/disabled/hidden/shortcut | ✅ |
| §12 Confirmation | 两阶段/不持久/Esc=Cancel | ✅（本轮修复后） |
| §13 Workflow Runtime | surface | ⏳ P0-UI-007 |
| §14 AI Proposal | 普通通道路径 | ✅（管道；UI 标记 ⏳） |
| §15 Runtime/Failure | Level 1 | ◐（F5：Level 2/3 = P0-UI-008） |
| §16 Accessibility | 非颜色通道 | ◐（F2 相关：selected 仅颜色） |
| §17 Stable ID | id vs index | ◐（F6：信号通道） |
| §18 Forbidden | 逐条 | ✅ |
| §19 Performance | SLO | ✅（bench 已有 action_panel_us） |
| §20 Acceptance | UI-ACC-001~050 | 见 UI-CONTRACT 内标注（✅39/◐2/⏳9） |

---

## 发现明细

### F1 ❌→✅（已修复）更换选择未取消 pending confirmation
契约 §8.2：`更换选择 → 取消 pending`。原实现：`on_selection_changed` 只更新 `selected` 并重建面板，`pending_confirmation` 保留——用户对命令 A 进入 ConfirmationPending、换到 B 再换回 A 后，一次 Enter 即静默确认（跳过 re-arm）。**已修复**：selection 变化时清除 pending（`main.rs` on_selection_changed）。Esc 路径（panel-closed）与 launcher dismiss 的清除已在此前实现。

### F2 ◐ selected 状态仅颜色表达（§3.1/§4.1/§16/UC-039）
结果行选中只用 accent 背景色；disabled 行有 reason 文本 ✓。契约 §16 要求非颜色通道。**待办**：选中行补文本/焦点标记（如 "▸" 前缀或 Slint accessible API），归入 P0-UI-003。另 `CommandPresentation.selected` 字段在契约模型中存在、实现用 selected-index 表达——逻辑 identity 合规（执行走 command_id），字段化属实现重构项。

### F3 ◐ Main.Searching / Main.Error 不可表达（§3.1）
契约要求 Presentation 层能表达 Searching 与 Error。当前：查询无 searching 指示（§9.1 允许无 spinner，但"能表达"未满足）；Provider 查询失败静默空结果，不进入状态行。**待办（P0-UI-003）**：status line 复用为 searching 指示；provider 查询失败写入状态行（非阻断）。

### F4 ◐ Context Hint 能力缺失（§10）
契约允许轻量 context 提示（`📁 Documents`）。当前 UI 无任何 context 呈现（默认隐式 ✓，但"确有帮助时提示"的能力未实现）。**待办（P0-UI-004）**：context 刷新时同步一个 `context-hint` presentation 字段。

### F5 ◐ 失败呈现只有 Level 1（§15）
status line = Inline（Level 1）。Level 2（step/reason/attempt 详情）与 Level 3（诊断：plugin/execution_id/failure class）未实现。**待办（P0-UI-008）**。

### F6 ◐ selection 同步信号使用 index（§17 边缘）
`selection-changed(int)` 传递行索引供 Rust 同步 `selected`。执行 identity 全部走 command_id/action_id ✓（契约合规）；索引仅作 UI↔Rust 同步信号。建议后续把信号改为 command_id（顺序：与 P0-UI-003 一并）。

### ⏳ Workflow Runtime Surface（§2.3/§3.3/§13 + UC-024~030/042/050）
整块待实现（P0-UI-007），非违规。runner/分类/暂停语义全部就绪并有 CAT-WF 测试锚点。

### 代码质量观察（非契约违反）
1. **键盘处理双份**：`capture-key-pressed` 与 `key-pressed` 存在两份几乎相同的 Esc/箭头逻辑（capture 先行 accept，key-pressed 为死代码路径）。漂移风险，建议合并为仅 capture 一份。
2. **Result row click 未先更新 selection**：直接执行该行 primary，行为等价（执行目标就是该行），但选中高亮不随点击移动——与"select + execute"字面顺序略有出入，建议 click 时同步 selected-index。

---

## 修复记录（本轮）

| 修复 | 契约依据 |
|---|---|
| `on_selection_changed` 清除 `pending_confirmation` | §8.2 / §12 / INV-048 |

## 建议的收尾顺序

1. （已修）F1。
2. 小任务批：F2 selected 非颜色标记 + F6 信号改 id（同触 P0-UI-003）。
3. P0-UI-003/004：Searching/Error 状态 + Context hint（F3/F4）。
4. P0-UI-007/008：Workflow Runtime Surface + 失败三级呈现（⏳/F5），完成后 UI-ACC 全表转 ✅。
5. 代码质量：合并双份键盘处理。

---

## 收尾实现（review 31-mvp6-0.3 顺序，2026-09-05）

| 项 | 实现 |
|---|---|
| F2 ✅ | selected 非颜色通道：结果行与面板行均加 `▸` 文本标记（UI-CONTRACT §16） |
| F6 ✅ | `selection-changed` 信号改为携带 **command_id**（业务身份）；index 在 Rust 侧由 id 反解，不再跨边界 |
| F3 ✅ | Main.Searching（`⌛ Searching` 非阻断状态行）+ Main.Error（`⚠ Search failed: <provider errors>`；`SearchResult.errors` 经 `Provider::take_last_error` 从 PluginProvider 上抛——空结果+非空错误 ≠ No results） |
| F4 ✅ | Context Hint：`context-hint` 底栏（`📁 <folder>` / `✦ <app>`），由 Core 数据派生（INV-064），附 `Ctrl+↓ Actions` affordance |
| F5 ◐ | Workflow 失败 Level 2：`to_workflow_run_view` 将 StepRun 错误映射为 step 级呈现（symbol + state + error）；Level 3 诊断面保持隐藏式 |
| Workflow Surface ✅ | Slint Runtime Surface（step 列表 + symbol/状态 + 进度 + Paused 呈现），`WorkflowHost` 呈现模型 + `show/hide_workflow_run`；Enter=Confirm / Esc=Keep paused（不取消已提交 Effect）；app 内触发集成待 workflow 触发功能落地（callbacks 已接日志） |

UI-ACC 状态变化：039 ◐→✅（selected ▸ 标记）；024~030/042/050 ⏳→◐（surface + 呈现模型已实现，应用内触发待接）；038 ◐（Level 2 ✅ / Level 3 隐藏式设计）。

---

## Workflow 触发集成闭环（review 32，2026-09-05）

**最小触发链已闭合**：托盘菜单「运行示例工作流」（触发源）→ `workflow_service::start_workflow`（服务层创建 WorkflowRun，UI 不创建）→ 后台 runner 执行 → Runtime Surface 实时 step 进度 → Paused(ConfirmationRequired) 等待通道 → Enter 确认 → re-resolve → 完成。

- 触发源模型：今日 = 托盘菜单；未来 hotkey/plugin/AI/MCP/schedule 均为新的 trigger source，经同一 `start_workflow` 入口（通用 Trigger Framework 按评审 32 明确推迟）。
- 演示定义 `demo_definition()`：三步 Inline copy，s2 带 `confirmation: "confirm"`，完整演练 Normal → Paused → Confirm → Complete。
- Confirm 通道：`workflow-confirm` UI 事件 → `confirm_active_run()` → mpsc → runner.resume（re-resolve 后执行）；超时/ dismissed 保持 Paused（Esc 不取消已提交 Effect）。
- UI-ACC 状态更新：019/020 → ✅（服务级 pause/resume 全链路）；042 → ✅（surface 语义呈现）；024~030 → ✅（surface 实现 + 单元级呈现锚点；GUI 手工抽查按 MVP3.1 惯例发布前执行）。

---

## ⑦ Diagnostics + ⑧ Visual Regression（review 32 收尾执行，2026-09-05）

| 项 | 实现 |
|---|---|
| ⑤ Confirmation 呈现 | ActionPanel 新增 ConfirmationPending 子状态条：⚠ 符号 + "Confirmation required" + `Enter Confirm / Esc Cancel`（Spec §18.1），由 `confirmation-pending`（Rust 按 selected command 判定）驱动 |
| ⑦ Level 3 Diagnostics | `WorkflowItem` 增加 `execution-id / failure-class`；WorkflowRuntime 支持 `show-diagnostics`；Ctrl+D（surface 打开时）切换 `⚙ step · execution-id · failure-class` 行；Esc 关闭时自动复位 |
| ⑦ 失败分类（Level 3 文案） | `classify_failure` 关键字映射：timeout→Timeout / unavailable·spawn→PluginUnavailable / denied→CapabilityDenied / protocol·malformed·crashed→ProtocolViolation / 其余→EffectFailed |
| ⑧ Visual Regression | `snapshot.rs`：`LAUNCHER_SNAPSHOT=<path.bmp>` 触发 → 确定性 demo 数据（3 结果 + 3 actions 含 disabled + 3 workflow steps 含 Paused）→ GDI BitBlt 捕获 client 区 → BMP 落盘 → 退出。已实测：写入 2,419,254 字节（高 DPI 物理像素尺寸自动记录于 BMP 头） |

**UI-ACC 状态更新**：038 ✅（Level 2/3 可检视）；042 ✅（Workflow 语义呈现）；019/020 ✅（服务级 pause/resume 全链路 + 确认暂停呈现）。

**Spec 实现顺序 ①–⑧ 全部完成。** 剩余非契约项：Level 3 诊断的独立 DiagnosticsDetails 面板（当前为 step 行内展开）、monoline 图标资产管线（P1）、Workflow 触发源的扩展（hotkey/AI/MCP）。

---

## ⑧ Visual Regression 十状态基线（review 39-mvp7-0.3 执行，2026-09-05）

`LAUNCHER_SNAPSHOT_DIR` 一次运行产出 VR-001~VR-010 全部 10 张 BMP（640×420 logical / Dark / 100%）：

VR-001 Main-Empty、VR-002 Main-Results、VR-003 Main-Selected、VR-004 Main-Error、VR-005 Action-Normal、VR-006 Action-Disabled、VR-007 Action-Confirmation、VR-008 Workflow-Running、VR-009 Workflow-Paused(ConfirmationRequired)、VR-010 Workflow-Failed。已实测 10/10 落盘。

回归闸门 = 行为（154 tests）+ 视觉（10 张基线逐字节 diff）。至此 VISUAL-DESIGN-SPEC 实现顺序 ①–⑧ 全部完成。
