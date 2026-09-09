# 113 — P2.8 Batch 3：事务化 Install（§19/§20/§23）

> 日期：2026-09-09。范围：P2.8 第三批（`P2.8 — Ecosystem & Distribution
> 1.0 技术设计规范.md` §16-§20/§23）。输入基线：history/112（740 tests）。

## 交付

- `apps/launcher-plugin-cli/src/transactional.rs`（新）：
  `execute_plan(&InstallPlan, &dyn PackageInstaller)`——按 plan 顺序对每个
  包 **stage → activate**（§19/§20 原子激活）；任一步失败则**逆序回滚全部
  已激活包**（§23），错误信息包含失败步骤 + 完整回滚日志（回滚失败也被
  报告，绝不静默）。
- `PackageInstaller` trait 使执行器完全可进程内测试；生产绑定是 CLI 的
  staged 目录安装 + rename swap（P2.4-D）。
- 测试 3 条：顺序执行、**中途失败逆序回滚且 active 清零**、回滚失败
  可报告。

## Gate 结果

- `cargo test --workspace`：**743 passed / 0 failed**（740 → 743，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.8 剩余

Batch 4（Signature 接口 + Trust 评估 + 生命周期宿主接线）→ Batch 5
（Repository/Index/UI）→ Batch 6（集成/QA/Gate）。
