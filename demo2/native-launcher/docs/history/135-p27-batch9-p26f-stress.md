# 135 — P2.7 Batch 9：Pipeline 风险等级收敛 + B06 集成测试 + P2.6-F Durable Stress

> 日期：2026-09-10。范围：P2.7 Batch 9 收尾 + P2.6-F durable stress/recovery
> 测试。输入基线：history/134（797 tests）。

## 交付

- **pipeline.rs 风险等级收敛修复**：PipelineOutcome::Proposal 的 risks
  向量现在与 plan 步骤一一对应且经风险分类（L0-L4），高风险步骤自动
  置 requires_approval。
- **P2.6-F durable stress/recovery 测试**（launcher-workflow/tests/
  durable_stress.rs）：
  - concurrent_runs_share_store_safely：两个 run 并发写同一 runs.db，
    互不干扰且双 finished；
  - paused_run_resumes_after_store_reopen：pause 后重开 store，恢复时
    已完成节点不重执行；
  - stress_sweep_20_runs_all_complete：20 runs 全部完成且 checkpoint
    齐全。
- 修复了测试的三处误用（entry 节点 ID、Exec delay 缺失、重复 import），
  实现语义未动。

## Gate 结果

- `cargo test --workspace`：**800 passed / 0 failed**（797 → 800，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 全阶段剩余

P2.7：A01（Provider 深化，llm.rs 已有 mock/OpenAI）、B01-B05、
C03-C07（宿主接线——pipeline.rs 已备编排核心）、D/E/F/G/H 线。
P2.6：E02-E05 的 Slint surface 接线（宿主投影层已就绪）。
P2.9：Windows Adapter（消费已冻结的分类/命令/策略）。
跨阶段尾项：Pinyin（A04/C04）、MSIX 签名（等证书）。
