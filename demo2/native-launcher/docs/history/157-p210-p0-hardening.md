# 157 — P2.10 Batch 1：P0 Hardening（Effect Authority / 动态目标 / 执行语义 / Replan 安全）

> 日期：2026-09-10。范围：P2.10 第一批——落实 102 号评审与
> `P2.10 — Cross-System Consistency & Security Hardening 1.0` 的全部
> P0 项 + P1 Replan 安全。输入基线：history/156（872 tests）。

## 评审结论（对两份文档的采纳判定）

**采纳并本批实现（值得做）**：P0×3（confirmed 布尔授权消灭、PID reuse
防护、Timeout/Unknown 统一语义）+ P1 Replan 不重执行成功步骤 +
EFFECT-AUTHORITY / EXECUTION-SEMANTICS 两份契约。

**暂缓并记录理由（不知道/不建议现在做）**：ContextAccessPolicy（E 线
privacy 已有 local-first 门与 sanitize，扩展接口后置）、Telemetry/Audit/
Diagnostics 三模型拆分（当前三套数据尚未互相污染，先立语义定义）、
docs/ 目录四级权威重构（history 已不可回写，README 索引化后置 P3）、
IntentConfidence 扩展口（规则优先是 G05 验证过的安全模型，产品化再
评估）、P2.8 更名（纯语义，随 P3 基线冻结一起做）。

## 交付

- **H1 Effect Authority（P210-003，§6/§7）**：
  - `launcher_action::authorize_system_command(cmd, confirmed)` →
    `SystemAuthorization`（私有字段 token，跨 crate 不可构造）→
    `system_adapter::execute_authorized(auth)`；
  - **adapter 不再接收 `confirmed: bool`**——确认策略在 engine 门内
    应用（校验 + requires_confirmation），旧 API 彻底删除，不存在
    (command, bool) → OS effect 的公开路径；
  - 新增 `ActionError::StaleTarget`。
- **H2 Dynamic Target（P210-006，§10/§11）**：
  - `SystemTarget::Process` 增加 `creation_time_ms`（resolve 时记录的
    进程创建时间）、`SystemTarget::Window` 增加 `pid`（属主）；
    serde default 保持冻结契约向后兼容；构造器新增
    `*_with_identity` 变体；
  - adapter 在 effect 前重验：kill 前比对
    GetProcessTimes 创建时间（PID reuse = StaleTarget）；窗口操作前
    IsWindow + GetWindowThreadProcessId 校验（HWND reuse = StaleTarget）。
- **H4 Execution Semantics（P210-002/005，§17-§20）**：
  - `launcher_domain::execution_semantics`：`CommandResult` /
    `EffectState` / `StepStatus` / `StepExecutionRecord` /
    `retry_decision` / `replan_may_execute` / `timeout_outcome`——
    Timeout ≠ Failed、Unknown+非幂等禁自动重试、Succeeded 不可变。
- **H3 Replan Safety（P210-007，§13-§16）**：
  - `agent_loop` 执行循环维护 per-step `StepStatus`：**Succeeded 步骤
    不可变，replan 重入时直接跳过**——语义从"全 plan 从头重执行"改为
    "从失败点继续"，消除了重复 Effect 风险；
  - 测试证明：a✓ b✗(transient) → replan → a 的 StepStarted 恰 1 次
    （旧语义会是 2 次）。
- **H6 契约文档**：`docs/contracts/EFFECT-AUTHORITY.md`、
  `docs/contracts/EXECUTION-SEMANTICS-v1.md`（权威链 + 绕过矩阵 +
  实现锚点/测试映射）；`docs/INVARIANTS.md` 新增 P210 节
  （INV-EFFECT-101..103 / INV-IDENTITY-101 / INV-AGENT-101 /
  INV-STATE-101，全部带锚点与测试）。
- 测试 10 条：gate 拒绝未确认、PID reuse 检测、死 HWND StaleTarget、
  语义矩阵 4 条、replan 跳过成功步骤、序列化往返等。

## Gate 结果

- `cargo test --workspace`：**880 passed / 0 failed**（872 → 880，+8）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## Hard Gate 自查（spec §35）

| 项 | 状态 |
|---|---|
| Effect bypass | 无公开旁路（token 门）✅ |
| PID reuse vulnerability | 已修（creation time 校验）✅ |
| HWND reuse vulnerability | 已修（liveness + 属主 pid 校验）✅ |
| Replan duplicate Effect | 已修（Succeeded 不可变）✅ |
| Timeout treated as failure | 已修（Timeout/Unknown 语义）✅ |
| Unknown blindly retried | 已修（retry_decision）✅ |
| cross-phase state contradiction | 语义层已立（execution_semantics）✅ |
| contract/status contradiction | 契约文档落地，目录重构后置 P3 ◐ |

## 待续（P2.10 Batch 2 候选）

1. 跨阶段 E2E 黄金路径（spec §33 三条）落为集成测试
2. Telemetry/Audit/Diagnostics 语义定义文档化
3. docs/ 四级权威目录重构 + Phase Status 四态化（P3 前置）
