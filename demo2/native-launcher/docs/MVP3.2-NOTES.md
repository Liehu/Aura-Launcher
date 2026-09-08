# MVP3.2 实现备注

## 桌面 E2E 修复记录（2026-09-04）

| 问题 | 根因 | 修复 |
|---|---|---|
| Paste 报 `no target to act on` | windows `execute` 在分发前做通用 target 提取，Paste 无 payload → 提前 MissingTarget | Paste 在 target 提取前短路进 `send_paste` |
| Paste 发送时机早于焦点回归（日志 `paste.sent` 早于 `foreground.restore` 76ms） | 250ms 延迟在 execute 内部，而 restore 在 execute 返回之后 | 架构修正：新增 `ActionPayload::Hwnd`，app 注入原前台句柄，effect 自己先恢复焦点再发键击（自包含前置条件，未来 Workflow/AI 调用方无需操焦点时序） |
| 粘贴出 `hi` 而非计算结果 | `cargo test` 的 Copy 单测写了**真实系统剪贴板**，覆盖了用户剪贴板 | 测试构建（`cfg(test)`）下 `copy_to_clipboard` 为 no-op；真实写入仅存在于非测试构建 |

## 语义备忘

- **Paste 粘贴的是当前剪贴板内容**：要粘贴计算结果，先 Copy/Insert 再 Paste（`system.paste` 契约不携带文本输入，保持 Effect 原子性）。
- **Confirmation 双 Enter**：第一次 Enter 只武装确认状态（状态行提示），第二次 Enter 才由 host 清除策略标志并执行（INV-041/042）。
- **Shortcut**：Ctrl+Shift+字母，仅在当前选中 command 内解析，第一个声明生效（INV-039/040）。

## 待人工验证

- [ ] Shortcut：`2*3` → 选中 `= 6` → Ctrl+Shift+C → 粘贴出 `6`
- [ ] Confirmation：Insert → 两次 Enter → 剪贴板 `(6)`
- [ ] Paste：Copy 后开面板选 "Paste at cursor" → 原窗口光标处自动出现结果

## 未决问题（2026-09-04，暂停待斟酌）

Insert action 已从 calculator-plus 移除（与 Copy/Paste 语义重叠）。**Copy / Paste / Insert 三者的产品语义尚未定稿**：
- Paste at cursor 的确认流程（双 Enter）验证可用；
- "一键把结果注入原光标"是否应该是 Copy+Paste 的合成动作、还是保留两步、还是提供 `system.insert_text`（写剪贴板+注入+还原原剪贴板），待产品思考后再定。
- 已实现并保留：`system.paste` effect（Hwnd payload + 自恢复焦点）、shortcut 分发、confirmation 闸门、context generation 守卫——基础设施完备，改产品语义只是插件 descriptor 层的事。
