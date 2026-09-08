# ACTION-CONTRACT-v0.1（Revision 2 · Proposed）

**Status:** **FROZEN v0.1**（review 16 有条件通过；语义修补完成后经 ADR-0011 冻结，2026-09-04）
**Date:** 2026-09-04
**来源:** review 14 + review 15；Raycast(Action Context)、Asyar(Capability-as-Action)
**不变量:** 所有 Effect 仍 MUST 经 Action Engine（INV-004）；插件提供的 ActionDescriptor 是**不可信输入**，必须先经 Action Resolution（INV-026）

---

## 1. ActionDescriptor

```json
{
  "id": "copy",
  "title": "Copy",
  "type": "system.copy_to_clipboard",
  "input": { "text": "80" },
  "requires": [],
  "confirmation": "none",
  "shortcut": null
}
```

## 2. Action Type Namespace 与 Execution Domain

| namespace | Execution Domain | 执行者 | 信任模型 |
|---|---|---|---|
| `system.*` | **Host-owned Effect** | Host Action Engine | 完全可信路径 |
| `plugin.<id>.*` | **Plugin-owned Effect** | 经 `action` RPC 回调插件，但**永远**经 Host Broker（validation + capability + process isolation） | 与 query 同级不可信 |

**`plugin.*` 不是"插件自己执行任意代码"**：它只是把 Effect 的计算交回插件进程，执行决定权、capability 检查、进程隔离全部仍在 Host（`action` RPC 未实现，需 Plugin Contract v0.1.x additive + 单独 ADR）。

## 3. Input Schema（type → input 表格契约）

不引入 JSON Schema，按表冻结；Host 校验 `validate(type, input)`：

| type | input | 校验 |
|---|---|---|
| `system.copy_to_clipboard` | `text: string` | 非空 |
| `system.open` | `target: string`（路径/URL） | 非空 |
| `system.reveal` | `path: string` | 非空 |
| `system.open_terminal_here` | `path: string`（目录） | 非空 |
| `system.execute` | `command_line: string` | 非空；capability `shell.execute`/`process.launch` 必须声明 |

未知字段忽略；`input` 缺失/类型不符 → `InvalidInput`（不执行）。

## 4. Capability Requirement

**`Action.requires ⊆ Manifest.capabilities`**（INV-027）——action 不能临时申请 manifest 未声明的 capability，堵死"偷偷扩权"路径。

```text
Action.requires = ["clipboard.write"]
  → Manifest 未声明          → Action 丢弃（声明期）
  → 声明但用户未授予          → CapabilityDenied（执行期，见 §9）
  → 声明且授予               → Action Engine 执行
```

三层模型不变（ADR-0006）：Requested → Granted → Enforced。

## 5. Context Requirement

secondary action 可携带 `requires_context`（语义同 COMMAND-CONTRACT §4：eligibility，非 permission），由 Host 按 Popup Session 冻结的 ContextSnapshot 判定可见性。

## 6. Action Resolution（安全边界，本轮核心）

```text
Plugin（UNTRUSTED）
    ↓ ActionDescriptor
ActionResolver
    ├── type resolution
    ├── input validation
    ├── context eligibility
    ├── capability validation
    └── confirmation policy
    ↓
ResolvedAction（TRUSTED）
    ↓
Action Engine（只接收 ResolvedAction）
    ↓
Effect
```

Host 内部形态：

```rust
enum ActionResolution {
    Ready(ResolvedAction),
    Hidden(HideReason),      // 不展示（未知 secondary / context 不满足）
    Disabled(DenyReason),    // 展示但禁用（capability denied / 未知 primary）
    Invalid(InvalidReason),  // 结构非法
}
```

未知 type 的处置（INV-030/031）：**secondary → Hidden；primary → Disabled（或 fallback 下一合法 action）；永不执行**。一个非法 secondary action 绝不使整个 Command 消失。

## 7. Execution

`ActionEngine::execute(ResolvedAction) -> ActionResult`。`system.*` 映射现有 `ActionKind`（Open/Copy/Reveal/OpenTerminalHere/Execute），v0.1 不新增 Effect 种类。

## 8. Primary / Secondary

Primary = 第一个 `Ready` 的 action（COMMAND-CONTRACT §5）；**Enter 恒等于执行 primary**（calculator-plus 验收语义：Copy=primary → Enter 复制；Insert/History=secondary → Action Panel）。`role` 字段 v0.2 预留。

## 9. Error Semantics：Action Execution Result ≠ Plugin RPC Error

`CapabilityDenied` 等是 **Action Engine 的执行结果**，不是 Plugin 协议错误：

```text
Plugin Protocol Error（JSON-RPC）: -32700/-32600/-32601/-32602 + -32001..-32007
    （仅用于 Plugin ↔ Host RPC 通道）

Action Execution Result（引擎内部）:
    Success | CapabilityDenied | InvalidInput | UnknownActionType
    | ConfirmationRequired | EffectFailed
```

理由：AI/Workflow 触发的 action 同样会产生 CapabilityDenied，显然不是"插件 RPC 出错"。`-32001` 保留在 RPC 区间用于插件主动调用受限 Host API 的场景。

## 10. Security Boundary

- Action Engine 永不接收原始 ActionDescriptor（INV-026）。
- Core 永不执行插件提供的裸 Effect。
- `plugin.*` 执行路径与 query 同级隔离（timeout/进程树/结果上限全部适用）。
- `confirmation` 字段 v0.1 预留不实现。

## 10.1 Revision 3 语义修补（review 16，冻结前最后一批）

1. **`plugin.*` = Defined Namespace ≠ Supported Execution**：v0.1 中 reserved/unsupported；出现即按未知 type 处置（secondary→Hidden，primary→Disabled），与未知类型规则统一。
2. **Capability monotonicity（INV-027 强化）**：`Action.requires ⊆ Manifest.capabilities` 且 Host **禁止自动补权限**——未声明即 UNSATISFIABLE，action 永远不能扩大插件声明权限。
3. **CapabilityDenied 是 Execution state，不是 Presentation**：契约只冻结 `ActionResult = CapabilityDenied`；置灰/隐藏/拒绝由各呈现面（Action Panel/搜索结果/AI/Workflow）的 Host policy 决定，UI 不被 Action Contract 绑死。
4. **ActionResolver UI-independent**：Resolver 属于 domain/action 层（`launcher_domain::resolve_descriptor` 纯函数），禁止进入 `launcher-ui`；AI/Workflow/MCP 与插件共用同一解析路径。
5. **Action Resolution Context**：Resolver 逻辑上接收 `Command + ActionDescriptor + ContextSnapshot + PluginPermissionState + HostPolicy + Action registry`（v0.1 仅实现 descriptor + capability；其余为签名预留方向）。
6. **INV-033（新增，最强边界）**：ActionEngine MUST 只接受 ResolvedAction；raw ActionDescriptor 永远到不了 ActionEngine。对 AI/MCP 同样适用——AI 生成的 descriptor 与插件输出同级不可信。
7. `ActionDescriptor` 是数据，`ResolvedAction` 才是可执行对象——本轮冻结的核心一句话。

## 10.2 Resolution 时序（review 17 §10，冻结语义）

**v0.1 采用"ResolvedAction 包含执行所需的全部不可变条件"模型**：

- Descriptor 在 query 结果进入 Host 时解析一次（`launcher_domain::resolve_descriptor`）；ResolvedAction 自此自足，ActionEngine 不再重新解释 Descriptor（INV-033）。
- 依据：v0.1 中 capability 按 manifest 静态声明（会话内不变）、ContextSnapshot 按 Popup Session 冻结（INV-011 生命周期）——展示（t2）到执行（t3）之间不存在会变化的输入。
- **UI 只保存 `command_id` / `selected_index`，不保存 action 语义**（INV-035：UI 是 projection）。
- v0.2 方向：当 capability 支持用户运行时撤销、context 支持会话内刷新时，迁移到 "resolve at execution boundary"（UI 存 id，Enter 时重解析）。迁移只影响 Host 内部时序，契约字段不变。

## 10.3 MVP3.2 语义（ADR-0013，additive v0.1.x）

### Shortcut（3.2-A）

- `ActionDescriptor.shortcut`（如 `"Ctrl+Shift+C"`）→ 解析后随 ResolvedAction/ActionPresentation 透传。
- 分发链冻结：**shortcut → 选中 command → action_id → ActionEngine**；快捷键永不直接绑定 Effect（INV-039/040）。
- 冲突规则：同一 Command 内多个 action 声明相同 shortcut → **第一个声明生效**（确定性，与控件顺序无关）；applicability 只在当前选中 command 内判断。

### Confirmation（3.2-B）

- `confirmation: "confirm"` 或 capability 策略（`requires` 含 `shell.execute`/`process.launch`）→ ResolvedAction 携带 `confirmation_required`。
- **Confirmation 是执行策略状态，不是 UI 标志**（INV-041）：引擎 `validate` 拒绝未确认 action（唯一闸门）；host 在用户第二次 Enter 时清除标志（INV-042）。
- UX：首次 Enter → 状态行 "Confirm: press Enter again"；再次 Enter → 执行；已提交 effect 仍不受 Esc 影响。

### `system.paste`（Effect 扩展案例）

- 新 Host-owned Effect：向焦点已回归的原前台窗口发送 Ctrl+V（SendInput）。走完整链 `descriptor → resolver → ResolvedAction → engine`（INV-045-类：NORMAL effect，非面板特殊按钮）。
- 时序：engine 延迟 250ms 发送，覆盖 host 的 park + focus-restore 往返；已文档化。

### Context generation（3.2-C）

- Popup 每次打开 `context_gen += 1`；结果集绑定 `results_gen`。执行前 `results_gen != context_gen` → 拒绝执行 + 提示（**stale resolution 不能静默执行**，INV-043）；context refresh 由 Core/Resolver 拥有，Panel 只消费新 Presentation（INV-044）。

## 11. 验收（calculator-plus 已实现，见 `apps/calculator-plus/tests/mvp3_acceptance.rs` ✅）

1. Enter → primary action（Copy）产生 Effect。
2. Action Panel → secondary（Insert/History）。
3. 缺 capability → Disabled/DENY（Execution Result），UI 呈现"被拒绝"而非静默失败。
4. 未知 secondary → Hidden，Command 保留。
5. 未知 primary → Disabled/fallback。
6. 畸形 action → 按结构非法规则处置，Core 不受影响。
7. Core 任何路径都不执行插件提供的裸 effect。

---

# Addendum 2（ADR-0014，MVP4.0，2026-09-04）：`plugin.*` 执行域已支持

`plugin.<id>.*` 从 "reserved/unsupported" 升级为受支持的 Execution Domain（Plugin Contract v0.1.x additive，新 RPC `execute_action`）：

```text
ActionDescriptor(plugin.<own>.<name>)
  → resolve_descriptor_for(own_plugin)   # 身份绑定 + 隐式 plugin.invoke + requires 子集
  → ResolvedAction(PluginInvoke)
  → ActionEngine（validate/policy）
  → Effect::PluginInvoked
  → Core::execute_plugin_action（PluginBroker = Effect executor，INV-044/045）
  → execute_action RPC（execution_id 回显、context_generation 随行）
  → Plugin Process
```

规则不变式：身份绑定（仅 own plugin，前缀匹配 reverse-DNS）、capability 单调性（隐式 `plugin.invoke` + 声明 requires ⊆ manifest）、execution_id 与 query_id 空间分离、插件级失败 = EffectFailed 不杀进程。Workflow/AI/MCP 仍是 Proposal 生产者/适配器（INV-046），不是新执行机制。
