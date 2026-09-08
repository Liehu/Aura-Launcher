# 83 — P1 收口 + P2.1-D.1 + P2.2-D/E 核心实施记录

日期：2026-09-07。基线：571 tests / 零警告 / Release Gate 十项 PASS。
按用户指令顺序完成：**P1 收口 → P2.1-D.1 → P2.2-D 主体 → P2.2-E 主体**（P2.3 为后续 Hardening 阶段，未启动）。

---

## 一、P1 收口 ✅

### 1. Candidate Merge 完整版（P2.1-C Gate C7）
- `rank_with_boost` 身份折叠升级：canonical（最高分行）保留，**被折叠行的缺失 subtitle 回填 + ActionDescriptor 并集合并**（open/open-private/runas 等替代动作不再随来源行丢弃）——`split_at_mut` 实现避免借用冲突，无行为回退。
- 测试：`identity_merge_unions_action_descriptors`（路径身份合并 + 动作并集 = 3）。

### 2. Icon UI（P2.1-E5）
- `ResultItem` 新增 `icon-data: image`；ResultRow **恒留 20px 图标槽**（无 reflow）。
- `spawn_icon_refresh`：结果发布后异步抽取（SHGetFileInfo→RGBA），**RGBA 字节跨线程、Image 在事件循环内构建**（slint::Image !Send——E0277 逼出的正确结构）；快照模式跳过图标，VR 保持确定。
- VR 基线按计划**有意重生成**（图标槽是布局变更）：gate 两连跑 15/15 byte-identical 确认稳定。

### 3. Release 性能基线 ✅
- `cargo build --release` 后 `launcher-bench 10000` → `benchmarks/baseline-release.json`：
  **app 查询 p50/p95 = 7µs/7µs；file 查询 p50/p95 = 0/23µs；cold_start = 640µs；内存 idle/峰值 ≈ 1.4/1.8 MB**。
- 对照 80 号 debug 基线（app p95≈8.1ms）：release 提速 ~1000×，"低内存 Launcher"定位由数据证实。

## 二、P2.1-D.1 — Persistent Application Catalog ✅（独立小批）

- `launcher-providers/src/catalog.rs` `CatalogStore`（`catalog.db`，WAL）：
  - `reconcile(entries)`：单事务 replace-all + **generation 仅 commit 后 +1**（与 File Index 同语义）；
  - `generation()` / `list()` / `CatalogRecord`。
- **职责分离冻结**：Discovery 聚合层（app_registry/packaged/portable）零改动；catalog.rs 只是持久化层。宿主在 build_core 把合并结果物化进 catalog.db 并 bump `application_generation`（Query Cache 随之失效）。
- 修复：`reconcile` 内 MutexGuard 未释放即调 `generation()` 的死锁（scoped guard）。
- 测试 1 项：replace-all + generation 递增 + 无重复。

## 三、P2.2-D — Plugin Ecosystem 核心 ✅

- `providers/plugin_registry.rs` `PluginRegistry`（`plugins.db`）：
  - `plugins` 表持久化 enabled/quarantined/failures——**重启后状态完全恢复**（Gate 1）；
  - `record_failure`：计数 +1，≥3 自动隔离（§59/§61）；`record_success` 清零并解除隔离（§62）；
  - `plugin_capabilities` 表记录 manifest 请求的能力，decision 默认 `unset`（运行时 = denied，§19/§20/INV-PLUGIN-001）。
- `PluginProvider::set_registry`：启动时水合持久状态（quarantined/disabled/failures）；执行失败经 registry 持久化计数并返回隔离判定；成功重置计数。
- 测试 3 项：阈值隔离+成功重置、未知插件默认 enabled、enable 状态跨重启。

## 四、P2.2-E — Installer / Upgrade / Recovery 核心 ✅

- **崩溃环 enforcement**（§80/INV-UPDATE-006）：`consecutive_failures ≥ 3` → **DEGRADED BOOT**——跳过 IndexCoordinator（昂贵组件），Launcher 仍可用于恢复；`startup_state.json` 重置后自动恢复正常。
- **P2.1-B 宿主接线补全**：`watch_enabled` 配置（默认 true）+ IndexCoordinator 正式接入 build_core（此前仅 crate 实现+测试，宿主接线是 P2.2-E 降级启动的依赖，本批补齐）；降级启动跳过 coordinator。
- **Upgrade Handoff**（§25/§49）：`update_handoff.json` 启动时消费——中断升级告警并清除（v0.1 MSIX 部署层拥有实际回滚，我们只呈现状态）。
- **卸载数据保留策略**（§86/§87）：install.ps1 明示 Uninstall 保留 `%LOCALAPPDATA%\native-launcher` 与 `%APPDATA%\NativeLauncher`，完全清除为显式操作。
- 已有基础保留：schema_version、config 原子写 + 损坏隔离、单实例 Mutex、`--require-baseline`。

## 五、验证

```text
cargo test --workspace   571 passed / 0 failed（累计 +6：merge actions 1、catalog 1、
                          registry 3、p22f context-generation 1；另 snapshot/scenario 适配 2）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS
VR 基线                  有意重生成（图标槽布局）后 15/15 byte-identical 稳定
Release 基线             benchmarks/baseline-release.json（7µs/23µs/640µs/1.8MB）
```

## 六、P2.3 之前的状态与剩余

五批核心已收口。按 82 §25 的表，产品闭环剩余（均有归属，非阻塞）：
- **P2.1-C 主体**：Provider Contract v2 / SearchCoordinator 结构化 / 超时隔离；
- **P2.1-D 主体**：Catalog 增量 reconcile（持久层已就位）；
- **P2.2-D 主体**：Package/安装流/capability 审批 UI（registry 持久层已就位）；
- **P2.2-E 主体**：MSIX 打包/签名（需证书）、UpgradeCoordinator 状态机（handoff/health 标记已就位）；
- E5 收尾项：DPI 抽查、Icon package-resource 源。

**P2.3（Hardening & Product Acceptance：真实规模压测/多版本兼容/长跑/泄漏/RC 验收）** 现在具备了启动条件——数据面（Index/Catalog/UserState/Context）、搜索面（Coordinator/Cache/Ranking）、权限面（唯一 ActionEngine）、生命周期面（单实例/健康标记/崩溃环/安装包）四层均已稳定且互相独立。
