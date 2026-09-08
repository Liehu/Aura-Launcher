# ADR-0007: Plugin Contract v0.1 冻结实现（协议变更）

日期：2026-09-04　状态：Accepted　来源：`docs/PLUGIN-CONTRACT-v0.1.md`（冻结稿）+ `docs/PLUGIN-DESIGN-REVIEW.md`（评审 §36 P0/P1）

## Context

MVP2 已有 stdio JSON-RPC 外部进程插件（ADR-0001/0005/0006），但协议只有 `query` 一个方法、无握手、无 query_id 回显、capability 采用 snake_case 粗粒度命名。Contract v0.1 冻结评审后要求把这些缺口协议化（INV-009：公开协议变更必须记录）。

## Decision

1. **Capability 点分命名**：协议层采用契约 §10 的点分名（`network.connect`、`context.location.read`、`store.read` 等 15 项），serde 逐变体 `rename`。旧 snake_case 线上名不再接受（未发布过的实现细节，非破坏兼容）。新增 `context.process/window/location/selection.read` 与 `store.read/write` 枚举变体；`context.*` 当前仅声明+校验，运行时 Context 供给随后续 MVP 接入。
2. **Manifest v2 字段**（全部可选、forward-compatible）：`schema_version`（出现时必须为 1）、`version`、`runtime{type, executable, args}`（`type` 仅 `"process"`）。`runtime.executable` 优先于遗留顶层 `executable`；两者均可缺一。
3. **握手**：spawn 后 Host 必须发 `initialize`（带 `protocol_version`/`plugin_id`），插件必须回显受支持的 `protocol_version`；不支持 → `-32007 VERSION_MISMATCH` 并终止进程（契约测试 `version_negotiation_rejects_unsupported_protocol`）。
4. **query_id 回显**：Host 为每次 query 生成 `q-<seq>`，params 为 `{query_id, text, limit}`；插件结果必须为 `{query_id, commands}` 且回显一致，不匹配视为协议违规（Malformed）。裸数组作为冻结前遗留形态容忍。
5. **错误码表**：`launcher_ipc::error_code` 冻结标准码 + 私有区间 `-32001..-32007`。
6. **优雅关闭**：`shutdown` 方法 → 插件自行退出（200ms + 宽限轮询），超时才强杀；Job Object（ADR-0005）不变。Job Object 的 CREATE_SUSPENDED→assign→resume 顺序已在 ADR-0005 落地，本 ADR 不变更。
7. **参考插件**：`apps/calculator-plugin`（PLUGIN-014）作为契约全链路参考实现：manifest v2 + 零 capability + 握手 + 回显，e2e 测试经真实 PluginHost 验证。

## Consequences

- 未实现 capability（如 `context.*`）的 enforced 路径是后续 MVP 工作；声明本身已被 manifest 校验。
- 第三方旧式（snake_case capability / 无握手）插件需更新到 v0.1 契约才能加载——发布前冻结期内的预期破坏。
