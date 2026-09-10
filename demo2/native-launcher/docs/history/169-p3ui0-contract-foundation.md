# 169 — P3-UI.0 Contract Foundation：契约地基（A/B/C/E/F/G + H 部分）

> 日期：2026-09-10。范围：P3-UI.0 契约地基批（评审文件：files2/P3/
> 两份 P3-UI 文档）。输入基线：history/168（916 tests）。

## 评审结论（对两份文档）

- **范围收敛正确**：P3-UI.0 明确只做契约地基（类型/manifest/session/
  IPC 形状/校验/RichResult 对接/测试套件），不做 UI Host、Slint 组件、
  Tool Runtime——与 P2.10 后"先语义闭环再功能"的路径一致。
- **与现状对齐良好**：15 条不变量（P3UI-001~015）与既有权威链、
  INV-026~033、隐私模型同源；RichResult 直接复用 P3.2 的
  `launcher_domain::rich`。
- **两处偏差，已处理**：
  1. 允许区域列出的 `launcher-plugin-testkit` crate 尚不存在——本批以
     域内单测 + fixture 覆盖矩阵代替，独立 testkit crate 后置
     （避免 topology 基数变化混入契约批）；
  2. §18-23 的三张 SQLite 表（plugin_tools / tool_functions /
     plugin_ui_contracts）后置——会话与 UI generation 本就 ephemeral，
     元数据表随管理页批次一起落 plugins.db。

## 交付

- **P3UI-A 契约类型（P3UI-001~007）**：
  `crates/launcher-domain/src/tool.rs`——`ToolId`（校验构造：
  `[a-zA-Z0-9._-]+` ≤128）、`ToolFunctionId` / `ToolSessionId` /
  `UiNodeId` 别名、`ToolDefinition`（fail-closed 校验：schema_version
  ==1、entry_type=="interactive"、名称/描述界）、`ToolFunction`
  （input/output schema 为 object、purity 元数据）、`ToolReference`
  （Workflow 只存逻辑引用，P3UI-007）、`UiSchema` / `UiNode`（25 种
  冻结节点类型，未知 kind 拒绝）。
- **P3UI-E UI Schema 校验（P3UI-013）**：`UiSchema::validate`——节点数
  ≤256、深度 ≤16、文本 ≤8K、重复节点 id 拒绝（会话域内唯一）、未知
  kind 拒绝；**未知 optional 字段忽略**（serde flatten properties）。
- **P3UI-C 状态模型（§25-§27）**：`ToolSessionState` 十态 +
  `can_transition_to` 白名单（Closed 终态；Failed/Crashed/Timeout 不可
  复活）+ `UiGeneration`（accepted +1 / stale 拒绝 / closed 拒收）。
- **P3UI-B manifest 扩展**：`PluginManifest.tools: Vec<ToolDefinition>`
  （serde default 向后兼容）；manifest 校验**逐 tool fail-closed**——
  一个坏 tool 拒绝整份 manifest（确定性、有界、版本感知）。
- **P3UI-D IPC 词汇**：`launcher_ipc::tool_method`（tool.list/open/
  close/event/update/cancel）+ 六组 params/result 类型；§45 语义注释
  （base_generation 不匹配即拒绝，无隐式合并）。运行时分发随 Tool
  Runtime 批次接线。
- **P3UI-F RichResult 对接**：复用 P3.2 `launcher_domain::rich`
  （已实现注册表/校验/渲染）；本批确认其满足 P3UI-004 投影定位，
  无需改动。
- **P3UI-H 契约测试（部分）**：6 条域单测覆盖 spec §14 矩阵的
  valid / malformed / wrong version / unknown kind / duplicate id /
  bounds 分支；IPC 词汇由类型系统承载。
- **schemas/p3-ui/**（新，7 个 JSON Schema 文件）：与 §31-§40 一一
  对应（plugin-ui-extension / tool / tool-function / ui-schema /
  ui-node / ui-event / rich-result / tool-reference）。

## 后置（P3-UI.0 尾款，已登记）

- `launcher-plugin-testkit` 独立 crate（契约 fixture 复用层）
- plugins.db 三张元数据表 + 迁移
- plugin-host 的 tool.* 运行时分发（依赖 Tool Runtime 批次）
- Workflow 引擎消费 `ToolReference` 的执行接线

## Gate 结果

- `cargo test --workspace`：**922 passed / 0 failed**（916 → 922，+6）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
- 注：`p0c_scenario_a_no_credential_401`（example-mcp-server HTTP
  fixture）存在与本批无关的偶发端口竞态，单跑即过；已观察两次，
  建议下批排查 fixture 端口分配。
