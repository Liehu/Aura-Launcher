# 159 — P2.10 Batch 3 + 收口：Recovery Probe / Cross-Phase E2E / State-Generation / 文档对齐

> 日期：2026-09-10。范围：P2.10 第三批（收口批）——Runtime Recovery
> Probe、Cross-Phase E2E、State/Generation Reconciliation、Documentation
> Reconciliation。输入基线：history/158（883 tests）。

## 交付

- **Runtime Recovery Probe（P210-G09 运行时闭环）**：
  - `system_adapter::probe_effect(cmd)`——Queryable 路由的探测实现：
    kill → 目标进程消失且有身份记录 = `Completed`、存活且身份匹配 =
    `NotStarted`、身份不匹配（pid reuse）= `Unknown`；window close →
    `IsWindow` + 属主 pid 同理；power/uri 等不可观测操作恒 `Unknown`；
  - `TurnExecutor::effect_state(action_ref)`（默认 `Failed`）：宿主执行器
    可超时/不可定论时上报 `Unknown`；
  - agent_loop 语义闭环：**Unknown 步骤不触发自动 replan**（进入
    Recovery 路由），且重入时与 Succeeded 一样跳过——"Executing/Unknown
    不得盲目重跑"从契约进入运行时；
  - 测试：probe 四象限（NotStarted/Unknown/Completed/非可观测）+
    Unknown 不重试（`path3`）。
- **Cross-Phase E2E（P210-G16，spec §33 三条黄金路径）**——
  `launcher-core/tests/p210_cross_phase_e2e.rs`：
  1. Search → AI Proposal → Approval → Plan Validation（B03 目录校验
     fail-closed 断言）→ Effect → Audit（§37 事件全弧）；
  2. Workflow-origin → System Target 变更（PID reuse / 死 HWND）→
     StaleTarget（policy 通过但身份不符，adapter 拒绝）；
  3. AI → Effect Timeout → Unknown → **不 replan 不重复执行**。
- **State/Generation Reconciliation（P210-004）**：
  `docs/contracts/STATE-GENERATION.md`——六类 Lifecycle State 分类矩阵
  （Intent/Planning/Authorization/Execution/Resource/Recovery）+ 四个
  generation（Catalog/Application/Context/Index）的注册表（version 什么
  / 何时 +1 / 失效什么 / 失败不递增）+ Generation ≠ Identity 与
  Recovery 接口。
- **Documentation Reconciliation（P210-011）**：
  - `docs/phase/status.md`（新）：四态词汇（ACCEPTED/IMPLEMENTED/
    DEFERRED/BLOCKED-EXTERNAL）的唯一阶段状态真相，覆盖 1.0–P2.10 全部
    阶段与后置项；P2.8 采用「Plugin Lifecycle & Local Ecosystem」定义；
  - docs/README 头部声明四级文档权威（contracts → phase → history →
    索引）；INVARIANTS/contract 映射在 157/158 号已建立。

## Gate 结果

- `cargo test --workspace`：**887 passed / 0 failed**（883 → 887，+4）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.10 Hard Gate（评审 §14 二十项）终态

G01–G17 全 PASS（含 G02 铸造唯一化、G03 命令绑定、G04 一次性（类型
强制）、G09 Recovery Probe 运行时闭环、G16 三条 E2E）；G18 Security
（Mimosa 常规扫描随 gate；deep 审计受 scanner 缓冲限制待重跑）；
G19 Performance（既有性能契约基线不变）；G20 Documentation（本批
INTEGRATED；HANDBOOK 索引化扫尾随 P3 启动批完成）。

**P2.10 = ACCEPTED（代码与契约域）。P3 可以启动。**
外部条件项（MSIX 签名、Marketplace 服务端）继续 BLOCKED-EXTERNAL，
不阻塞 P3（发布/分发域）。
