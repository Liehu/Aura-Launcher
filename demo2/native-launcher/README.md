# Native Launcher 1.0

Keyboard-first, native-rendering Windows launcher. Rust + Slint (software
renderer), no Electron/CEF/WebView. Indexer and plugin runtime are separate
boundaries; plugin processes are spawned on demand and shut down when idle.

## 完成状态（2026-09-08，1.0 RC1 → GA Closure）

**Native Launcher 1.0.0 RC1**：核心产品完成、核心架构冻结、平台基础设施完成。
当前权威基线：**P2.4 进行中（见 docs/history/）** / zero warnings / topology
17 crates + 9 apps /
adversarial S0=S1=S2=0 / Release Gate G01~G12 PASS / 30min 资源 soak PASS /
10,000 次 show/hide soak PASS / hotkey→popup P50 307µs · P95 19.7ms（目标
20ms/35ms）**。逐条验收基线见 `demo2/files2/97-launcher-1.0-final-audit.md`
（权威 1.0 baseline：Closed / Core Slice / Experimental / Planned / Failed 映射 +
GA 前必须项 + P2.4 正式推迟清单）；里程碑全景见 `docs/PROJECT-HANDBOOK.md`。

> 下方历史里程碑（M0~M5、MVP2~4.x、P2.x）按时间序保留，其中测试数字是当时快照；
> 当前权威基线以本节为准。

| 里程碑 | 状态 | 说明 |
|---|---|---|
| M0 基础工程 | ✅ | workspace（见 ARCHITECTURE.md）、CI、AGENTS/ADR/文档 |
| M1 Launcher Core | ✅ | 热键（可配置）、popup、command、action、托盘常驻 |
| M2 Search | ✅ | 应用（Start Menu + Uninstall 注册表）、文件索引（21 万条实测）、最近文件 |
| M3 Context | ✅ | **MVP2.1 完成**：Explorer 当前目录（IShellWindows COM）→ ContextSnapshot → Context-aware Provider → Quick Switch + Open Terminal Here / Copy Path → Action，GUI 端到端验证通过 |
| M4 Plugin | ✅ | 外部进程插件、stdio JSON-RPC、超时/崩溃/畸形/洪泛隔离、空闲自动退出 |
| M5 Performance | ✅ | baseline v1（环境元数据+Max+PeakPrivate）+ 回归门禁 + soak 趋势检测；空闲 Private 12.6MB |

P0 功能：全局热键、popup、搜索、应用/文件 Provider、键盘导航、Enter 执行、
Esc 关闭、基础 Action、SQLite、External Plugin Host、超时/崩溃处理、
benchmark harness —— 全部实现。
P1：recent provider ✅、settings ✅（TOML 配置）；Quick Switch / preview 未做。
暂缓项（AI/MCP/Workflow/Marketplace/Python/Node/WASM/跨平台）按计划未做。

### Plugin Contract v0.1 — Frozen（2026-09-04，ADR-0007/0008/0009/0010）

✅ Contract frozen（breaking → v0.2，additive → v0.1.x）　✅ Manifest v2　✅ 协议握手/版本协商
✅ query_id 关联　✅ 生命周期（优雅关闭/进程树隔离）　✅ Capability 点分命名
✅ 资源/帧双层限制　✅ Contract Test Kit（16 项）　✅ Rust 参考插件（Canonical）
✅ Python SDK + Python 参考插件　✅ 跨语言 E2E

**下一阶段：MVP3 — Native Extension Experience**（协议扩展期结束，review 14）

- MVP3.0 Command + Action Contract ✅ **FROZEN**（ADR-0011）：ActionDescriptor → ActionResolver → ResolvedAction → ActionEngine 信任边界落地；calculator-plus 参考插件 + 验收测试通过
- MVP3.1 Native Action UI ✅（P0 完成，`docs/MVP3.1-ACCEPTANCE.md`）：同窗口 Action Panel、primary/secondary/disabled/hidden 呈现、Ctrl+↓ 开面板、id 选择、结果反馈；P1（shortcut/confirmation/context refresh）未做
- MVP3.2 Advanced Action Lifecycle ✅（ADR-0013）：Shortcut（Ctrl+Shift+C 直达 Copy）/ Confirmation（二次 Enter）/ Context generation 守卫 / `system.paste` Effect；桌面 E2E 待人工验证
- MVP3.3 richer Python SDK → MVP3.4 Node SDK；WASM 单独评估
- MVP4.0 Plugin-owned Action RPC ✅ **FROZEN**（ADR-0014 + review 23/24 关账确认）：execute_action RPC + PluginBroker + plugin.invoke + 身份绑定纯函数（边界矩阵测试）+ execution_id/context_generation 语义 → MVP4.1 Workflow 契约 ✅ **FROZEN**（WORKFLOW-CONTRACT-v0.1 + ADR-0015：五对象/线性 Step/8 类失败矩阵/双 Resolver/Confirmation 暂停语义；实现 = CAT-WF-001~012；Addendum WF-A1~A4 已修入，下一步 MVP4.1 implementation）→ MVP4.2 AI Planner ✅（ADR-0016：ActionProposal 生产者，`execute_proposals` 零新通道 + KeywordPlanner 参考实现 + 伪造字段对抗测试） → MVP4.3 MCP Adapter ✅ Phase 1-4（ADR-0018：launcher-mcp crate + stdio transport + Tool Catalog + McpProvider + empty-query discovery；Phase 5-12 executor/Effect/Workflow/AI 集成待续）
   并行线：Launcher UX ✅ **UI-CONTRACT-v0.1 Fully Conformant**（ADR-0017 + Conformance Audit + P0 presentation 收尾 F2~F6 全清 + Workflow Runtime Surface + 触发集成闭环：托盘触发 → 后台 runner → 实时进度 → Confirmation 暂停/恢复全生命周期；下一阶段 = Visual & Interaction Design Review）（Search → Context → Command → Action → Confirmation → Workflow → AI Proposal → Runtime Status 全生命周期已有冻结语义，具备评审输入条件）
- 继续不做：Store、Context supply、资源 quota、cancel RPC（排在 Action 之后）

细节见 `ARCHITECTURE.md`（系统框架/插件框架）、`docs/TESTING.md`（测试情况）、
`docs/PERFORMANCE.md`（性能/内存）、`docs/KNOWN-ISSUES.md`（已知限制）。

## Build & test

```bash
cargo build --workspace
cargo test --workspace          # unit + integration + plugin contract + stress
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Run

```bash
cargo run -p launcher-app
```

- default hotkey `Ctrl+Space` (configurable) toggles the launcher popup
- tray icon: left-click summons, menu offers quit (resident while hidden)
- Quick Switch: open the popup while Explorer has a folder in front — its
  files plus "Open Terminal Here" / "Copy Folder Path" appear immediately
- type to search: applications (Start Menu + registry Uninstall keys),
  recent files, indexed files (Documents / Desktop / Downloads + `index_dirs`),
  context commands, plugin commands
- config: `%APPDATA%\NativeLauncher\config.toml` (hotkey / theme_color /
  autostart / index_dirs); logs: `%APPDATA%\NativeLauncher\logs`
- `↑/↓` navigate, `Enter` executes, `Esc` dismisses

## Plugins

Drop a plugin directory under `%LOCALAPPDATA%\native-launcher\plugins\<id>\`
containing `plugin.json` and the executable:

```json
{ "id": "echo", "name": "Echo Plugin", "executable": "example-echo-plugin.exe" }
```

The plugin speaks newline-delimited JSON-RPC on stdin/stdout
(`launcher-plugin-api::serve`). See `plugins/examples/echo-plugin/`.
Plugin framework details: `ARCHITECTURE.md` §插件框架.

## Benchmark

```bash
cargo run -p launcher-bench          # print + target check
cargo run -p launcher-bench record   # write benchmarks/baseline.json
cargo run -p launcher-bench check    # fail on >10% regression
```

## Layout

| Path | Role |
|---|---|
| `crates/launcher-ai` | P0-D LLM planning: LlmProvider (mock/OpenAI-compatible) + LLMPlanner — proposal-only, no executor/authority access |
| `crates/launcher-runtime` | external process lifecycle (P0-A): spawn/Job Object/bounded IO/shutdown/reap — no protocol, no authority |
| `crates/launcher-domain` | pure model: Command/Action/Provider/Manifest |
| `crates/launcher-search` | deterministic scoring/ranking |
| `crates/launcher-indexer` | directory scan + SQLite index (Phase 1) |
| `crates/launcher-context` | ContextSnapshot engine |
| `crates/launcher-action` | Action Engine (only path to effects) |
| `crates/launcher-ipc` | JSON-RPC message model + result sanitization |
| `crates/launcher-plugin-host` | plugin process lifecycle, timeout/crash isolation |
| `crates/launcher-core` | provider registry + query orchestration |
| `crates/launcher-workflow` | Workflow Runner + AI ActionProposal pipeline (ADR-0015/0016, orchestration only) |
| `crates/launcher-mcp` | MCP Adapter: stdio transport + Tool Catalog + McpProvider projection (ADR-0018, proposal-only) |
| `apps/example-mcp-server` | MCP stdio fixture server (mcp-calculator) + E2E host |
| `crates/launcher-config` | TOML config (hotkey/theme/autostart/index_dirs) |
| `crates/launcher-hotkey` | hotkey parsing + RegisterHotKey thread |
| `crates/launcher-providers` | registry Uninstall + recent files providers |
| `crates/launcher-ui` | Slint presentation + tray |
| `apps/launcher-app` | merged UI+Core binary (ADR-0002) |
| `apps/launcher-indexer-service` | standalone indexer process (stdio JSON-RPC) |
| `apps/example-echo-plugin` | example external plugin |
| `apps/calculator-plugin` | calculator external plugin (math queries) |
| `apps/example-testplugins` | contract-test plugins (normal/slow/crash/malformed/flood) |
| `apps/launcher-bench` | perf baseline harness |
| `apps/launcher-plugin-cli` | plugin dev CLI (init/validate/package/install/uninstall/run/inspect, P2.4-D) |

## Test coverage vs. test plan

Full details in `docs/TESTING.md`.

- Unit (54): domain serialization, manifest validation, ranking, indexer,
  context staleness, IPC framing + flood truncation, action validation,
  config fault tolerance, hotkey parsing, registry/recent providers
- Integration: plugin contract suite (normal / slow-timeout / crash /
  malformed / flood-truncation / spawn-soak), 100-provider stress with
  latency guard
- UI smoke: real-GUI end-to-end (hotkey → search apps/files/plugin →
  keyboard → Esc), verified on release build
- Performance: baseline v1 with environment metadata, P50/P95/P99/Max,
  peak-private sampling, and `soak` trend detection (staircase/slope);
  P50/P95 gated at +10%; popup latency instrumented as T1/T2 (see
  docs/PERFORMANCE-CONTRACT.md)

Known v0.1 limitations are listed in `docs/KNOWN-ISSUES.md`.
