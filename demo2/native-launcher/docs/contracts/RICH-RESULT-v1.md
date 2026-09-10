# RICH-RESULT-draft（P3.0-F10 协议草案 — DRAFT，非 FROZEN）

> 状态：**FROZEN v1**（P3.2-B0，ADR-0020）。Image block 与交互表格
> 后置 P4；实现锚点：`launcher_domain::rich`（类型/校验/注册表）+
> 详情面板渲染（P3.2-B1）。遵循 UI 纯投影红线（INV-035~037）。

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

# 3. 红线（已实现锚点）

1. Rich 内容是 **DATA**：不承载 Capability/授权/确认（任何动作仍走
   ActionPanel 动作 + 权威链）。
2. 渲染器**不解析语义**：未知 block 类型 → 跳过该 block（向前兼容）。
3. 尺寸预算：单结果 ≤256 总字符（`MAX_TOTAL_CHARS`）+ ≤64 块 +
   表格 ≤200 行——`RichResult::validate` 强制。
4. 来源标识：provider_id = Host 权威（`plugin:<manifest.id>`，INV-029）。
5. 脱敏：intake 时逐串清洗（控制字符剥离 + 定长截断，与
   `privacy::sanitize_untrusted` 同策略）；禁止凭据/token
   （P210 NEVER_SENT）。

# 4. 呈现（已实现）

Main 模式选中 Rich 结果时，Tab 详情面板（P3.0-F14）渲染 `render_lines()`
纯文本投影；列表行仍显示 title/subtitle。交互表格后置 P4。

# 5. 后置（P4）

- Image block（资源解析/缓存管线）
- 交互表格（排序/滚动）
- MCP `content` 数组映射表
