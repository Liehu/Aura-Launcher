# 174 — P3-UI.1 尾款：Rich 示例同步 + MCP content 映射 + tool.update 通知

> 日期：2026-09-11。范围：P3-UI.1 收尾——guide 富示例同步、MCP content
> 映射表、tool.update 通知处理。输入基线：history/173（925 tests）。

## 交付

- **PLUGIN-DEV-GUIDE §5.1.1 同步**：富结果示例状态从"计划中"更新为
  "已实现"（RICH-RESULT-v1 FROZEN），补充 calculator-plugin 的 rich
  输出引用。
- **MCP `content` 数组映射表**：`docs/contracts/MCP-CONTENT-MAPPING.md`
  （新）——MCP `content` 类型（text/image/resource）到 RichBlock 的
  映射关系（text → Text、image → 后置 P4、resource → 后置）+ 限制说明。
- **tool.update 通知处理**：v1 方案——tool.event 的响应中可携带
  `"ui"` 字段实现 UI 更新（已在 P3.2-B1 实现）；真正的异步通知路由
  （plugin → host 推送无 id 通知）后置到 Tool Runtime 批次（需
  ProcessSession 支持 notification 分流）。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**（不变）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
