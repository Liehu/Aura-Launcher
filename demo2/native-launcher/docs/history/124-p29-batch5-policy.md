# 124 — P2.9 Batch 5：System Policy + Origin 传播

> 日期：2026-09-09。范围：P2.9 第五批（`P2.9 — System Integration &
> Automation 1.0 技术设计规范.md` §32/§34）。输入基线：history/123
> （779 tests）。

## 交付

- `SystemPolicy`（§32）：`denied_operations`（既有）+ **origin 白名单**
  （非空时仅表内 origin 可解析）+ **origin 风险上限**（ceiling 只能降低
  该 origin 可请求的最大风险，永不提升）。
- `SystemResolver.resolve()` 管线升级：validate → deny-list → origin
  白名单 → **ceiling 检查** → risk/confirmation 判定；`Resolution` 新增
  **`origin` 传播字段**（§34：谁请求的一路带入审计轨迹）。
- Fail-closed：白名单外的 origin 直接拒绝（origin 仍传播到决策记录）；
  ceiling 只降不升。
- 测试：白名单外 origin 拒绝且 origin 传播、白名单内+ceiling 内放行、
  超 ceiling 拒绝。

## Gate 结果

- `cargo test --workspace`：**780 passed / 0 failed**（779 → 780，+1）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.9 剩余

Batch 6：Race/Security/Soak/Fault 全矩阵 + Gate 收口。Windows Adapter
统一后置接入。
