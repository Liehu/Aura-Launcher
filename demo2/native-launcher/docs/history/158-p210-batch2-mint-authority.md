# 158 — P2.10 Batch 2：铸造权唯一化 + 授权绑定 + 恢复协议（103 号评审落地）

> 日期：2026-09-10。范围：P2.10 第二批——落实 103 号评审的全部 P0/P1。
> 输入基线：history/157（880 tests）。

## 评审采纳判定

**立即修（P0）**：`authorize_system_command` 曾是 pub——任何 crate 可
`(cmd, confirmed=true)` 直接铸造 token。已收归 `pub(crate)`，外部唯一
公共入口是 `execute_system_effect`（先 Resolver policy 再铸造）。

**顺手修（P1）**：token 绑定命令 + 审计元数据、move-only 类型化一次性、
PID 身份升为 FILETIME 原始 100ns、`retry_decision(result, effect_state,
idempotent)` 三参化、Recovery Protocol（`recovery_route`）。

**暂缓（沿用上批判定 + 评审同意）**：同进程 HWND 生命周期身份
（WindowGeneration）后置——跨进程 reuse 已覆盖；三套数据模型拆分与
docs 目录重构按评审建议排 Batch 3。

## 交付

- **INV-EFFECT-104（P0）**：`authorize_system_command` 降为
  `pub(crate)`；新增公共入口
  `execute_system_effect(resolver, cmd, confirmed)` =
  Resolver policy（origin 允许表/风险上限/deny 表）→ 确认门 → 铸造 →
  adapter 全链一次完成。测试证明：deny 的 origin 在铸造前被拒，
  allow 的 origin 全链直达 adapter（StaleTarget 即链路证据）。
- **INV-EFFECT-105**：token 无 Clone/Copy，`execute_authorized(auth)`
  按值消费——一次性成为编译器约束（`token_is_move_only` 所有权断言）。
- **P210-C07**：token 携带完整被授权命令 + `SystemAuthorizationAudit`
  （origin/risk/policy_approved/confirmation_confirmed/minted_at_ms）；
  adapter 执行的就是 token 里的命令。
- **P210-B07**：`creation_time_ms` → `creation_time_ft`（FILETIME 原始
  100ns），身份精度不再降采样；比较逻辑同步升级。
- **P210-D07**：`retry_decision(result, effect_state, idempotent)` 三参
  化——`Cancelled + Unknown` 不再天然可重试，与 `Unknown` 同走幂等门；
  `Started`（在途）一律 Forbidden。
- **P210-D08**：`recovery_route(idempotent, queryable)` →
  `Queryable` / `PolicyRetry` / `ExplicitRecoveryRequired`——Unknown
  效果的显式恢复路由；EXECUTION-SEMANTICS-v1 增补 Recovery Protocol
  节与语义依赖图。
- 测试 6 条：公共入口 policy 前置、token move-only、retry 矩阵 v2、
  recovery 路由、（含既有 4 条 P210 测试迁移适配）。

## Gate 结果

- `cargo test --workspace`：**883 passed / 0 failed**（880 → 883，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## Hard Gate 状态（评审 §14 版本）

G01 Effect Authority ✅ / **G02 Mint Authority ✅（本批）** /
**G03 Command Binding ✅（本批）** / **G04 One-shot ✅（本批，编译器约束）** /
G05 PID Identity ✅（本批精度强化）/ G06 HWND Identity ✅（同进程 reuse
后置 Batch 3）/ G07 File Target Safety ✅ / G08 Replan ✅ /
**G09 Executing/Unknown Recovery ✅（路由语义本批；运行时探测钩子后置）** /
**G11 Cancelled/Unknown ✅（本批）** / G12 Retry ✅ / G10/G13–G20 ✅ 或随
Batch 3 文档批。

## 待续（Batch 3）

1. Unknown 效果的运行时 Queryable 探测钩子（当前只有路由语义）
2. Telemetry/Audit/Diagnostics 语义文档 + docs 目录四态化
3. 同进程 HWND 生命周期身份（WindowGeneration，P2）
