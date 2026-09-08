# PLUGIN-CONTRACT-v0.1 — 实现状态快照

> **状态更新（2026-09-04，ADR-0007）**：本文档是契约冻结前的实现快照。冻结稿 `PLUGIN-CONTRACT-v0.1.md` 的 P0/P1 已落地：
> - Manifest v2 字段（`schema_version`/`version`/`runtime{type,executable,args}`）已实现，`runtime.executable` 优先于遗留顶层 `executable`；
> - `initialize`/`shutdown` 握手 + `query_id` 回显已在 plugin-host/plugin-api 强制；
> - 错误码表 `launcher_ipc::error_code`（-32001..-32007）已冻结；
> - Capability 线上名改为点分（`network.connect` 等 15 项，含 `context.*`、`store.*`），旧 snake_case 不再接受；
> - 参考插件 `apps/calculator-plugin`（PLUGIN-014）经真实 PluginHost e2e 验证。
> 下文保留为实现快照（capability/manifest 段落描述的是 ADR-0007 之前的形态）。

# PLUGIN-CONTRACT-v0.1 — 插件契约冻结稿

来源：`10-mvp2-0.6`（Plugin Architecture / SDK Design Freeze）。本文件冻结**协议与架构**，不是插件生态。语言 SDK（Rust/Python/Node）只是本契约的实现；协议一旦发布，变更需 ADR + 版本 bump（INV-009/010）。

## 0. 设计来源（评审矩阵摘要）

| 设计项 | 主要来源 | 我们的决策 |
|---|---|---|
| RPC / 跨语言 | Flow Launcher（stdio JSON-RPC 统一边界） | newline-delimited JSON-RPC over stdio，保持不变 |
| Command/Action 模型 | Raycast（Command→Action，声明意图） | 插件声明意图，Host 决定呈现；不暴露 UI 控制原语 |
| Manifest / 开发体验 | uTools（plugin.json feature 声明） | 抄 Manifest 声明方式，**不抄** HTML/preload/WebView 运行模型 |
| 轻量 Host / 脚本插件 | Wox | 外部进程即边界，无内嵌运行时（INV-002） |
| 扩展面（Context/Preview） | Lertaro | Context/Preview 作为 capability，非每个插件必备 |
| Capability / 沙箱 | Asyar | requested → granted → enforced 三层（ADR-0006） |

## 1. Manifest Schema v1（forward-compatible）

当前实现：`launcher_domain::PluginManifest`（`crates/launcher-domain/src/lib.rs`）。

```json
{
  "id": "com.example.github",
  "name": "GitHub",
  "api_version": "0.1",
  "executable": "plugin.exe",
  "capabilities": ["network"],
  "timeout_ms": 2000,
  "idle_timeout_ms": 10000
}
```

冻结规则：
- `id`/`name`/`executable` 必填；`executable` 为插件目录内相对路径（INV-013 拒绝绝对/`..`/UNC）。
- `api_version` 当前仅接受 `"0.1"`；未来新增字段 MUST 可选（unknown-field tolerance），保证旧 Host 能加载新 Manifest 的子集。
- 计划字段（schema v1 预留，暂不实现）：`schema_version`、`version`、`runtime{type,args}`、`commands[]`、`limits{max_result_bytes}`、`permissions{filesystem[]}`。
- `timeout_ms` ∈ (0, 60000]。

## 2. IPC：newline-delimited JSON-RPC（stdio）

消息形态（`launcher_ipc::{Request,Response}`）：

```json
{"jsonrpc":"2.0","id":42,"method":"query","params":{"query":"term","context":{}}}
{"jsonrpc":"2.0","id":42,"result":{"commands":[...]}}
{"jsonrpc":"2.0","id":42,"error":{"code":-32601,"message":"unknown method"}}
```

冻结规则：
- 每行一条消息；请求 MUST 带 `id`，Host 按 id 关联响应。
- 方法集（`launcher_ipc::method`）：`query`（必需）；预留 `action` / `context` / `preview` / `settings` / lifecycle（`init`/`shutdown`）。
- **`query()` 不得演化成万能 API**：新能力一律走新 method + capability 声明，禁止在 params 里夹带 UI 控制/副作用指令。

## 3. Command / Action Schema

插件结果即 `launcher_domain::Command` 数组，字段与内置 Provider 完全一致（`id`、`title`、`subtitle`、`icon`、`provider_id`、`score`、`keywords`、`category`、`actions[]`、`target`）。Action 只有 `kind + payload`，执行统一进入 Action Engine（INV-004）：

```text
Plugin → Command{actions:[{kind:"open_terminal", payload:path}]} → Host → ActionEngine → Effect
```

插件不得请求 ShellExecute 类自由执行；敏感 kind 受 capability 门禁（ADR-0006）。

## 4. Limits（Host 强制）

| 限制 | 值 | 强制点 |
|---|---|---|
| 结果条数 | `MAX_PLUGIN_RESULTS = 100`，超出截断 | `launcher_ipc::sanitize_results` ✅ |
| 结果 schema | 非法字段/类型 → 丢弃 | `sanitize_results` ✅ |
| 查询超时 | manifest `timeout_ms`（默认 2000ms） | `launcher-plugin-host` ✅ |
| 空闲回收 | manifest `idle_timeout_ms`（默认 10000ms） | `launcher-plugin-host` |
| 进程树 | 崩溃/超时 → Job Object 全树击杀 | `job.rs`（INV-014）✅ |

## 5. Capability Model（v0.1）

```text
filesystem_read / filesystem_write / clipboard_read / clipboard_write /
network / shell_execute / process_spawn / notifications / ui_render
```

- Manifest 声明 = **requested**；Host 配置允许 = **granted**；运行时 Broker 检查 = **enforced**（ADR-0006）。
- 未声明即未授予；denied 时 method 返回 JSON-RPC error（capability denied），插件不得绕行。

## 6. UI Schema v0.1（最小集）

UI 不使用 WebView（架构红线）。插件永远只见 JSON，不见 Slint/Window/Widget：

```json
{ "view": { "type": "list", "items": [ { "title": "...", "subtitle": "...", "icon": "..." } ] } }
```

v0.1 仅冻结 `list`（即 Command 列表）；`text / markdown / form` 为 v0.2 预留。原则：**插件声明意图，Host 决定呈现**；禁止 `show_popup()` / `set_html()` 类 API。

## 7. Versioning & Compatibility

- `api_version` 语义化：破坏性变更 bump minor（0.1 → 0.2）并要求 ADR。
- Host MUST 拒绝不支持的 `api_version`（现状 ✅），MUST 忽略未知 manifest 字段（forward-compatible）。
- 协议（method/params/result/error）任何变更：INV-009 版本 bump + ADR。

## 8. 落地路线（对应 10-mvp2-0.6 §17）

已完成：External Process Host、Rust plugin-api crate（`serve()`）、Echo 插件、contract tests（crash/slow/flood/malformed）、Capability enforcement、Job Object。
下一步：PLUGIN-002 Manifest 预留字段、PLUGIN-003 Command/Action Schema 测试套件、PLUGIN-012 Reference Plugin（Rust 真插件，如 Calculator Extended），再之后才做 Python SDK（跨语言协议试金石）。
