# 144 — P2.7 Batch 16：触发源 helpers（D03-D06）+ P2.5/P2.6 A04 Pinyin 首字母

> 日期：2026-09-10。范围：P2.7 D 线触发源 helpers + P2.5/P2.6 A04 Pinyin。
> 输入基线：history/142（820 tests）。

## P2.7 D03–D06 — 触发源 helpers

- `crates/launcher-workflow/src/trigger_sources.rs`（新）：
  `enqueue_hotkey/plugin/schedule/ai_mcp(queue, workflow_id, input)`——
  四类触发源的类型化构造器，全部入队到 TriggerQueue（P2.6-D02）。
  Plugin 源额外注入 `_plugin_source` 标记以便审计。
- 测试 1 条：四源顺序入队+出队，kind/workflow_id 断言。

## P2.6 A04 — Pinyin 首字母搜索

- `crates/launcher-domain/src/pinyin.rs`（新）：
  `pinyin_initials(name)`——对 CJK 字符串提取拼音首字母（BMP 范围→
  GB2312 排序近似边界映射，覆盖常用简体字）；非 CJK 字符跳过。
  确定性、无外部依赖。完整拼音表（含韵母/声调）为 2.x 项。
- 测试 3 条：确定性、混合 CJK+ASCII、非 CJK 产出空。

## Gate 结果

- `cargo test --workspace`：**824 passed / 0 failed**（820 → 824，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
