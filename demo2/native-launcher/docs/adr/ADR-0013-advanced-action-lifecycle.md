# ADR-0013: MVP3.2 — Advanced Action Lifecycle

日期：2026-09-04　状态：Accepted　来源：review 20-mvp3.2-0.1

## Context

MVP3.1 关账后，Action 从"静态可执行描述"升级为具有 **Shortcut → Confirmation → Context → Execution** 生命周期的系统对象。`system.paste` 作为第一个新增 Host-owned Effect 兼作架构验证点。

## Decision

1. **Shortcut（3.2-A）**：`ActionDescriptor.shortcut`（如 `"Ctrl+Shift+C"`）经解析随 ResolvedAction/ActionPresentation 透传；分发链冻结为 `shortcut → 选中 command → action_id → ActionEngine`。冲突规则：同 Command 内第一个声明生效（确定性）；applicability 仅限当前选中 command。快捷键永不直接绑定 Effect（INV-039/040）。
2. **Confirmation（3.2-B）**：执行策略而非 UI 标志（INV-041）。触发条件：descriptor `confirmation: "confirm"` 或 `requires` 含 `shell.execute`/`process.launch`。引擎 `validate` 拒绝未确认 action；UX 为二次 Enter 确认（host 第二次清除策略标志，INV-042）。已提交 effect 仍不受 Esc 影响（MVP3.1 语义不变）。
3. **Context generation（3.2-C）**：popup 每次打开 `context_gen += 1`；结果集绑定 `results_gen`；执行前 generation 不匹配 → 拒绝执行 + 状态行提示（INV-043）。刷新由 Core/Resolver 拥有，Panel 只消费新 Presentation（INV-044）。
4. **`system.paste`**：新 ActionKind + engine effect（SendInput Ctrl+V 到焦点已回归的原窗口；engine 延迟 250ms 覆盖 park+restore 往返）。走完整 descriptor→resolver→engine 链（INV-045），非面板特殊按钮。
5. calculator-plus 升级为四项能力的 UI/Domain Reference：Copy(shortcut) / Insert(confirmation) / History / Paste + 注入的 unknown secondary。

## 明确不做

`plugin.*` action RPC、UI Schema、Workflow/AI/MCP、Store（维持 MVP4/5 排期）。

## Consequences

- 121 tests 全绿（含 CAT-012 shortcut conformance、confirmation gate 测试）。
- calculator-plus 已重新部署到 `%LOCALAPPDATA%\native-launcher\plugins\calculator-plus\`。
- MVP3.2 桌面 E2E 待人工验证：Shortcut（Ctrl+Shift+C 直接触发 Copy）、Confirmation（Insert 首次 Enter 提示、二次执行）、Paste（原窗口光标处出现剪贴板内容）。
