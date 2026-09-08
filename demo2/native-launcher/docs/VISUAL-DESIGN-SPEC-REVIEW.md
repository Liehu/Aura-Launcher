# VISUAL-DESIGN-SPEC-v0.1 — 设计评审意见

**Date:** 2026-09-05
**Reviewed:** `demo2/files2/VISUAL-DESIGN-SPEC-v0.1.md`（52 节 + UI-ACC 式验收清单）
**对照:** 当前实现（app.slint / launcher-ui / main.rs）、UI-CONTRACT-v0.1、性能定位（软件渲染器 / 低内存）

---

## 评审结论：APPROVE WITH REVISIONS

**视觉方向、Token 体系、冻结边界全部正确，且与既有架构零冲突**——特别是三点值得直接确认：① Workflow 符号词汇（✓/●/○/⚠/⏸/⊘）与已实现的 `to_workflow_run_view` 完全一致；② 语义化 Token 命名（`bg.surface` 而非 `darkGray1`）为未来 Light/High-Contrast 主题预留了正确结构；③ 结尾的工程决策（**先建 theme.slint + primitives，再增量替换现有 UI，不重写 app.slint**）是唯一能保住 Fully Conformant 行为基线的做法，强烈支持。

以下 6 个修订点：R1/R2 是与现有实现的真实冲突，R3/R4 是软件渲染器可行性风险，R5/R6 是缺口。修订后维持 FROZEN v0.1。

---

## R1（必须修）accent Token 与用户配置 `theme_color` 双源冲突

Spec 冻结 `accent.primary = #4DA3FF`，但现有实现存在用户可配置的 `theme_color`（config.toml，默认 `#5B8DEF`，经 `parse_theme_color` 注入 accent）——现在有**两个 accent 真相源**。

**修订（冻结进 Spec §2.4）：**
```text
accent.primary 默认值 = #4DA3FF（Dark theme）
用户配置 theme_color 覆盖 accent.primary / accent.primary_hover
（覆盖关系：config > theme default；theme.slint 是唯一注入点）
```
不建议删除 `theme_color` 配置（既有用户能力），但必须声明覆盖链，禁止组件绕过 Theme 直接读取。

## R2（必须修）窗口默认尺寸与现实现不一致

Spec §7：600×420；当前实现：**640×420**。二选一并写明迁移：建议采纳 **640×420** 为冻结值（避免无意义回归），Spec 同步改；min/max（480–320 / 960–680）保留。

**✅ 已裁决并被取代（2026-09-05，Window Size System v0.2）**：R2 的"冻结固定尺寸"方案作废，改为**内容驱动高度**。冻结系统如下（实现：`theme.slint` 窗口令牌 + `app.slint` 高度公式 + `result-list.slint` 可见行/Flickable）：

```text
Window Size System
──────────────────

Width:
    default 620
    min 480
    max 960

Height:
    content-driven（禁止固定高度）

Visible rows:
    default 6
    min 4
    max 8

Density:
    Normal

Result row:
    44

Action row:
    40

Workflow step:
    36

Search:
    36

Context:
    28

Status:
    28
```

高度公式（Theme 令牌，单一来源）：

```text
height = padding-window(8) + search(36) + gap(4)
       + visible_rows × result_row(44) + gap(4)
       + context(28) + status(28)
       + padding-window(8)
       + ActionPanel.content-height + WorkflowRuntime.content-height
```

规则：
- `visible_rows` = clamp(results.count, 4, 8)；面板/工作流表面打开时收敛到 **min 4**，由表面获得空间（overlay 不再遮挡结果行）。
- 超出可见行数的结果在 ResultList 内部 Flickable 滚动，viewport 跟随键盘选中行。
- 宽度 min/max 为未来 resize 预留令牌（`win-width-min/max`），v0.2 不实现拖拽 resize（no-frame 窗口）。
- 组件高度一律取 Theme 令牌（row-main/row-action/row-workflow-step/row-search/row-context/row-status），禁止散落字面量（Spec Rule 1）。
- `ActionPanel.content-height` / `WorkflowRuntime.content-height` 为组件对外暴露的确定性高度，窗口高度公式直接读取；面板/工作流改为常驻实例化 + `visible` 标志（高度可读、无 if 卸载）。

**迁移影响**：窗口不再 640×420 固定；默认宽 620，高度随结果数量 4~8 行 + 底部表面浮动。原 R2 的"640 冻结"与 Spec §7 "600×420"、min-height（480×320 / 960×680）一并作废。

**Progressive Disclosure（v0.2 核心交互原则，2026-09-05 实现）**：

> **Launcher 是渐进展开的瞬时界面：Ctrl+Space 初始只显示 Search Box；Result / Context / Action / Confirmation / Workflow 按当前交互阶段按需呈现。**

- 新增视觉状态 `Main.Initial`（仍属 Main，不新增顶层 Mode）：空查询 = 仅 Search Box，**无结果区、无空态文案、无 footer**（"No results" 只在用户输入过之后才有意义）。
- 可见性矩阵：Search = Main 常驻（Action/Workflow 模式由表面替换）；Results = 有查询才出现；Context = 有查询且有结果时作为辅助信息（面板/工作流打开即隐藏）；Status = 有消息才占位（不再常驻 28px）。
- 三模式互斥替换主内容（不再叠加）：Main（Search+Results）/ Action（Command header + actions，Search 隐藏）/ Workflow（Workflow surface 独占）；Confirmation 仍是 Action 内的 transient 条带。
- 离散高度状态（无连续动画）：`H0 Initial = padding×2 + 36`；`H1 Main+query = … + rows×44`；`H2 +context/status`；`H3 Action = padding×2 + panel.content-height`；`H5 Workflow = padding×2 + wf.content-height`；任意状态有消息时 +28。
- 焦点跟随模式：进 Action/Workflow → 根 FocusScope 持焦（capture 键位继续生效）；Esc 关表面 → 焦点回搜索框（`focus-keys()` / `focus-input()`）。
- 实现：`app.slint` 的 `in-main/has-query/show-results` 状态属性 + `mode-height` 公式；`SearchBox.text` 对外暴露；`ResultList.show-empty` 区分 Initial 与空态。

**WIN-TRAY v0.1（窗口生命周期与任务栏抑制，2026-09-05 实现，review 50-mvp6-0.6）**：

- **Launcher popup = Tool Window**：`WS_EX_TOOLWINDOW`（`apps/launcher-app/src/win_platform.rs`，组件创建后 + 每次显示时幂等应用）。Windows 对 Tool Window 不创建任务栏按钮；托盘图标（SystemTrayIcon）是常驻身份。**禁止** `WS_EX_NOACTIVATE`（会破坏键盘交互）；AppUserModelID 不用于任务栏抑制（它管 shell identity/grouping）。
- **窗口状态模型冻结**：`LauncherWindowState = Resident(Hidden | Visible(Main | Action | Workflow))`；进程永不退出，Ctrl+Space = Hidden ↔ Visible(Main)。
- **WIN-TRAY 验收门**（001~006，见 `win_platform.rs` 注释与 MVP3.1-ACCEPTANCE）：001 常驻+隐藏+托盘在+任务栏无按钮；002 任意模式可见时任务栏仍无按钮；003 Esc 后进程常驻；004 退出清理托盘；005 Action/Workflow 模式任务栏仍无按钮；006 explorer.exe 重启后托盘重新注册（P1 人工验证：确认 Slint 托盘是否自动处理 `TaskbarCreated` 广播，若无则需自行重加图标）。
- **验证**：程序化断言可见窗口 `exstyle & WS_EX_TOOLWINDOW`（2026-09-05 通过，`before=262424 after=262552`）。

**分辨率自适应缩放（同日追加）**：所有尺寸/间距/字号令牌乘以 `Theme.ui-scale`（Rust 端 `apply_ui_scale` 推导：主屏**逻辑**高度 / 1080 基线，量化 0.25 步进，clamp [1.0, 2.0]；除以 Slint DPI scale 避免与 Windows 缩放双重放大）。1080p@100% = 1.0；1440p ≈ 1.25；4K@150% ≈ 1.25（物理约 1.9×）。每次 popup 显示时重算（首次 show 前后 winit DPI 才可信）。字号字面量已全部令牌化（24 处 → Theme.type-*），禁止回退。

## R3（可行性风险）阴影在软件渲染器下的成本

§6 Level 3 阴影（blur 32px / 45%）与 §46"禁止 continuous shadows"存在张力。Slint `drop-shadow` 在软件渲染器下逐像素计算，Launcher 每帧重绘时可能触发性能预算（§19.6 禁止 blur-heavy）。

**修订：** v0.1 阴影降级为"**1px border.default + 表面色差**表达 elevation"（Level 2 已足够：#16202C on #111822 对比可辨）；真阴影标记为 P1，且实现前必须过 `action_panel_us`/帧耗时基准。Confirmation emphasis 用 warning 边框 + 表面色差表达，不用阴影。

## R4（可行性风险）窗口级 Open 动画（fade + scale 0.97→1.0）

窗口 opacity/scale 动画在 winit + 软件渲染路径下支持不确定，且 popup 打开在 T1 性能路径上（P95 ≤ 10ms）。

**修订：** v0.1 窗口打开 = **即时显示**（这与 popup 延迟目标一致）；Motion 条款中的 fade/scale 降级为 P1 可选项，仅元素级动画（面板 80ms、状态 80–120ms）进入 v0.1。

## R5（可行性风险）8px window radius

无框窗口圆角需要透明窗口背景，软件渲染器 + always-on-top 下可能出现边缘伪影与额外合成成本。

**修订：** v0.1 窗口保持直角（或至多 4px，需视觉验证）；`radius.large = 8px` 保留给未来 Hardware renderer / Acrylic 路线。Radius token 语义不变。

## R6（缺口）图标资产来源未定义

§8 定义了 19 个 Core Icons 的风格（monoline 1.5px 16–20px），但未定义**资产来源与渲染方式**。Slint 消费图标只有两条路：@image-url 位图/SVG，或文本 glyph。当前实现中 ResultItem.icon 为空字符串占位。

**修订（v0.1 最小方案）：** 第一版不引入图标资产管线——Result Row 的 icon 列以 **符号 glyph / 首字母**（与 Workflow symbol 同源）占位；monoline 图标资产（来源：自绘 SVG 或引入 Fluent UI System Icons 子集）列为 P1，进入前需补充资产构建说明。否则 §8 是无法验收的空头条款。

---

## 确认项（冻结，无修订）

Token 语义化命名 + Light/HC 前向结构（§41/42）；Typography scale 与行高（§3）；4px spacing 体系与行高表（§4.2，与现实现 44px/36px 吻合）；Workflow 符号+accent 表（§19.2，与实现一致）；Shortcut compact token 右对齐（§12）；三级失败呈现与 Level 3 默认隐藏（§21/24）；AI "✨ Suggested" 非授权语义（§22）；Context Hint 低对比 + 打开面板/Workflow 时隐藏（§27，与实现一致）；状态行 `[Context][Status]` 合并方向（§28）；Motion ≤200ms + reduced motion（§25/45）；Forbidden drift 清单（§47）；50 条视觉验收清单（§50）；与 UI-CONTRACT 的 What/How 分层（§53）。

## 微小不一致（合并时顺手处理）

| # | 项 | 修订 |
|---|---|---|
| m1 | §4.2 Context Hint 28px / Status 28px 与 §28 合并状态行 `[Context][Status]` 表述并存 | 定稿：**单行 28px，左侧 Context、右侧 Status**；Error 时该行切换 error 样式（避免两行叠加占空间） |
| m2 | §17.1 示例 `← Calculator Plus` 的返回箭头 | Action mode 由 Esc 返回，左上角箭头装饰可省略；标题保留即可 |
| m3 | `bg.selected_hover`（§2.1）与 §10"Hover 弱于 Selection"规则 | 明确 hover 只作用于未选中行 |
| m4 | §3.2 `type.title` 16px Semibold vs 现 workflow 标题 15px | 实现迁移到 16px，纳入 P0-UI 收尾 |

## 与 UI-CONTRACT 的关系确认

Spec 严格遵守"视觉不改行为"（§53）：所有状态呈现（is_primary/disabled reason/⏸ Paused/▸ marker/✨ Suggested）都能映射到已实现的 Presentation 字段，未发现需要新增行为或修改 UI-CONTRACT 的条目。**唯一行为级注意点**：m1 的 Error 样式切换不得引入新的用户操作。

---

## 冻结路径（修订吸收后）

1. Spec 补 R1–R6 + m1–m4 修订 → 维持 FROZEN v0.1（视觉规范无需 ADR，Token 登记按 Rule 7）。
2. 实现按 Spec 尾部顺序：`theme.slint`（Token 全量）→ shared primitives（focus/selected/symbol）→ Main → Action → Confirmation → Workflow Runtime → 诊断 → 视觉回归（§51 基线 + §50 清单）。
3. 每步保持行为基线不回归（154 测试 + UI-ACC 状态标注不回退）。

---

## 实现记录（2026-09-05，确认后第一步：Theme / Tokens）

- `crates/launcher-ui/ui/theme.slint` 落地：Surface/Border/Text/Accent-Semantic 全量 Token + Radius/Spacing/行高/字号（Spec §2–§5）。
- `app.slint` 全部散落 hex literal（20 处）替换为 Theme token（Spec Rule 1 合规）。
- **R1 落地**：`accent.primary` Dark 默认 = `#4DA3FF`；config `theme_color` 保持覆盖链（config > theme default），launcher-config 默认值同步更新为 `#4DA3FF`。
- 阴影/窗口动画/圆角按 R3/R4/R5 维持 v0.1 降级（无边框表面色差、即时显示、直角）。
- 行为基线未回归：154 tests 全绿、零警告。
