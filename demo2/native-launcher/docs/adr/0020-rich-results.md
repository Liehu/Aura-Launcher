# ADR-0020 — Rich Results（插件富结果 v1）

日期：2026-09-10 ｜ 状态：ACCEPTED

## 背景

插件结果目前只能呈现 title/subtitle/icon + 动作。Wox/uTools/Raycast 的
核心插件体验包含富内容。`RICH-RESULT-draft.md`（P3.0-F10）已给出草案。

## 决策

1. **协议**：`PluginResultItem.rich`（可选 JSON 值，加法性 serde 字段），
   形状 = `RICH-RESULT-v1` 契约的四种只读 block（Text/KeyValue/Table/
   Divider；Image 后置 P4）。
2. **传输**：进程内有界注册表（cmd_id → RichResult，≤512 FIFO），
   `Command` 类型**保持冻结**——rich 不进 Command 结构体（避免 100+ 处
   字面量改造），详情面板按选中 cmd_id 查询渲染。
3. **宽容解析**：未知 block 跳过；任一校验失败只丢弃 rich 部分，条目与
   普通搜索结果照常。
4. **安全**：intake 时逐串清洗（控制字符/定长）；总量 ≤256K 字符；
   渲染为纯文本投影，无语义解析。
5. **呈现**：Tab 详情面板（P3.0-F14）渲染 `render_lines()` 纯文本行；
   不新增 UI Mode。

## 后果

- 正面：插件表达力升级的最小路径；契约/实现/测试全链落地（P3.2-B0/B1）；
- 负面：注册表为进程内状态（重启即清——与插件按需拉起的生命周期一致）；
- 中性：Image/交互表格后置 P4。
