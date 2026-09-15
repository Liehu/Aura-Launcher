# ADR-0021: Deferred Tray Capabilities 与左键语义（P3-E/H 决策记录）

**状态：ACCEPTED（决策记录 + Follow-up 登记）**

## 决策 1 — Tray 的 Reindex / Pause Hotkey / Check Updates：先补 Core 公共能力，再接 UI

P3-H 规范 §15 要求托盘提供 Reindex / Pause Hotkey 一跳入口。当前 Core/服务层
没有对应的受控公共能力（IndexCoordinator 无手动 rescan 触发口；hotkey service
无暂停通道）。**不为满足 P3-H 从 Slint/UI 层直调内部实现、也不在 UI 层私设
boolean 状态**（Pause Hotkey 涉及全局热键服务生命周期，属 launcher lifecycle，
不是 UI 状态）。

规定顺序：

```text
P3-H（发现缺 public capability）
  ↓ 本 ADR 登记
  ↓ Core API 设计（需满足：权限边界、可测试、不绕过既有生命周期管理）
  ↓ 实现 + 测试
  ↓ Tray UI 接线
```

Follow-up：

- [ ] `Core::request_reindex()`（或 IndexCoordinator 显式 rescan 触发口）+
      托盘 Reindex。
- [ ] HotkeyService 暂停/恢复通道（带自动恢复或显式状态提示）+ 托盘 Pause
      Hotkey。
- [ ] Check for Updates：依赖现有 update handoff 设施评估是否公开。

## 决策 2 — 左键单击 = Execute：保持 legacy，不改

规范 §14 写"Single click → Select, Double click → Execute"，但现状是
单击执行主 action。该语义由三者共同固化为 legacy constraint：

1. 既有 UI-CONTRACT（§9/§11 行为约定）；
2. VR Snapshot 回归基线；
3. 既有用户行为。

**P3-E 结论：Conditional PASS** — context-menu 交互已统一（右键复用同一
Action Panel：同 action id、同 execute-action 路径、同 Esc 关闭）；单击语义
有意保留既有 UI Contract，作为独立 UX 决策延后（若改，须先更新 UI-CONTRACT
并重制 VR 基线，不允许在本轮任务内顺手改）。

## 决策 3 — P3-I 的两个显式缺口：按正确链路延后，不在 UI 层偷塞

### 3a. Unavailable / Recovering 状态

UI 当前以 provider 级 Unavailable（`VR-015-Main-ProviderUnavailable`）呈现，
插件级 Quarantined/Recovering 的完整卡片需要运行态数据。**禁止**在
PluginProvider 缺口上由 UI 臆测状态或私塞临时字段。规定链路：

```text
Plugin Control Plane
  ↓ Plugin Runtime State
  ↓ PluginProvider
  ↓ UI View Model
  ↓ Unavailable / Recovering 呈现
```

否则会制造第二套 Plugin Runtime State（与 UI 不拥有 trust/capability
authority 的冻结边界冲突）。

### 3b. Error Recovery Action

Error 状态目前为 `Error → RuntimeMessage`（语义错误色 + 文本提示），**完全
可接受**。禁止为了让 UI"看起来完整"而让 Recover 按钮直调某个恢复函数——
恢复是 control-plane capability，须在 P3-J 之后做专门的 capability/control
plane 设计（参照决策 1 的顺序：缺口 → ADR → Core API → UI）。

