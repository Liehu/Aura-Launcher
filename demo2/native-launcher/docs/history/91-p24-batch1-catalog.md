# 91 — P2.4 Batch 1: Application Catalog 2.0（A01–A05）+ Plugin Control Plane C02/C03

> 日期：2026-09-08。范围：P2.4（`demo2/files2/01-P2.4-DESIGN-SPEC.md`）第一批。
> 输入基线：Launcher 1.0 GA（history/90，commit 4d41324）。
> 执行记录格式遵循 `02-P2.4-AGENTIC-CODING.md` §3。

## Task P24-A01 — Application Identity v1

- `crates/launcher-providers/src/app_identity.rs`（新）：
  `ApplicationObservation`（spec §4.2 全字段）+
  `canonical_identity()` 纯函数（spec §4.3 五级 precedence：
  pkg → aumid → win32 exe → apppath → source-scoped fallback）。
  无 IO、确定性（INV-IDENTITY-003）。
- 测试 7 条：同 exe 跨源归一 / 不同 exe 不同身份 / packaged 优先且大小写稳定 /
  portable 走 win32 层 / fallback 源隔离不跨应用碰撞 / AUMID 优先级 / 确定性。

## Task P24-A02 — Catalog Schema v2

- `crates/launcher-providers/src/catalog.rs`：`CURRENT_SCHEMA_VERSION = 2`。
  v2 新列：`lifecycle`（ready/stale/broken）、`sources`（JSON provenance）、
  `aumid`、`package_identity`、`entry_path`（**stable-id 关键**：保存原始发现
  入口路径如 .lnk，保证 command_id 与 history/favorites join 不变）。
- 迁移：PRAGMA 探测 v1（无 lifecycle 列）→ 幂等 ALTER + backfill
  （sources←source，entry_path←launch_path）。真机验证：catalog.db
  generation=68、485 条全部保留。
- fresh 库直接建 v2 形状。

## Task P24-A03 — Catalog Reconciliation

- `reconcile_observations(&[ApplicationObservation])`：规范化+合并在事务外
  纯计算（identity 排序 → 同 identity 合并 sources），单事务 DELETE+INSERT，
  generation 仅 commit 后 +1。
- `reconcile()`（v1 元组形）保留为包装层（恢复测试/诊断用）。
- 测试：多源同身份合并且顺序无关（交换输入序结果相等）/ 缺失删除+元数据更新 /
  失败 reconcile（EXCLUSIVE 锁）保持前代可查。

## Task P24-A04 — Catalog Lifecycle

- `CatalogLifecycle`（Ready/Stale/Broken）+ `set_lifecycle()`。
- 语义：re-observation 重置 ready（新证据）；stale/broken 是元数据，
  Provider 层直接排除出候选集（见 A05 测试）——"metadata != authority"。

## Task P24-A05 — CatalogProvider（权威读路径）

- `AppRegistryProvider::from_catalog_records()`：provider 从**已提交的
  catalog**构建，搜索路径零同步发现。stable-id 规则：`path` = entry_path。
- `Core::set_application_generation(gen)`：cache 的 application_generation
  改为采用已提交的 CatalogGeneration（此前是启动时盲 bump、与 catalog 代际脱钩）。
- `build_core` 重写：发现 → observations → reconcile → list() 读回 →
  provider；catalog 读回失败/为空时回退 `from_entries`（发现结果在手，
  catalog 故障不拖垮应用搜索——INV 单点失败隔离）。
- 测试：from_catalog_records 合并/排除非 ready/id 稳定。

## Task P24-C02/C03 — Trust State + Capability Decisions

- `crates/launcher-core/src/providers/plugin_registry.rs`：
  - `TrustState`（Unknown/Known/Trusted）+ `trust` 列（PRAGMA 迁移，存量
    插件一律 unknown——**迁移永不自动信任**）。
  - `note_manifest_observed()`：只能 Unknown→Known；永不降级 Trusted。
  - `set_trust()`：唯一 Trusted 通道（host/admin 显式操作）。
  - `CapDecision`（Unset/Allow/Deny）+ `capability_decision()` /
    `set_capability_decision()`：唯一决策写入方；manifest request 记录
    （`record_capability`）永远保持 unset → enforcement 点 fail-closed。
  - 所有 trust/decision 变更走 backup 刷新（89 号的恢复语义自动覆盖新状态）。
- 测试：trust 阶梯不自动信任 / capability unset≠allow、持久化、deny 跨 reopen。

## Contract / Architecture impact

- 无冻结契约变更（Plugin/Command/Action/Workflow/UI 均未动）。
- catalog 身份语义升级（v1 win32-拼接 → v2 canonical identity precedence）按
  spec §10 属"Catalog identity semantics"ADR 范畴——本文件即该决定的记录：
  身份五级 precedence 冻结于 `app_identity::canonical_identity`。
- 行为可见差异：应用命令 id 不变（entry_path 规则）；search identity target
  不变；packaged/shell: 路径条目的 catalog 身份从 `win32:shell:…` 变为
  `src:packaged:…`（更正确：shell: 非文件系统身份）。

## Performance Impact

- 启动：多一次 list() 读回（485 行 SQLite 读 ≈1ms 级），reconcile 纯计算
  排序 O(n log n)（n≈500），无可测差异。
- 搜索：provider 内存快照查询路径不变；p95 7µs 基线不受影响。
- Memory：无新增常驻结构（records 构建后即释放）。

## Gate 结果

- `cargo test --workspace`：**638 passed / 0 failed**（623 → 638，+15）
- `cargo build --workspace`：零警告；`check_topology.py`：ok
- 真机冒烟（VR snapshot 模式）：v1→v2 迁移 ✅、generation 68 保留 ✅、
  485 条 catalog 读回 ✅、`catalog_authoritative=true` ✅、VR 15/15 ✅

## P2.4 剩余（下一批）

P2.4-A06（catalog golden 恢复）、P2.4-B 全部（index root 状态机/watcher 重注册/
DirtyRoot/健康模型/soak）、P2.4-C01/C04/C05/C06（lifecycle 模型整合/诊断；
C05 registry 恢复已在 history/89 完成）、P2.4-D/E（SDK CLI/诊断）、P2.4-F。
