# MCP-CONTENT-MAPPING — MCP `content` 数组到 RichBlock 的映射关系

> 状态：CURRENT。定义 MCP `tools/call` 响应的 `content` 数组如何映射
> 为 RichBlock（RICH-RESULT-v1），使 MCP 工具的输出可在详情面板中
> 富渲染。

## 映射表

| MCP content type | RichBlock | 备注 |
|---|---|---|
| `text` | `Text { text }` | 直映射 |
| `image` | —（后置 P4） | 需资源解析/缓存管线 |
| `resource` | —（后置） | 需 resource 管线 |
| `audio` | —（不支持） | |
| embedded `resource` | —（后置） | |

## 规则

1. 多个 `text` 块合并为多个 `Text` RichBlock（保持顺序）。
2. 空数组 / 无 content → 无富渲染（降级为普通 title/subtitle）。
3. MCP `isError: true` → Rich 内容不渲染，错误文本走 status 路径。

## 实现状态

**DEFERRED**（P3-UI.1 尾款后置项）——映射表已冻结，实现在
`launcher-mcp` 的 result → Command 转换路径中落地。
