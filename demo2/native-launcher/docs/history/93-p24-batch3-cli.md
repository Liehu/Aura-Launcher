# 93 — P2.4 Batch 3: Plugin Control Plane 收尾（C01/C06）+ Plugin SDK CLI（D01–D05）

> 日期：2026-09-08。范围：P2.4 第三批（`demo2/files2/01-P2.4-DESIGN-SPEC.md` §6/§7）。
> 输入基线：history/92（642 tests）。

## Task P24-C01 — Plugin Lifecycle Model / Installation Revision

- `plugins.db` schema v3：`revision` 列（PRAGMA 幂等迁移，存量归零语义安全——
  revision 是相对计数器）。**全部** lifecycle 变更路径
  （enable/disable、failure、trust、manifest observation）原子 `revision+1`。
- `PluginLifecycleRecord`：单一视图暴露 lifecycle 层切片（enabled/quarantined/
  failures/trust/installation_revision），供 cache 失效与诊断消费。

## Task P24-C06 — Plugin Diagnostics（结构化）

- `launcher-core::providers::plugin_diagnostics`（新）：
  `DiagnosticClass`（ok/timeout/crash/malformed/flood/spawn_failed，与
  Plugin Contract 失败分类同词表）+ `DiagnosticEntry`（plugin_id、query_id/
  runtime_id/protocol_session_id 关联、elapsed、frame_size、result_count）+
  `DiagnosticLog`（有界 ring，快照确定性 oldest→newest）。
- 无任何执行路径（纯观测数据；契约由测试钉死）。与 P2.4-E 的
  RPC Inspector/Replay 的集成点已预留（snapshot()）。

## Task P24-D01–D05 — launcher-plugin CLI（新 app，bin `launcher-plugin`）

- `apps/launcher-plugin-cli`（lib+bin 结构；逻辑在 lib，测试进程内驱动
  `run_cli`，不派生测试进程）。依赖：launcher-domain + launcher-plugin-host
  + serde_json（无 SDK 边界违规；CLI 是 host 侧工具）。
- **子命令**（spec §7 全集）：
  - `init <dir>`：scaffold Python 插件骨架（manifest + main.py）
  - `validate <dir>`：manifest parse+validate（复用 launcher-domain 权威实现）
    + entrypoint 存在性 + 路径 confinement 检查
  - `package <dir> --out <f.nlpkg>`：确定性 JSON envelope
    `{contract_version, manifest, files(base64)}`；拒绝空包/超限（32MB）
  - `install <pkg> --root <dir>`：**staged install**——fail-closed 校验
    （contract 版本白名单、每个打包文件名 traversal 检查、entrypoint 存在且
    非空、plugin id 词法校验）→ 写 `<id>.staging` → 校验 staged 副本 →
    原子 swap；重装=替换
  - `uninstall <id> --root <dir>`：id 词法校验后移除
  - `run <dir> [--query TEXT]`：dev driver 走**生产 host spawn 路径**
    （PluginHandle::spawn——Job Object 隔离、bounded IO、协议握手），
    开发工具不绕过 host 隔离（P2.4-D05 红线）
  - `inspect <pkg|dir>`：manifest + 文件清单（含字节数）
- 测试 8 条：init→validate 闭环 / 缺 entrypoint 拒绝 / 非法 id 拒绝 /
  package→inspect roundtrip / staged install+替换+uninstall 闭环 /
  **traversal 包拒绝且 root 零污染** / 未知 contract 版本拒绝 / 未知命令退出 1。

## Task P24-D06 — SDK 合规

- CLI 未引入新 SDK 面；plugin-api/plugin-host 契约未动；
  calculator-plugin / example-testplugins contract kit 继续全绿（workspace gate）。

## Topology

- workspace 17 crates + **9 apps**（+launcher-plugin-cli）；
  README/ARCHITECTURE 已同步，`check_topology.py` 通过
  （README/ARCHITECTURE 数量断言 9/9 一致）。

## Performance Impact

- CLI 为开发期工具，不进常驻路径；core/indexer 改动仅新增只读 API 与
  SQL revision 自增（lifecycle 变更本就是低频操作）。

## Gate 结果

- `cargo test --workspace`：**654 passed / 0 failed**（642 → 654，+12）
- `cargo build --workspace`：零警告；`check_topology.py`：ok（9 apps）

## P2.4 剩余（收尾批）

P2.4-E（E01 Dev Mode 接线 Provider 失败路径 → DiagnosticLog、E02 RPC
Inspector、E03 Diagnostic Snapshot 落盘、E04 Replay、E05 crash/timeout/flood
诊断打通）、P2.4-F（Gate G-A~G-R 接入 release_gate + 文档收口）。
