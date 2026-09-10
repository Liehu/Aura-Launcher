# 160 — P2.10 Batch 3 扫尾：HANDBOOK 全面索引化

> 日期：2026-09-10。范围：P2.10-011 Documentation Reconciliation 的最后
> 一项——PROJECT-HANDBOOK 从"状态承载者"改为"索引 + 导航"。输入基线：
> history/159（887 tests）。

## 交付

- **§7 里程碑进度索引化**：约 265 行逐批次状态事实（✅/⏳/⏸ 混排的
  ASCII 矩阵）替换为 8 行里程碑 → `docs/phase/status.md` 权威链接表。
  HANDBOOK 不再承载状态事实（§30：README/HANDBOOK 只做 Index/Navigation/
  Architecture Summary/Current Accepted Baseline）。
- **头部基线刷新**：748 tests（P2.8 Batch 5 时代）→ 887 tests / 零警告
  / topology 17+9，并指向 `docs/phase/status.md` 为阶段状态唯一真相。
- **§3 契约表补全**：P210 三份契约（EFFECT-AUTHORITY /
  EXECUTION-SEMANTICS-v1 / STATE-GENERATION）入冻结契约索引。
- **§5 Invariants 索引**：补 P210 节不变量组（INV-EFFECT-101~106、
  INV-IDENTITY-101~102、INV-RETRY-101、INV-RECOVERY-101、INV-AGENT-101、
  INV-STATE-101）。
- **§9 常用命令刷新**：测试计数与 gate 覆盖说明更新（AI 测试随
  `cargo test --workspace` 全量运行）。
- **§10 红线速查扩展**：新增系统 Effect 只能走 `execute_system_effect`
  全链（adapter 无 authority 参数、token move-only）、效果不确定性统一
  引用 EXECUTION-SEMANTICS-v1、阶段状态查 phase/status.md 三条。
- HANDBOOK 体积：452 → 204 行。

## Gate 结果

- `cargo test --workspace`：**887 passed / 0 failed**（纯文档批，不变）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.10 H6 终态

四级文档权威全部就位：`contracts/`（当前有效语义）→ `phase/status.md`
（阶段状态唯一真相）→ `history/`（不可回写证据）→ README/HANDBOOK
（索引）。**G20 Documentation = ACCEPTED。**
