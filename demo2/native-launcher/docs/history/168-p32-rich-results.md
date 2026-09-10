# 168 — P3.2 Batch B0/B1：Rich Results v1（协议冻结 + 渲染落地）

> 日期：2026-09-10。范围：P3.2 前两批合并推进（用户确认的 v1 取舍：
> 四 block 静态渲染、Image 后置、渲染落点 = Tab 详情面板）。
> 输入基线：history/167（910 tests）。

## 交付

- **契约冻结**：`docs/contracts/RICH-RESULT-v1.md`（原 DRAFT 转正）——
  四种只读 block、五条红线（全部带实现锚点）、传输/呈现/后置项；
  **ADR-0020** 记录三项关键决策。
- **类型与校验（P3.2-B0）**：`crates/launcher-domain/src/rich.rs`——
  `RichBlock`（Text/KeyValue/Table/Divider）+ `RichResult::from_value`
  （宽容解析：未知 block 跳过、校验失败仅丢 rich 部分且条目保留）+
  `validate`（≤64 块 / 表 ≤200 行 / ≤256K 字符）+ 逐串清洗（控制字符
  剥离 + 定长截断，intake 即脱敏）。
- **通道（P3.2-B0）**：`PluginResultItem.rich`（可选透传值）→
  plugin-host 校验后存入进程内有界注册表（`rich::store/lookup/remove`，
  cmd_id 键、≤512 FIFO）——**`Command` 类型保持冻结**（评审确认的
  100+ 处字面量改造成本规避方案，pattern 先例：P3.1 PIN_TABLE）。
- **渲染（P3.2-B1）**：管理选中变化时 `rich::lookup(cmd_id)` →
  `render_lines()` 纯文本投影 → 详情面板 `detail-lines` 逐行渲染；
  Tab 面板从骨架升级为真正的富内容呈现。
- **端到端测试**：合规 Python 插件返回 rich payload → plugin-host 校验
  存储 → `rich::lookup` 命中且 render_lines 含预期文本（LAUNCHER_PYTHON
  门控，同 crash-soak 惯例）；domain 单测 5 条（解析/未知块跳过/边界/
  清洗/注册表淘汰）。

## Gate 结果

- `cargo test --workspace`：**916 passed / 0 failed**（910 → 916，+6）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（P3.2-B2）

MCP `content` 数组映射表 + 管理窗口 General 页新增"富结果"说明 +
`docs/PLUGIN-DEV-GUIDE.md` §5.1 增补 rich 输出示例（当前已标注 DRAFT
转正状态，需同步实现示例）。
