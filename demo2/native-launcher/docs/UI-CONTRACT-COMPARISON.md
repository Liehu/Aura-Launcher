# UI-CONTRACT 两版本对比评审（GPT 版 vs Native-Launcher 版）

**Date:** 2026-09-05
**A = `demo2/files2/UI-CONTRACT-V0.1-GPT.md`**（外部评审产出，50 条 UI-ACC 验收）
**B = `demo2/native-launcher/docs/UI-CONTRACT-v0.1.md`**（当前仓库冻结版，UC-001~010，与已实现 Slint UI 对齐）
> **状态：合并已执行（2026-09-05）**——四步清单全部完成；D1 采纳建议（单击 = 选中并执行）。

**结论先行：两者核心语义 95% 一致，无架构分歧；差异集中在粒度与 2 个真实行为分歧。建议合并为一份 canonical 契约（保留 B 路径 + 吸收 A 的 7 项增量），解决"双 FROZEN"版本冲突。**

---

## 一、共同点（核心语义完全一致，可直接确认）

| 语义 | 两版一致 |
|---|---|
| 3 顶层 Mode：Main / Action / Workflow；Confirmation = Action 子状态 | ✅ |
| AI / Plugin / Context / MCP 不是 Mode，是内容来源 | ✅ |
| 瞬时工作空间定位 + 信息层级 ≤3 + 禁 Dashboard 化 | ✅ |
| UI 职责六词（Present/Select/Navigate/Invoke/Confirm/Observe） | ✅ |
| Presentation Model 层：UI 只消费投影，禁止解析 ActionKind/capability | ✅ |
| `is_primary` 由 host（`primary_action()` = first Ready）计算，UI 禁止 `actions[0]` 推导 | ✅ |
| Stable ID（command/action/step/run）；禁 index/title/position | ✅ |
| 失败呈现消费分类结果（8 类映射），三级呈现（Inline/Detail/Diagnostic） | ✅ |
| Accessibility 非颜色通道强制；disabled 必带 reason | ✅ |
| Confirmation 两阶段 + confirmed 永不持久化 + Esc=Cancel | ✅ |
| Workflow = Runtime Surface 非 Builder；Esc 不取消已提交 Effect | ✅ |
| AI 提案走普通 Action UX + "✨ Suggested" 仅来源标记 | ✅ |
| Empty state 不推 AI；搜索框不做 "Ask AI" | ✅ |
| 无第二窗口 / 无 WebView/Electron/CEF | ✅ |
| Keyboard/Mouse 同一 stable ID 执行路径 | ✅ |
| StaleContext 不作为默认用户错误文案；v0.1 不自主刷新 selection | ✅（A §10.3 与 B §10 措辞一致） |

## 二、A 有而 B 无（建议全部吸收进 canonical）

| # | A 的增量 | 价值 | 备注 |
|---|---|---|---|
| A1 | **Main 细分状态**：Idle/Searching/Results/Empty/Error | ✓ | B 只有 Normal/NoResults；Presentation 层应能表达全部 |
| A2 | **Workflow.Cancelled** UI 状态 | ✓ | domain 已有 Cancelled，B 漏了 UI 呈现 |
| A3 | **焦点保持规则**（§5.6）：Presentation 更新不得无条件重置焦点；§5.5 Workflow Running 不得抢焦点 | ✓✓ | 防止"更新导致焦点跳走"类回归，B 缺失 |
| A4 | **execution_id 列入 Stable IDs**（§17.5，retry 产生新 id） | ✓ | B 的 ID 清单漏了它 |
| A5 | **Enter → Effect P95 ≤ 50ms** 新 SLO + §19.5 五状态内存记录清单 | ✓ | 沿用既有性能体系，补 UI 侧指标 |
| A6 | **50 条细粒度验收**（UI-ACC-001~050，按 Main/Action/Shortcut/Confirmation/Context/Workflow/AI/Runtime/Accessibility/Architecture 分组） | ✓✓ | 比 B 的 UC-001~010 更适合 Agent Coding 逐条验收；每条天然映射到实现测试 |
| A7 | §1.1 "本契约不定义" 显式清单（Resolver/Capability/Effect/调度/AI 算法/RPC/MCP 协议） | ✓ | 边界更硬 |

## 三、B 有而 A 无（保留进 canonical）

| # | B 的独有 | 说明 |
|---|---|---|
| B1 | **禁止迁移清单**（Main 直达 Executing / ConfirmationPending 跳过 engine / 携带旧 ResolvedAction 执行） | A 只有正向迁移表，无显式禁止集 |
| B2 | **Workflow surface 位置语义**：长工作流 launcher 关闭后台跑、ConfirmationRequired 时重开；Serializable≠Resumable → UI 不承诺崩溃恢复 | A §13.8 只提 crash recovery 一半 |
| B3 | **Confirmation 呈现位置**：status line + Action 子状态的具体实现绑定 | A 只定义了抽象状态 |
| B4 | **与现有实现的对应状态**：Main/Action 已实现 ✅、Workflow ⏳（P0-UI-007） | A 无实现状态标注 |
| B5 | **INV-062~065 链接**（UI 责任类 invariant 的仓库编号） | A 无仓库 invariant 引用 |

## 四、真实行为分歧（需要决策，二选一）

**D1. Result Row 单击行为**

- A §7.1：`Click → select command`（仅选择；Double Click 未要求）
- B（当前实现）：TouchArea click → **选中并立即执行 primary**（launcher 惯例，与 Raycast/PowerToys Run 一致）

**建议：保留当前实现**（单击执行），在 canonical 契约 §7.1 明确写入"Click = select + execute primary"作为 Windows launcher 惯例修正 A 的表述。A 的"仅选择"更像文件管理器范式，会多一次点击、违背瞬时定位。

**D2. 双 FROZEN 版本冲突**

两份文件都自称 FROZEN v0.1。协议只能有一个 canonical。

**建议：** 合并后以仓库 `docs/UI-CONTRACT-v0.1.md` 为唯一 canonical（吸收 A1–A7 + D1 决议 + 保留 B1–B5），外部 GPT 版标注为 "superseded by repo canonical"。合并属 additive/clarification，不需重新评审，但按惯例补一条 ADR-0017 Addendum 记录合并决议。

## 五、其他微小差异（记录即可）

- A §3.4 Runtime State 独立成节（Ready/Disabled/Executing/...）——与 B §4 StatusLine 等价，合并时保留 A 的细分。
- A §8.1 引入 Searching 中间态与"query generation"——与 INV-012（query supersession）呼应，保留。
- A §6.2 明确 disabled 不可成为 focus target——B 有"跳过 disabled"实现但未写成焦点规则，吸收。
- B §13 Workflow surface 三级失败呈现（Inline/Detail/Diagnostic）——A §15.2 也有，一致。
- A §5.4 Confirmation 焦点规则（Confirm/Cancel 两出口、禁自动确认）——B 隐含，吸收成显式条款。

## 六、合并执行清单（决议后一次完成）

1. B 吸收 A1–A7、D1 决议、四 §微小项 → canonical 更新（仍为 v0.1，additive）。
2. UI-ACC-001~050 作为 canonical §20 验收集替换 UC-001~010（保留 UC 编号映射注释）；对照当前实现标记 ✅/⏳（预计 ~40 项已满足，Workflow 相关 ~8 项待 P0-UI-007）。
3. GPT 文件头部加 "SUPERSEDED — see docs/UI-CONTRACT-v0.1.md"。
4. ADR-0017 Addendum：合并决议 + D1 决议 + acceptance 迁移。
