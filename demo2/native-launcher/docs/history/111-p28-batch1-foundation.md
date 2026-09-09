# 111 — P2.8 Batch 1：Foundation（PluginIdentity / SHA-256 Integrity / 生命周期状态机）

> 日期：2026-09-09。范围：P2.8 第一批（`P2.8 — Ecosystem & Distribution
> 1.0 技术设计规范.md` §5/§7/§10/§35）。输入基线：history/104-110（705）。

## 交付

- `apps/launcher-plugin-cli/src/foundation.rs`（新）——P2.8 生态基座，与
  Batch 1 计划（history/102）一致：
  - **§5 PluginIdentity**：id/version/contract_version + 词法校验
    （与 CLI install 路径共享规则，路径形式 id 拒绝）。
  - **§10 Package Integrity**：自包含 SHA-256（无外部依赖，标准测试向量
    "" / "abc" / 长串全部通过）+ `verify_integrity` 常量时间比较——
    任何字节改动都是硬完整性失败。签名验证（§9）按 GA-6 裁决留接口，
    完整性先行。
  - **§35 生命周期状态机**：Uninstalled/Installed/Enabled/Disabled/Broken
    + `transition_allowed()` 白名单——**fail-closed：Broken 不得直接回到
    Enabled**（必须走 repair/reinstall），Uninstalled 不得直接 Enabled。
  - 枚举 serde roundtrip（持久化契约）。
- 测试 5 条：SHA-256 向量、完整性校验、identity 校验、非法迁移拒绝、
  roundtrip。

## Gate 结果

- `cargo test --workspace`：**734 passed / 0 failed**（729 → 734，+5）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.8 剩余

Batch 2（Package 工具 + Dependency Resolver）→ Batch 3（事务化
Install）→ Batch 4（Signature 接口 + Trust + 生命周期宿主接线）→
Batch 5（Repository/UI）→ Batch 6（集成/QA/Gate）。
