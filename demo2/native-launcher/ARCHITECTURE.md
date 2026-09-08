# ARCHITECTURE.md — Native Launcher v0.1（当前实现）

基线架构定义见 `demo2/files2/01-design-spec-v0.1.md`。本文档描述**已实现**的系统框架与插件框架（2026-09-03 更新）。

## 系统框架

```text
                +--------------------------------------+
                |            launcher-app              |
                |      (UI + Core 同进程, ADR-0002)     |
                |                                      |
  Ctrl+Space -->  hotkey 线程 -- mpsc -->             |
                |  Slint popup (launcher-ui)           |
                |   输入/↑↓/Enter/Esc + 托盘常驻        |
                |        | invoke_from_event_loop      |
                |        v                             |
                |  Core (launcher-core)                |
                |   Provider registry + 排名 + 历史     |
                +---+---------+---------+---------+---+
                    |         |         |         |
        +-----------+   +-----+----+   +---------+--+
        v               v          v             v
  AppProvider       FileProvider  PluginProvider  ContextProvider
  (Start Menu +     (SQLite 索引) (外部进程插件)   (Quick Switch +
   Uninstall 注册表)                              Open Terminal Here /
                                                    Copy Path)
  ▲ Explorer 当前目录: IShellWindows COM → ContextSnapshot（MVP2.1）
        |               |              |
        v               v              v
  内存目录(有界)   launcher-indexer  launcher-plugin-host
                   SQLite+WAL      进程隔离/超时/崩溃隔离
                   (可独立进程:
                    launcher-indexer-service)
```

- **UI 线程不做 IO/进程等待**：查询与 Action 执行在后台线程完成，经 `invoke_from_event_loop` 回传结果。
- **常驻成本**：软件渲染器 + 托盘常驻 + 空闲即隐藏；空闲 Private ≈ 12.6 MB（见 `docs/PERFORMANCE.md`）。
- **事件循环**：`run_event_loop_until_quit` + 托盘，隐藏窗口后进程常驻等待热键/托盘。

## Crate 依赖方向（无环）

```text
launcher-domain (纯模型, 无 OS/IO 依赖)
    ^
    |
launcher-search / launcher-context / launcher-action / launcher-ipc / launcher-indexer
    ^                                       ^
    |                                       |
launcher-core (Provider trait, 编排)   launcher-plugin-host / launcher-plugin-api
    ^
    |
launcher-config / launcher-hotkey / launcher-providers / launcher-ui
    ^
    |
apps/launcher-app (组装入口, ADR-0002 UI+Core 同进程)
apps/launcher-indexer-service (独立索引进程, stdio JSON-RPC)
apps/example-echo-plugin, apps/calculator-plugin、calculator-plus、example-mcp-server
- `crates/launcher-workflow`、`crates/launcher-ai`（P0-D LLM Planner：LlmProvider + 严格 schema + catalog 精确绑定——proposal-only）、`crates/launcher-runtime`（P0-A 外部进程生命周期：spawn / Job Object / bounded IO / shutdown / reap——不含协议与授权语义）、`crates/launcher-mcp`（MCP Adapter，ADR-0018）：Workflow Runner（WORKFLOW-CONTRACT-v0.1 / ADR-0015，orchestration only）, apps/example-testplugins (示例插件/契约测试 binaries)
apps/launcher-bench (性能基线 harness)
```

## Ownership

| Area | Owner crate |
|---|---|
| Domain types (Command/Action/Context/Manifest) | launcher-domain |
| Scoring/ranking | launcher-search |
| Context engine | launcher-context |
| Action engine | launcher-action |
| IPC message model + result sanitization | launcher-ipc |
| Indexing (SQLite, Phase 1) | launcher-indexer |
| Plugin process boundary | launcher-plugin-host |
| Core orchestration (Provider registry, history) | launcher-core |
| TOML config | launcher-config |
| Global hotkey (parse + RegisterHotKey) | launcher-hotkey |
| Extra built-in providers (registry uninstall / recent files) | launcher-providers |
| UI (Slint, tray) | launcher-ui |
| Binary assembly | launcher-app |

## 插件框架（已实现）

### 层级
- Tier 0 内置 Provider：apps（Start Menu + Uninstall 注册表）、files（SQLite 索引）、recent-files、context 命令
- Tier 2 外部进程插件（MVP 唯一对外插件形态；WASM/Python/Node 预留未做）

### 插件协议（ADR-0001）
- 传输：stdin/stdout 上的换行分隔 JSON-RPC（`launcher-ipc` 定义 Request/Response）
- 方法：`query`（必须）；未知方法返回 `-32601`；坏 JSON 返回 `-32700`
- 结果：Native UI Schema（仅 list item：title/subtitle/actions），Host 侧校验并**截断到 100 条**（flood 防护）

### Manifest（`plugin.json`）
```json
{ "id": "echo", "name": "Echo Plugin", "api_version": "0.1",
  "executable": "example-echo-plugin.exe",
  "capabilities": [], "timeout_ms": 2000, "idle_timeout_ms": 10000 }
```
Host 校验：必填字段、`api_version == "0.1"`、`timeout_ms ∈ (0, 60000]`。能力声明为 allow/deny 检查（`PluginManifest::requests`）。

### 生命周期
```text
Discovered(<data>/plugins/*/plugin.json)
  -> Validated(manifest 校验)
  -> Spawned(首次 query 按需拉起)
  -> Running(每次 query 刷新 idle 计时)
  -> Idle 超过 idle_timeout_ms 自动 kill
  -> 超时/崩溃/坏结果 => kill，下次查询重新拉起
```
插件崩溃、挂起（超时 kill）、畸形/超量结果均**不影响 Core**（契约测试覆盖，见 `docs/TESTING.md`）。

### 插件编写
链接 `launcher-plugin-api` 并调用 `serve(handler)`（stdin/stdout 请求循环）。示例：`apps/example-echo-plugin`。安装：`%LOCALAPPDATA%\native-launcher\plugins\<id>\plugin.json` + 可执行文件，重启生效。

## Context 链（MVP2.1 已验证）

```text
Explorer 前台窗口 (hwnd)
  ↓ IShellWindows COM（ LocationURL → 文件路径, hwnd 匹配）
ContextSnapshot.current_folder
  ↓ launcher_core::context_commands + 目录文件列表（有界 15）
ContextProvider（ContextHandle 共享刷新）
  ↓ Quick Switch（空查询直出） / 关键词搜索（参与排名）
Command（Open Terminal Here / Copy Folder Path / 目录文件）
  ↓ Action Engine
Effect（cmd 于该目录启动 / 路径复制）
```
GUI 端到端已验证：Explorer 停在目标目录 → Ctrl+Space → Quick Switch 列表 →
Enter → cmd 打开在该目录。

## 进程边界现状
- `launcher-app`：UI + Core 同进程（ADR-0002，v0.1 允许的临时合并）
- `launcher-indexer-service`：索引可独立进程运行（stdio JSON-RPC：status / rebuild / search / shutdown），已实测 8.8 万文件索引与搜索
- 插件：强隔离外部进程

## MVP2 Architecture Freeze（2026-09-03，采纳 07-mvp2-0.3 §14）

以下决策**冻结**——在 MVP2.1（Explorer Context + Quick Switch 垂直切片）的
性能与质量数据出来之前不重构：

- Rust + Slint **软件渲染器**
- UI + Core 同进程（ADR-0002）——12.6MB 空闲证明"同进程 ≠ 臃肿"
- External Process Plugin + stdio JSON-RPC + Job Object 隔离（CREATE_SUSPENDED 生命周期已冻结）
- SQLite Indexer（Phase 1；MFT/USN 为 Phase 2 且必须独立进程）
- Action Engine 作为 effects 唯一入口；Provider/Command 领域模型
- 性能方法学 v2 + ADR 流程 + AGENTS 规则

"理论上应该拆进程"不构成重构理由；数据出来之后再评审（Architecture Review → MVP3 Design）。

## Architecture Change Policy

改动进程边界、公开协议、插件模型、DB schema 或 UI 技术前，先新增/更新 ADR（`docs/adr/`）。
