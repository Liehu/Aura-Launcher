# 120 — P2.7 Batch 5：Prompt Builder + 注入清洗（A04/E06-lite + A02 上下文）

> 日期：2026-09-09。范围：P2.7 第五批（`P2.7 开发设计规范` §10/§12/§31）。
> 输入基线：history/119（767 tests）。

## 交付

- `crates/launcher-ai/src/prompt.rs` 扩展（A04/E06-lite/A02）：
  - `sanitize_untrusted()`：**E06-lite 注入清洗**——剥控制字符、中和伪造
    role 标记（`system:`/`user:`/`assistant:` 转义），文本保持可读但无法
    伪造指令结构；
  - `bound_chars()`：字符边界安全的有界截断（§12 上下文预算，不劈
    codepoint）；
  - `build_agent_prompt()`：Agent prompt 组装——GOAL（清洗+512 界）/
    QUERY/TARGET（256 界）/ **`<untrusted-context>` 围栏内的有界上下文**
    / 目录投影（复用 `format_catalog` 既有围栏）。确定性。
- 测试 3 条：注入中和、预算+Unicode 边界、组装确定性。
- 与既有防线叠加：catalog 围栏（LLM-SEC-009）+ 本批 role 标记中和 +
  预算截断 = Prompt Injection 纵深防御的第一、二层（E06 完整矩阵随
  G05 安全测试批推进）。

## Gate 结果

- `cargo test --workspace`：**770 passed / 0 failed**（767 → 770，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 剩余

A01（Provider 深化）、B 线 Planner（B03 校验器=structured_output 已备）、
C 线 Runtime 产品化、D/E/F/G/H 线。
