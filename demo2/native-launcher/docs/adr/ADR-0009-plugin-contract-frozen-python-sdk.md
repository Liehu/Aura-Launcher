# ADR-0009: Plugin Contract v0.1 冻结 + Python SDK / 脚本运行时

日期：2026-09-04　状态：Accepted　来源：review `12-mvp2-0.8`（Freeze Gate 全项通过）

## Context

ADR-0007/0008 后，契约冻结门禁 checklist（协议/Manifest/Process/Testing 四组）全部通过，评审结论：**Plugin Contract v0.1 正式冻结**（breaking → v0.2，additive → v0.1.x）。下一阶段是 SDK / Developer Experience，Python SDK 是跨语言契约的最佳验证器。

## Decision

1. **宣布 Contract v0.1 Frozen**。状态写入 `PLUGIN-CONTRACT-v0.1.md`；后续变更遵循 INV-009/010 与 §24 兼容政策。
2. **Python 脚本运行时（additive）**：`runtime.type = "python"`，`executable` 为插件目录内脚本路径（confinement 不变）；Host 以 `interpreter script [args]` spawn。
3. **解释器路径解析（用户可自定义 + 环境变量）**，优先级：
   1. `config.toml` → `python_path`（支持 `%VAR%` / `${VAR}` / `$VAR`）
   2. 环境变量 `LAUNCHER_PYTHON`
   3. PATH 上的 `python`
   宿主在 `launcher_plugin_host::expand_env` 统一展开（未知变量原样保留，spawn 报真实错误）。`launcher-app` 在注册 Python 插件时注入（`PluginProvider::set_interpreter`）。
4. **Python SDK v0.1**：`plugins/python/launcher_plugin.py`（单文件、纯标准库）。插件作者只写 `Plugin.query(text) -> list[Command]`；SDK 处理 initialize/version 协商/query_id 回显/shutdown/错误封装，并把 `print()` 重定向到 stderr（INV-019 的 DX 化）。
5. **跨语言 Conformance**：新增 `launcher-plugin-host/tests/python_sdk_e2e.rs`——真实 PluginHost 驱动 Python calculator（`plugins/python/example-calculator`），验证握手/回显/优雅关闭；解释器缺失时跳过并提示 `LAUNCHER_PYTHON`。Rust calculator-plugin 继续担任 Canonical Reference；Test Kit 命名与合并门禁写入 AGENTS.md。
6. **文档语义补充**：shutdown 丢弃 in-flight query 的迟到结果（§29.15）；资源 quota（memory/CPU/rate）列入 v0.2+ roadmap（§29.16），本期不引入 Job Object quota。

## Consequences

- 插件作者现在可以用 Python 写出与 Rust 完全同构的插件（同样的 manifest、同样的 conformance）而不知道 JSON-RPC 存在——契约抽象得到跨语言验证。
- Python 解释器不在 PATH 的用户通过 `python_path`/`LAUNCHER_PYTHON` 显式指定；两者都缺失且 PATH 无 `python` 时，Python 插件 spawn 失败但仅产生该插件的空结果（INV-011 风格的局部失败）。
