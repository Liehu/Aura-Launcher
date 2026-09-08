# ADR-0008: 插件协议边界硬化（Contract v0.1 Addendum）

日期：2026-09-04　状态：Accepted　来源：review `11-mvp2-0.7`（§1/2/4/5/6/7/8/10/11/13/14 采纳；§16/18 等明确不做）

## Context

Contract v0.1（ADR-0007）冻结后评审指出：协议能跑 ≠ 长期可兼容。缺口集中在版本语义、兼容路径边界、并发语义、通道纪律与资源上限。

## Decision

1. **版本四元组语义互斥**（INV-018）：`schema_version` / `version` / `api_version` / `protocol_version` 各管一层，禁止混用。
2. **Manifest Profile**：`schema_version` 缺省 = Legacy Profile；=1 = Contract v0.1 Profile。
3. **Response Profile 有界兼容**：裸数组仅对 Legacy Profile 容忍；v1 manifest 的裸数组 = 协议违规。防止"裸数组兼容"永久化。
4. **并发语义冻结**：单 PluginHandle 最多 1 个 in-flight query（INV-021）；过期结果 Host 侧丢弃，不要求插件 cancel。
5. **协议状态机**：NotStarted→Spawned→Initializing→Ready⇄Running→ShuttingDown→Stopped；非法迁移 = 违规。
6. **stdout 红线**（INV-019）：stdout 只承载协议帧，日志走 stderr；违反即失败。
7. **byte 级限制**（INV-020）：`MAX_FRAME_BYTES = 256KB`/帧，超限 = 违规 kill（区别于 100 条截断）；`max_result_bytes`/`max_stderr_bytes` 为 manifest 预留字段。
8. **shutdown 宽限期 = Host policy**（默认 200ms），协议只规定 `shutdown requested`。
9. **进程树回收**：不改 ADR-0005 实现，新增真实行为测试（childspawner + ping.exe 孙进程回收）。
10. **不做**：reference-stateful plugin（store 后端未实现）、错误码三层之外的扩展、四层文档重组、Python/Node、cancel RPC、regex trigger。

## Consequences

- 契约测试 16 项（新增 5 项协议边界测试）全部通过；`PLUGIN-CONTRACT-v0.1.md` §29 Addendum 为规范文本。
- calculator-plugin 保持 Canonical Reference Plugin 门禁地位：协议变更必须先过其 E2E。
