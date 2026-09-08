# ADR-0010: Runtime 抽象冻结与 SDK 边界

日期：2026-09-04　状态：Accepted　来源：review `13-mvp2-0.9`

## Context

`runtime.type` 从单一 `process` 扩展为 `process | python` 后，runtime 成为一个真正的抽象层。若在 plugin-host 内以散落的 `match runtime_type` 继续生长（node/wasm/lua/...），会形成巨型分支。评审要求在只支持两个 runtime 的现在就冻结抽象。

## Decision

1. **RuntimeResolver 单一扩展点**：`runtime.type → LaunchPlan{kind, program, args}` 的映射只存在于 `launcher-plugin-host::runtime::resolve_launch`；`PluginHandle::spawn` 变为 runtime-agnostic。新增 runtime 只改该函数并补 `LaunchPlan` 测试。
2. **`runtime.type` 语义 = Host 启动策略**，不是编程语言：`"python"` 表示 Host 采用 Python 策略启动；`"process"` 启动自带解释器的 host 可执行是合法且语义不同的选择。写入 Contract §29.19。
3. **Runtime Discovery 策略统一**（config → env override → system PATH → unavailable 局部失败），解析来源必须进日志：`runtime.discovered { source }`（app 层）+ `runtime.resolved { program }`（host 层）。`launcher plugins doctor` 属 Developer UX，不进 v0.1 Contract，但日志字段按其需求设计。
4. **作者 API / Host API 边界**：插件作者只依赖 `launcher-plugin-api`（Rust）/ `plugins/python/launcher_plugin.py`（Python）；`launcher-plugin-host/core/context/action` 对作者不可见，SDK 不得 re-export Host 内部类型。写入 AGENTS 红线。
5. **SDK Conformance 分层**：Protocol Conformance（Test Kit）/ SDK Conformance（每 SDK 跑同一 Kit）/ Reference Plugins（Rust calculator = Canonical；Python calculator = 跨语言对照）。见 `docs/TESTING.md`。
6. **继续不做**：Node SDK（信息增量低于 UI Schema + Action Contract）、UI/Action 参考插件（协议新增面，待下一阶段独立 ADR）、Store、Context supply、资源 quota、doctor CLI。

## Consequences

- 未来 Node/WASM 的接入成本 = resolve_launch 一个分支 + 解释器发现策略 + 一份 SDK；Contract 文本零改动。
- Python 多版本/Anaconda/uv/pyenv 的用户排障有 `source` + `program` 日志可查。
