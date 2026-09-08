# 89 — RC1 Closure: Registry Backup/Restore + Update Handoff + MSIX Packaging + 30min Soak

> 日期：2026-09-08。范围：`docs/PROJECT-HANDBOOK.md` §7 P2.3-I "RC1 挂账（阻塞 GA）" 三项中可在仓内完成的全部代码/流程项。测试基线见文末。

## 1. Plugin Registry 安全选择恢复（backup/restore）

**动机**：`plugin_registry.rs` 的 C6 损坏自愈此前是"直接删库重建"，注释两处标记为
"P2.3-D.1 backup candidate"。损坏会让 disabled/quarantined 等用户安全选择静默
回到 permissive 默认——这是安全回归。

**实现**（`crates/launcher-core/src/providers/plugin_registry.rs`）：

- `PluginRegistry` 持有 `db_path`；每次 mutating 操作
  （`set_enabled` / `record_failure` / `record_success` / `record_capability`）
  后 best-effort 刷新 `plugins.db.bak`：先 `PRAGMA wal_checkpoint(TRUNCATE)`
  再经 `.tmp` + rename 原子替换（崩溃不会毁掉最后一份好备份）。行数少，拷贝成本可忽略。
- `open()` 恢复顺序升级为三层：
  1. 损坏（magic 检查失败或 open/schema 失败）→ 损坏文件 **quarantine**
     （rename 为 `plugins.db.corrupt-<ts>`，留证不删）→ 尝试从 `.bak` 恢复；
  2. 有可用备份 → 恢复成功，安全选择完整保留；
  3. 无备份（missing/empty）→ 原有 safe-default 重建。
- 失败计数器语义不变：恢复后的计数是"恢复时刻的真实状态"，不是新策略。

**新增测试**：`corrupt_db_restores_from_backup`（损坏后 disabled/failures 状态恢复）、
`corrupt_db_without_backup_rebuilds_and_quarantines`（fresh 默认 + corrupt 文件保留）。

**契约影响**：C12 Golden Suite 的 `golden_plugin_registry_all_modes` 场景按
backup-first 策略更新（garbage/truncated = 恢复保留计数 2；empty/missing = 重置 0）。
这是有意的契约演进（安全增强方向），记录于本文件为唯一权威说明。

## 2. Update Handoff / UpgradeCoordinator（消费端切片）

**动机**：P2.2-E §25/§26/§50 的 `update_handoff.json` 只有规格、零代码。

**实现**：

- `crates/launcher-config/src/handoff.rs`（新）：`UpdateHandoff`
  （transaction_id / target_version / reason / started_at_ms / resume，
  §26 元数据-only，不含 clipboard/query/payload/credentials/tokens——
  由测试 `serialization_is_metadata_only` 序列化断言钉死）；
  `write_handoff`（tmp+rename 原子写）、`read_handoff`（损坏文件丢弃不阻塞启动）、
  `consume_handoff`（consume-once，§50 WaitingForExit → recover handoff）。
- `apps/launcher-app/src/main.rs`：启动链在 degraded-boot 判定之后、
  `build_core` 之前消费 handoff 并打日志。是否认定升级成功由 startup health
  marker 决定（§51 不许猜状态），handoff 本身不改变启动决策。
- `launcher-config` 新增 `serde_json` 依赖（workspace 已有版本）。

**产出协议**：升级器侧（外部 MSIX 流程 / 人工升级）在退出前写
`<data_dir>/update_handoff.json` 即可；下一次启动自动消费。

**新增测试**：roundtrip / consume-once / corrupt 丢弃 / 元数据-only 序列化（4 条）。

## 3. MSIX 打包（无签名）

- `scripts/make_msix.py`（新）：staging zip 同内容 + `AppxManifest.xml`
  （FullTrust + runFullTrust rescap、x64、占位 Logo 资产）→ Windows SDK
  `MakeAppx.exe pack` → `artifacts/dist/NativeLauncher-<ver>-win64.msix`。
- **签名是 GA 外部步骤**（仓库无证书）：安装前需
  `signtool sign /fd SHA256 ...`。未接入 `release_gate.py`（G12 证据链不变）。

## 4. 30min 资源长剖面 soak（C11）

- `LAUNCHER_SOAK_SECONDS=1800 cargo test -p launcher-core --test c11_resource_soak`
  夜间档执行；结果日志 `benchmarks/c11-soak-30min-run.log`。
- **结果（2026-09-08）：PASS** — 111,500 cycles in 1800s |
  RSS 8.4→12.2 MB（+3.8，预算 ≤50MB）| threads 5→2 | handles 76→77。
  增长有界，无泄漏漂移。
- 4h 档（14400）仍保留为手动/nightly 项（不阻塞 GA，同规格原文）。

## 5. 本批未动（非欠账，2.x 路线图）

Provider Contract v2 / Catalog 增量 reconcile / P2.2-D 主体（Registry SQL、
Package、Capability UI）/ Streamable HTTP+OAuth / 多语言 SDK 矩阵 /
插件 empty-text discovery（DISCOVERY-TODO-001 插件侧）。

## 测试基线

- `cargo test --workspace`：**623 passed / 0 failed**（617 → 623，+6）
- `cargo build --workspace`：零警告
- `python scripts/check_topology.py`：ok（17 crates + 8 apps）
