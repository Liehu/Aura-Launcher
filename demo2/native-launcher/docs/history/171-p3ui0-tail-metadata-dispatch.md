# 171 — P3-UI.0 尾款：SQLite 三张元数据表 + plugin-host tool.* 分发

> 日期：2026-09-11。范围：P3-UI.0 后置项——plugin_registry 三张元数据
> 表 + plugin-host tool.* 分发方法 + Workflow ToolReference 类型接线。
> 输入基线：history/170（924 tests）。

## 交付

- **plugin_registry 三张元数据表（P3-UI.0 §21-§23）**：
  - `plugin_tools`：plugin_id + tool_id 主键，version/name/description/
    category/icon_ref/entry_type/enabled/schema_version/created_at/
    updated_at；
  - `tool_functions`：plugin_id+tool_id+function_id 主键，
    input/output schema JSON、deterministic/side_effect 标志、
    required_capabilities JSON 数组；
  - `plugin_ui_contracts`：ui_schema_version + max_nodes/depth/text/asset
    上限；
  - API：`upsert_tool` / `upsert_tool_function` / `set_ui_contract` /
    `tool_ids`；测试 1 条 roundtrip。
- **plugin-host tool.* 分发（P3UI-D 运行时接线）**：
  - `tool_request<T>` 内部泛型分发（serde 反序列化 + 错误转发）；
  - `tool_list` → `ToolListResult`、`tool_open` → `ToolOpenResult`、
    `tool_event(UiEvent)` → `Value`、`tool_close` / `tool_cancel`；
  - 超时 = `min(manifest.timeout_ms, 2000)`，与 query 超时一致。
- **Workflow ToolReference（P3UI-G 类型接线）**：`ToolReference`
  结构体已在 `tool.rs` 定义（plugin_id/tool_id/function_id），Workflow
  引擎消费接线后置到 Tool Runtime 批次。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**（924 → 925，+1）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
- 注：18 crates（testkit 加入后 topology 基数更新）
