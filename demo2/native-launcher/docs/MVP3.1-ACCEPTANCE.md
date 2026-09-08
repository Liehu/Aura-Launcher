# MVP3.1 — Native Action UI 验收清单（ADR-0011 域契约之上）

**Date:** 2026-09-04　**Scope:** review 18-mvp3.1-0.1 P0

## 已实现

| 项 | 实现 |
|---|---|
| ActionPresentation model | `launcher_ui::{ActionPresentation, to_action_presentations}`（UI 只见 id/title/enabled/reason，INV-036） |
| Action Panel Slint 组件 | 同窗口双模式（main ↔ action），底部 overlay；无第二窗口/复杂层级（review §17） |
| Primary/Secondary 呈现 | 首行 "Enter" 徽标 = primary（first Ready）；secondary 依声明顺序 |
| 键盘交互 | Ctrl+↓ 开面板；面板内 ↑/↓ 导航（Rust 侧跳过 disabled 行）；Enter 执行选中；Esc 关面板（再按才关启动器） |
| Disabled 呈现 | 可见、置灰、带 reason（如 `requires clipboard.write`）、不可选不可执行（engine `validate` 拒绝，唯一执行闸门） |
| Hidden 语义 | unknown/invalid action 在 host 层已丢弃，永不进入 Command（INV-038） |
| id 选择 | `execute-action(command_id, action_id)` 按 id 定位，禁止位置索引（INV-035） |
| 执行桥 | ActionPanel → `launcher_action::execute`（Effect 唯一路径，INV-037/004） |
| 结果反馈 | Success → 关闭 popup；失败 → 状态行 `⚠ {error}`（Esc 不取消已提交的 effect，review §10） |
| Informational command | `actions=[]` → 面板不开启、Enter 无效果不报错 |

## 验收清单（review 18 §23）

- [x] 多 action Command 可打开 Action Panel
- [x] 第一个 Ready action 为 primary
- [x] Enter 执行 primary（`primary_action()` = first non-disabled）
- [x] Secondary 可选择并执行
- [x] Disabled 与 Hidden 可区分（disabled 可见置灰带 reason；hidden 不存在）
- [x] Unknown secondary 被隐藏
- [x] 非法 secondary 不杀死 Command（CAT-007）
- [x] Capability-denied action 到不了 Effect（engine 拒绝，`disabled_actions_are_never_executable`）
- [x] Unknown ActionType 永不到达 ActionEngine（CAT-008）
- [x] UI 按稳定 command/action id 选择
- [x] 所有 Effect 仍经 ActionEngine
- [x] calculator-plus 验收通过（mvp3_acceptance + domain_contract_kit CAT-001~011）
- [x] Rust/Python Command 模型保持兼容（Action 新字段全部 serde default，legacy 字符串路径不变）
- [x] 16 项 Plugin Contract 测试仍通过
- [x] Workspace 全部测试通过（117）
- [x] 性能回归门禁 ✅（`launcher-bench` `action_panel_us`，10k iterations，50 actions 半 disabled，ADR-0012）：
  Panel Open P95 = 4µs（gate ≤ 10ms）、Selection Nav ≈ 0µs（gate ≤ 5ms）、Execute Bridge P95 = 1µs
- [x] 无新增 WebView/Electron/CEF 依赖

## 双工作线合并验证（2026-09-05）

`main.rs` 曾被两条工作线并行修改（WIN-TRAY/scale/Progressive Disclosure ← 本线；焦点归还 restore_foreground + prev_foreground、Confirmation 双击确认、Paste 带 Hwnd payload、Workflow/MCP ← MVP3.2/4 线）。合并后树自洽：**229 + 16 测试全绿、零警告**，并经真实 GUI 重验完整链路：

- 单实例（曾出现双实例争抢热键：`RegisterHotKey failed`——多实例互斥是既有预期行为）
- Ctrl+Space → Initial 搜索框 → `20*2` → `= 40` 首位（score hint）→ ↓ → Ctrl+↓ → 面板 → Enter
- `clipboard.copied chars=2`，**Get-Clipboard = `40`** ✅
- `foreground.restore set_foreground=true`：焦点归还之前的前台应用 ✅（MVP3.1 焦点归还与 park 机制共存）
- 任务栏无启动器按钮（WS_EX_TOOLWINDOW + 清除 WS_EX_APPWINDOW）、托盘 feature 已启用（Slint Windows 托盘含 TaskbarCreated 重注册，WIN-TRAY-006 自动满足）

## WIN-TRAY v0.1 验收（review 50-mvp6-0.6，2026-09-05）

- [x] WIN-TRAY-001/002：popup 为 Tool Window（`WS_EX_TOOLWINDOW` 已程序化断言），显示时不产生任务栏按钮，托盘图标常驻
- [x] WIN-TRAY-003：Esc 后窗口 park、进程常驻、托盘仍在
- [x] WIN-TRAY-005：Action/Workflow 模式下任务栏按钮同样不存在
- [ ] WIN-TRAY-004/006：退出清理与 explorer.exe 重启重注册（P1 人工验证；若 Slint 托盘不处理 `TaskbarCreated` 广播，需在宿主层自行重加图标）
- 实现：`apps/launcher-app/src/win_platform.rs`（文档 50 §5 的分层建议以 app 层最小模块落地；Core 不接触任何 Win32 概念）

## 真实桌面 E2E 手工脚本（review 19；发布前由人执行一次）

准备：`cargo run -p launcher-app --release`；安装 calculator-plus 插件（`target/release/plugin-calculator-plus.exe` + `apps/calculator-plus/plugin.json` 放入 `%LOCALAPPDATA%\native-launcher\plugins\calculator-plus\`）。

1. **Primary**：前台记事本 → Ctrl+Space → 输入 `12+34*2` → **↓ 选中 `= 80` 行**（注意：已安装的 Echo 插件结果 `Echo: 12+34*2` 因精确词法匹配会排在 plugin hint 之上；选错行时面板只会显示 Echo 的 `echo` action）→ Enter → 关闭启动器 → 记事本 Ctrl+V 粘贴为 `80`（clipboard 写入成功）。
2. **Secondary**：再次唤起 → `12+34*2` → ↓ 选中 `= 80` → Ctrl+↓ 打开面板（应显示 Copy/Insert/Open History；若显示 `echo` 说明选中的是 Echo 行）→ ↓ 选 Insert → Enter → 粘贴为 `(80)`。
3. **Capability denied**：用未声明 `clipboard.write` 的 manifest 启动插件 → 面板中 Copy/Insert 置灰带 `requires clipboard.write`、不可选中、Enter 无效果（引擎拒绝）；History 仍可执行。
4. **Memory 采样**：三种状态各记录 Private（任务管理器/`launcher-bench soak`）：popup hidden / popup visible / action panel visible，对比 idle 基线 12.6MB 无突增。

## Progressive Disclosure / 布局修复验证记录（2026-09-05，真实桌面 GUI 自动化验证 ✅）

`43-mvp6-0.5` Progressive Disclosure 落地后，经计算机控制自动化完成全链路验证（截图 + 剪贴板断言）：

| 步骤 | 结果 |
|---|---|
| Ctrl+Space Initial：仅通栏搜索框（~76px 高小窗） | ✅ |
| 输入 `20*2`：结果区出现，score hint 使 `= 40` 排名第一（超过 echo 的词法匹配） | ✅ |
| ↓ 选中 `= 40`，底部 Context 条按需出现（`Ctrl+↓ Actions`） | ✅ |
| Ctrl+↓ 打开 Action Panel：替换式展开（搜索框隐藏、命令头 + 3 actions + 快捷键徽标） | ✅ |
| Enter 执行选中 Copy：`clipboard.copied chars=2`，Get-Clipboard = `40`，窗口 park 关闭 | ✅ |

**布局/事件修复过程中的根因记录**（均已修复）：
1. **小搜索框**：Slint 普通容器的子元素默认按首选尺寸**居中**——`keys` FocusScope、VerticalLayout、SearchBox 内的 LineEdit 三层都需显式 `width: parent.width`（LineEdit 还需 `y` 垂直居中）。
2. **capture 阶段不触发**：`capture-key-pressed` 在本 Slint 版本 + winit 路径下不会下发（Esc/↓ 全灭）。最终架构：**主模式按键在 SearchBox 内部 LineEdit 的 `key-pressed` 回调拦截**（TextInput 先调用开发者回调再做光标处理，源码级保证），**Action/Workflow 模式按键在根 FocusScope 的 `key-pressed`（bubble）处理**（这些模式隐藏搜索框、焦点经 `focus-keys()` 移到根 scope）。
3. **面板模式 Enter 缺失**：搜索框隐藏后 `accepted()` 不再触发，keys handler 补 Return 分支（按 id 执行选中 enabled action）。

## E2E 进展记录

- **Step 1 (Primary) ✅（2026-09-04 真实桌面验证）**：记事本前台 Ctrl+Space → 输入 `20*20` → `= 400` 列表首位 → Enter → 启动器关闭 → 粘贴为 `400`，日志出现 `clipboard.copied`。
- **修复过程发现的两个真实缺陷**（均已修复并有测试）：
  1. **Action Engine 的 Copy 从未写真实剪贴板**（`Effect::Copied` 只是标记）→ `launcher-action` 实现 Win32 `CF_UNICODETEXT` 写入（带剪贴板锁重试），失败经状态行呈现；
  2. **排名器丢弃无词法关联的插件结果**（`rank()` 的 `retain(score > 0)` 使 `= 400` 对查询 `20*20` 恒为 0 分被丢弃）→ 落地契约 §19 的 bounded score hint：wire 层 `PluginResultItem.score` ∈ [0,1]（归一化）→ host 透传 → 排名按 `hint × 45` 计入（< 精确词法匹配 100，插件无法劫持排名），hint ≠ 0 的结果不被词法零分过滤器丢弃。新增测试 `hinted_plugin_result_survives_rank_without_lexical_match` / `hint_is_bounded_and_cannot_hijack_exact_matches`。
- **附带修复**：spawn 失败的插件（如过期 echo 二进制）按 `idle_timeout`（≥5s）冷却，不再每按键重试 spawn（每次 ~145ms）。
- Step 2/3（Secondary 面板 / Capability denied 呈现）待复测；Step 4（内存采样）发布前执行。

## 窗口生命周期（重要实现注记，ADR-0012 Addendum）

"隐藏" = `park()`（移屏外 + 释放焦点），**不调用 Slint `hide()`**——hide/show 循环会使 winit 窗口进入 iconic + 死渲染面状态（OS 报 visible 但内容空白）。"显示" = `show()`（已映射窗口为 no-op）+ recenter + repaint。已知 trade-off：parked 窗口仍在 Alt-Tab 列表中。

## 交互缺陷修复验证记录（2026-09-04，真实桌面复测通过）

| 缺陷 | 根因 | 修复 | 验证 |
|---|---|---|---|
| 第二次 Ctrl+Space 后窗口不显示（任务栏有图标） | Slint `hide()`（winit）使窗口进入 iconic + 死渲染面；`SW_SHOW`/`SW_RESTORE`/置顶/强制重绘均不可靠 | **窗口常驻映射，永不 `hide()`**：`park()` 移屏外 + `SetFocus(None)`；显示 = recenter + repaint（ADR-0012 Addendum） | ✅ 用户复测多次均正常 |
| 输入框有字符时 Esc 无法关闭启动器 | Slint 按键沿父链冒泡，原 Esc 处理 FocusScope 是输入框的**兄弟节点**，不在冒泡链上；Esc 在编辑器内部被消费 | `keys` FocusScope 改为**包住整个布局**（成为输入框祖先），Esc 经冒泡必达 | ✅ |
| ↑/↓ 作用于输入框光标而非列表 | ↑/↓ 被 TextInput 当作 TextShortcut::Move 消费，永不冒泡 | 改用 FocusScope 的 **capture 阶段**（`capture-key-pressed`，在按键到达焦点元素之前从窗口向下拦截）：↑/↓/Ctrl+↓ 拦截，←/→ 及文字输入 reject 放行 | ✅ |
| Ctrl+Space 唤起后键盘焦点不在输入框 | park 释放 Win32 焦点后，re-show 只恢复了窗口级焦点，Slint 内部焦点元素未回到输入框 | app.slint 新增 `public function focus-input() { input.focus(); }`，两条唤起路径（热键/托盘）显示后立即调用 | ✅ |

**键位分发最终架构**：`capture-key-pressed`（祖先 FocusScope）拦截 Esc/↑/↓/Ctrl+↓ → 其余按键 reject 放行给焦点元素（输入框文字编辑不受影响）。

## 真实桌面 E2E 结果（2026-09-04，手工执行）

| 路径 | 结果 |
|---|---|
| Primary（Enter → Copy → 剪贴板） | ✅ |
| Secondary（Ctrl+↓ 面板 → Insert → 剪贴板 `(6)`） | ✅ Effect 成功；**不会自动粘贴到原输入点**——MVP3.1 的 Insert 定义为 clipboard-based insert preparation（review 18 §14），真实文本注入（SendInput 模拟 Ctrl+V，新 effect 类型 `system.paste`）留给 MVP3.2 |
| Capability denied（置灰行不可选/不可执行） | ✅ |
| 焦点回归（执行后焦点回到原前台窗口） | ✅（`foreground::restore_foreground`，所有关闭路径统一） |
| 面板标题与选中行一致 | ✅（修复：`on_selection_changed` 未写回 `AppState.selected`，面板恒读第 0 行——INV-035 状态错位的变体，真实桌面测试发现） |

已知限制（有意不做）：
- Insert 不自动粘贴——需 `system.paste` effect（SendInput 注入），MVP3.2 候选；
- Echo 等 legacy 插件的精确词法匹配会排在 plugin hint 之上——契约冻结的排序规则，操作时需先选中目标行。

## 键位（MVP3.1 冻结一个方案）

| 键 | 主模式 | 面板模式 |
|---|---|---|
| Enter | 执行 primary | 执行选中 action |
| ↑/↓ | 移动命令选择 | 移动 action 选择（跳过 disabled） |
| Ctrl+↓ | 打开 Action Panel | — |
| Esc | 关闭启动器 | 关闭面板 |
