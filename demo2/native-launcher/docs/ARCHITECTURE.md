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
  AppProvider       FileProvider  PluginProvider  recent-files /
  (Start Menu +     (SQLite 索引) (外部进程插件)   app-registry /
   Uninstall 注册表)                              context 命令
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
apps/example-echo-plugin, apps/example-testplugins (示例插件/契约测试 binaries)
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

## 进程边界现状
- `launcher-app`：UI + Core 同进程（ADR-0002，v0.1 允许的临时合并）
- `launcher-indexer-service`：索引可独立进程运行（stdio JSON-RPC：status / rebuild / search / shutdown），已实测 8.8 万文件索引与搜索
- 插件：强隔离外部进程

## Architecture Change Policy

改动进程边界、公开协议、插件模型、DB schema 或 UI 技术前，先新增/更新 ADR（`docs/adr/`）。
