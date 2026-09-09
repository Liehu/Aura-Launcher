# 137 — P2.7 Batch 11：Provider Capabilities + Health Check（A01）

> 日期：2026-09-10。范围：P2.7 第十一批（`P2.7 开发设计规范` §28-§29）。
> 输入基线：history/135（800 tests；P2.9 Batch 6 并行至 804）。

## 交付

- `crates/launcher-ai/src/provider_caps.rs`（新）：
  - **§29 ModelCapability**：StructuredJson / Streaming / LongContext /
    LocalInference（serde 稳定）；
  - **ProviderProfile**：provider 身份 + capability 集合（宿主上报——
    模型自述数据视为 untrusted，不含任何授权语义）；
  - **health_check()**：对 provider 发送平凡请求的确定性探针（一次、
    不重试——重试策略归调用方）。
- 测试 2 条：mock provider 健康通过、capability 匹配（含否定断言）。

## Gate 结果

- `cargo test --workspace`：**806 passed / 0 failed**（804 → 806，+2）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 全局进度

| 阶段 | 进度 |
|---|---|
| 1.0 GA / P2.4 / P2.5 / P2.8 | ✅ |
| P2.6 | ✅ 完成（E02-E05 画布 VIEW 推迟） |
| P2.7 | 8/10 批（A01 ✅ 本批；剩 B01-B05、C03-C07 宿主接线、D/E/F/G/H） |
| P2.9 | 5/6 批组（Windows Adapter 接线批次） |
