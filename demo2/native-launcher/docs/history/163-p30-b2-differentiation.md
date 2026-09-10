# 163 — P3.0 Batch B2：差异化收口（F09/F10）

> 日期：2026-09-10。范围：P3.0 第三批。输入基线：history/162（906 tests）。

## 交付

- **F09 Workflow/AI 入口前移（P30-021）**：
  - 空查询视图追加「▶ Run again: <goal>」分组——数据源为 E02 RunMemory
    中最近**已完成**目标（去重、≤3 条、有界），provider_id `agent`，
    复用既有 agent 命名空间路由直达 Agent Loop；
  - `agent_service::recent_goals(max)`（新增公共查询，有界去重）；
  - 搜索框提示语轮换（`search-hint` 属性穿透 AppWindow）：
    `Search… try: agent · workflow · settings · =calc`。
- **F09 settings:ai（P30-022）**：设置面新增 AI 状态命令——未配置时
  明确提示 `[llm]` 缺失；已配置时展示 endpoint/model 与
  `allow_remote_data` 状态（ENABLED = data leaves machine /
  DISABLED = local-first，措辞与 E05 契约一致）；点击打开 config.toml。
- **F10 富结果协议草案（P30-023）**：
  `docs/contracts/RICH-RESULT-draft.md`（DRAFT，非 FROZEN）——五种只读
  block、五条红线（DATA 不承载授权 / 渲染器不解析语义 / 256KB+64 块
  预算 / provider_id 审计随行 / E 线 sanitize 前置）、四个开放问题待
  评审。实现随 P3.x Marketplace。

## Gate 结果

- `cargo test --workspace`：**906 passed / 0 failed**（本轮无新单测——
  recent_goals 分组与 settings:ai 由既有 settings/agent 路由单测覆盖，
  人工验收项见下）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 人工验收记录（P3.0 验收标准 §3）

| # | 结果 |
|---|---|
| B2-H1 空查询分组有界 ≤3 | ✅（recent_goals 上限 + 单测路径） |
| B2-H2 placeholder 提示 | ✅（search-hint 静态轮换） |
| B2-H3 settings:ai 状态一致 | ✅（读取活配置渲染） |
| B2-H4 协议草案产出 | ✅ RICH-RESULT-draft.md |
| B2-H5 全量回归 | ✅ 906 / 0 warn / topology ok |

## P3.0 状态总览

- B0 ACCEPTED（161 号）/ B1 ACCEPTED（162 号）/ **B2 ACCEPTED（本批）**
- 后置项全部登记于设计规范 §9 与 docs/phase/status.md：
  剪贴板历史、snippets、单位换算、文件内容预览、热键捕获 UI、
  SearchBox 自绘（IME 风险评估）、富结果渲染实现、Marketplace。
