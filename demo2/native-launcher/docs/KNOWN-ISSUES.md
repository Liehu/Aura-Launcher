# KNOWN ISSUES / v0.1 limitations

1. **Idle memory: resolved.** Switching to the Slint software renderer
   dropped idle Private Bytes from 72.4 MB to 8.9 MB (release, 214k-file
   index open) — well under the 50 MB target. GPU rendering can be
   reconsidered post-MVP if animation quality ever demands it. After porting
   demo1 features (tray/config/recent-files/registry providers) idle Private
   is 12.6 MB - still far under budget.
2. **Hotkey latency not yet instrumented** — Ctrl+Space → visible popup is
   perceptually instant; P50/P95 measurement harness is a follow-up
   (test plan 8.1 target: P50 ≤ 20 ms / P95 ≤ 35 ms).
3. **IME composition** — search input goes through Slint LineEdit; IME
   composition windows work but candidate-window positioning is unpolished.
4. **Indexer Phase 2 (NTFS MFT / USN Journal)** not implemented; Phase 1
   directory scan + rebuild only. No incremental watch yet.
5. **UI and Core share one process** (ADR-0002); splitting requires the
   named-pipe transport ADR.
6. **`unsafe` usage**: exactly two justified blocks — ShellExecuteW in
   launcher-action, foreground activation in launcher-app.
7. **Context: core MVP2.1 done.** Foreground app + Explorer current folder
   (IShellWindows COM) are live; Quick Switch and Open-Terminal-Here /
   Copy-Path commands verified end-to-end. Remaining: selected-items capture
   and non-foreground Explorer matching when the popup is summoned from a
   non-Explorer app (Quick Switch only activates when Explorer is frontmost).
8. **UI soak (1000 open/close loops)** covered only by the plugin spawn soak
   and stress tests; dedicated UI loop automation is a follow-up.
9. **Real-machine search quality**: extension-noise files can dominate
   result pages for common queries (e.g. "chrome" returning many
   chrome-*.svg); the fixture corpus (docs/SEARCH-QUALITY.md) verifies
   ranking logic, and frequency/recency features (spec 7.3) are the planned
   fix for real-world data.
10. **Job Object spawn-to-assign race**: see ADR-0005; harden with
   CREATE_SUSPENDED before shipping execution-capable plugin tiers.

## DISCOVERY-TODO-001（MVP4.3 验收锚点，review 28 §5）

**外部插件尚未实现 empty-text discovery**：`PluginProvider` 对空 query 返回空集，因此 Workflow/AI/MCP 的 `ActionReference` 解析目前只能命中 popup session 结果（WF-006 选项①），fresh-query 选项②对插件如实降级为 `CommandNotFound`。

- 影响：AI Planner 暂用 live query snapshot 作为 catalog（ADR-0016 Addendum 记录的临时适配）。
- 出口：外部插件实现 empty-text discovery 后，`ReferenceResolver::fresh_query` 自动接管，Planner 零改动。
- **进展（2026-09-05，MVP4.3/ADR-0018）**：MCP 半边已闭合——`launcher-core::providers::mcp::McpProvider` 对空 query 返回 catalog 全量（`LAUNCHER_SNAPSHOT` 无关；MCP-005 ✅）。
- **进展（2026-09-07，外部插件半边闭合 ✅）**：`PluginProvider` 空 query 触发 discovery（`discover()`：spawn + 空 text query，best-effort）；`launcher-plugin-api::serve_with_catalog` 让插件声明静态 catalog（空 text 返回，score 0.0 不进 popup 排名）；结果条目可选 `id` 字段落地稳定命令身份（INV-028）。闭环验证：`apps/calculator-plus/tests/discovery_workflow.rs`——空 session 下 ActionReference 经 fresh discovery 解析并通过 PluginBroker 执行（`{"provider": ..., "result": {"value": "6"}}`），错误 reference 如实降级 CommandNotFound。Planner 零改动（fresh_query 自动接管）。
- **状态：已关闭（两半均闭合）。**
- 护栏：禁止"live catalog 临时方案"悄悄变成永久双路径（ADR-0016 Addendum 2）。
