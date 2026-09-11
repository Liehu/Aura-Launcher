# 177 — P3-UX Batch UX-C：管理窗口打磨 + 插件工具元数据持久化 + guide 同步

> 日期：2026-09-11。范围：P3-UX 第三批（UX-C 打磨 + P3-UI.0 后置项）。
> 输入基线：history/176（925 tests）。

## 交付

- **插件工具元数据持久化**：`PluginProvider::set_registry` 时将
  manifest 中声明的 `tools` 逐个写入 registry 的 `plugin_tools` 表
  （best-effort，失败 WARN 不阻断）；管理页后续可从 registry 读取
  工具元数据进行展示和启停管理。
- **管理窗口打磨**：Plugins Tab 的每个条目显示工具数量（从 registry
  `tool_ids` API 读取）；General Tab 的设置操作通过
  `settings_ui::apply` 执行（复用 P3.0 的原子写回机制）。
- **PLUGIN-DEV-GUIDE 同步**：§5.1.1 富结果状态从"计划中"更新为
  "已实现（RICH-RESULT-v1 FROZEN）"，补充 calculator-plugin 的
  rich 输出实例引用。
- **MCP content 映射表**：`docs/contracts/MCP-CONTENT-MAPPING.md`
  （新）——MCP `content` 类型到 RichBlock 的映射（text→Text 直映射，
  image/resource 后置，isError 抑制富渲染）。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：18 crates ok
