# 104 — P2.9 Batch 1：系统集成契约基座

> 日期：2026-09-09。范围：P2.9 第一批（`P2.9 — System Integration &
> Automation 1.0 技术设计规范.md` §4-§8/§31）。输入基线：history/103。

## 交付

- `crates/launcher-domain/src/system.rs`（新）：
  - `SystemCapability`（v0.1 taxonomy：Process/Window/File/Clipboard/
    Shell/Uri/Notification/Power/Settings）；
  - `SystemRisk` L1-L5 对齐 P2.7 RiskLevel 语义（Info/ReadOnly/Reversible/
    Destructive/Privileged），destructive/privileged 需确认（§35）；
  - `SystemTarget` 类型化身份（Process/Window/File/Uri/System——§8，
    **绝不是原始命令字符串**）；
  - `SystemCommand`（capability+operation+target+risk+origin，§34 origin
    供审计）+ `validate()` 确定性 fail-closed（operation 词法、origin、
    空 target）；
  - `SystemResolver` 骨架（§31）：决策记录 approved_by_policy/reason，
    **永不执行**；policy denied_operations 表；
  - `MockSystemProvider`（§30）：无 OS 交互的观测记录，测试/诊断用。
- 测试 4 条：resolve 允许/拒绝/非法 operation、destructive 需确认、
  roundtrip + origin 审计。

## 边界重申

五"≠"边界原样保持：Resolution 是决策记录，Effect 仍仅经
`validate → Policy → Approval → launcher-action`。Native API 零引入
（Adapter 在后续批次）。

## Gate 结果

- `cargo test --workspace`：**708 passed / 0 failed**（705 → 708，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.9 剩余

Batch 2 File/Clipboard Adapter（既有 Effect 收编）→ Batch 3 Window/
Process → Batch 4 Shell/URI/Notification/Hotkey/Power → Batch 5 Policy/
Confirmation/Origin 传播 → Batch 6 全矩阵 + Gate 收口。
