# 114 — P2.8 Batch 4：Trust Model + 生命周期管理器（§8/§9/§35）

> 日期：2026-09-09。范围：P2.8 第四批（`P2.8 — Ecosystem & Distribution
> 1.0 技术设计规范.md` §8/§9/§35）。输入基线：history/113（743 tests）。

## 交付

- `apps/launcher-plugin-cli/src/trust.rs`（新）：
  - **§8 Trust 评估矩阵（fail-closed）**：`evaluate_trust(SignatureStatus,
    PackageSource) -> TrustDecision`——仅 **Signed + OfficialRepository =
    Trusted**；Signed sideload 与 ChecksumVerified 官方包 =
    RequiresExplicitApproval（可用但用户必须看到批准内容）；Unsigned 一律
    **Untrusted**（默认策略拒绝安装）。决策是 DATA，最终闸门仍是
    Resolver/Policy。
  - **§9 签名接口**：`SignatureStatus` 枚举即 GA-6 外部证书接口——真实
    证书校验落在枚举之后，模型不变。
  - **§35 LifecycleManager**：所有生命周期变更强制过 `transition_allowed`
    白名单——Broken 永不自动回到 Enabled（repair 路径必须经
    Installed→Enabled），Uninstalled 不能跳过安装。
- 测试 2 条（覆盖 5 格矩阵 + 全非法迁移路径）。

## Gate 结果

- `cargo test --workspace`：**745 passed / 0 failed**（743 → 745，+2）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.8 剩余

Batch 5（Repository/Index/Marketplace 搜索 + 管理 UI）→ Batch 6
（集成/Audit/QA/Gate 收口）。
