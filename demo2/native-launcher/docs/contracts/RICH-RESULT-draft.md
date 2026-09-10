# RICH-RESULT-draft（P3.0-F10 协议草案 — DRAFT，非 FROZEN）

> 状态：DRAFT（评审中）。本文件只提出协议形状，不修改任何 FROZEN 契约；
> 实现随 P3.x Marketplace 批次。遵循 UI 纯投影红线（INV-035~037）。

# 1. 动机

当前插件/MCP 结果只能呈现 Command（title/subtitle/icon + 动作）。
Raycast/uTools 类工具的核心体验之一是插件返回**富内容**（表格、图片、
表单）。本草案定义"只读富结果"的最小协议：数据驱动、无脚本、无
Authority。

# 2. 形状（ActionContract 增补提案）

```text
ActionPayload::Rich {
    blocks: Vec<RichBlock>
}

RichBlock（只读，v1 五种）:
    Text      { text, emphasis: none|strong|code }
    KeyValue  { rows: Vec<(key, value)> }
    Table     { headers: Vec<string>, rows: Vec<Vec<string>> }   // ≤200 行
    Image     { png_png_base64? — 否：resource_id，宿主经既有 icon 管线解析 }
    Divider
```

# 3. 红线

1. Rich 内容是 **DATA**：不承载 Capability/授权/确认（任何动作仍走
   ActionPanel 动作 + 权威链）。
2. 渲染器**不解析语义**：未知 block 类型 → 跳过该 block（向前兼容）。
3. 尺寸预算：单结果 ≤256KB（与插件帧上限一致）；块数 ≤64。
4. 来源标识：Rich 结果必须携带 provider_id（Host 权威），审计随行。
5. 脱敏：进入 Rich 前内容必须过 E 线 sanitize（privacy::sanitize_untrusted
   同源规则）；禁止嵌入凭据/token（P210 NEVER_SENT）。

# 4. 呈现

Main 模式选中 Rich 结果时，Tab 详情面板（P3.0-F14 已有骨架）渲染
blocks；列表行仍显示 title/subtitle。

# 5. 开放问题（评审输入）

- Image resource 的解析边界（icon 管线 vs 独立缓存）
- 表格排序/滚动交互是否进 v1
- 与 MCP `content` 数组的映射表
