# 112 — P2.8 Batch 2：Dependency Resolver + InstallPlan

> 日期：2026-09-09。范围：P2.8 第二批（`P2.8 — Ecosystem & Distribution
> 1.0 技术设计规范.md` §14-§17）。输入基线：history/111（734 tests）。

## 交付

- `apps/launcher-plugin-cli/src/resolver.rs`（新）：
  - `PackageMeta`（repository index 条目：id/version/depends）；
  - `resolve(index, requested) -> InstallPlan`——DFS 依赖优先的**确定性
    拓扑排序**（同 index 同请求 → 同 plan），共享依赖只安装一次；
  - 失败精确可解释（§14）：`MissingDependency`（含缺失 id）/
    `DependencyCycle`（含链路）；
  - `InstallPlan` 是 DATA（§16）：serde roundtrip 供审批 UI 展示，
    从不自行安装。
- 测试 6 条：依赖先行、缺失解释、环检测、确定性、共享依赖单次安装、
  roundtrip。

## Gate 结果

- `cargo test --workspace`：**740 passed / 0 failed**（734 → 740，+6）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.8 剩余

Batch 3（事务化 Install：InstallPlan → staged 安装执行器）→ Batch 4
（Signature 接口 + Trust 评估 + 生命周期宿主接线）→ Batch 5（Repository/
Index/UI）→ Batch 6（集成/QA/Gate）。
