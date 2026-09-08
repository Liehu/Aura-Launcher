# 84 — P2.3 系列评审与收口启动记录（83-roadmap + 84~93 九批）

日期：2026-09-07。基线：571 tests / 零警告 / Release Gate（含新增 G10）全 PASS。

---

## 一、评审结论

P2.3 系列（A–I 九批，约 1.1 万行）定义为 **Launcher 1.0 Release Closure**——"不再新增核心功能；验证 P1/P2 能力已成完整产品"。评审结论：

- **定位正确**：A 架构契约收口 → B 产品 E2E → C 可靠性 → D 性能 → E 安全 → F UI QA → G 安装升级 QA → H Release 工程化 → I Release Candidate。每批"不新增功能"的铁律正确。
- **83-roadmap §一 的 Cross-Cutting Invariant Audit** 与 **82 §26 的横向契约文档**是同一件事——本批直接落地（见下）。
- **不能一批做完**：九批合计约 1.1 万行 + 大量需要真实安装环境的验收（MSIX 签名、多版本升级矩阵）。本批只做**零风险、即做即固化**的收口项；其余按批挂账。

## 二、本批实施（P2.3 启动切片）

### 1. `docs/P2-CROSS-CUTTING-CONTRACTS.md`（正式立档，ACCEPTED）

横向契约唯一登记处：
- **ID 目录**：SearchRequestId / ExecutionId / RuntimeId / ProtocolSessionId / PluginId / PluginInstanceId / UpgradeTransactionId——产生者、生命周期、持久化、互不替代铁律；
- **Generation 目录**：file / application / user_state / context / ranking / CatalogGeneration / PluginCatalogGeneration——递增时机、持久化、Cache 影响；
- **State Matrix**：七类变化 → 缓存/用户态/持久层/运行时的精确影响；
- **Authority 五条**（唯一 ActionEngine 等）与**生命周期统一原则**（Plugin 与 Launcher 共享原则、分离实现）。

### 2. Release Gate G10：性能回归 gate（P2.3-H/H6 第一片）

`release_gate.py` 新增 **G10**：Release 构建的 launcher-bench 运行 10k 数据集，release app p95 对照 `benchmarks/baseline-release.json`（本批基线 = 7µs），容差 +50%（P2.3-D §42：budget = baseline × tolerance）。当前 **7µs，达标**。基线缺失或未构建 release bench 时 SKIP（不阻塞非性能验证轮）。

## 三、逐批挂账（均为专门会话）

| 批 | 内容 | 前置 |
|---|---|---|
| A 架构契约收口 | INV-* 全目录审计对照实现（85 号已列 INV-AUTH/RUNTIME 等条目式契约） | 无；可直接对照 CROSS-CUTTING-CONTRACTS.md 逐条验证 |
| B 产品 E2E | 启动→任务→完成的真实链路自动化 | 无 |
| C 可靠性 | 崩溃/超时/损坏/中断恢复矩阵 | 建议在 B 后 |
| D 性能/内存 | Release + 真实规模 + 长跑预算 | 依赖基线（已有） |
| E 安全 | 不可信输入→权限提升矩阵终审 | 无 |
| F UI QA | DPI/键盘/VR 全量 | Icon E5 完成后 |
| G 安装升级 QA | MSIX 生命周期矩阵 | 依赖签名证书 |
| H Release 工程化 | Pipeline 统一编排（G10 是第一片） | 依赖 A–G |
| I RC | 交付物冻结 | 最后 |

## 四、验证

```text
cargo test --workspace   571 passed / 0 failed
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十一 Gate 全 PASS（新增 G10：release app p95 7µs 达标）
```

## 五、P2.3 执行建议

按 84 号批次结构逐会话推进（A → B → C → D → E → F → G → H → I），每批自带 Gate 与回归测试；**H 的 Pipeline 编排（92 号 §106 Release Pipeline）在各批完成后统一收口**。I（RC）完成即打 **Launcher 1.0**，之后进入 P3.x 产品演进（83-roadmap §三：Search Intelligence / Personalization / 等四条主线），不再于 1.0 内堆功能。
