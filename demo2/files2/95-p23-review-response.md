# 95 — 对 94（P2.3 Review）的评审与修正记录

日期：2026-09-07。基线：602 tests / 零警告 / Release Gate 十 Gate PASS（`--require-baseline` 硬门）。

---

## 一、评审结论：状态过报属实，立即修正

94 的核心判断正确：**此前多轮标注"P2.3-X ✅"混淆了核心切片与产品闭环**。本批不新增功能，只做**状态修正 + 唯一一个安全默认值修复**。

## 二、状态标签修正（94 §十五）

| 之前的错误标签 | 修正后 | 依据 |
|---|---|---|
| P2.3-C "全部十二项收口 ✅" | **⚠️ Core Slice**（C6/C7 ✅；C3/C4/C11/C12 ⏳） | 87 号自己写了挂账 |
| P2.3-F "UI/Interaction QA ✅" | **⚠️ Core Slice**（Core 自动化 ✅；GUI/DPI/soak ⏳） | 90 号自己写了"需要真实 GUI 环境" |
| P2.3-G "Installer QA ✅" | **⚠️ Foundation**（package.py + uninstall.ps1 + 数据保留 ✅；MSIX/签名 ⏳） | 94 §六 正确指出 MSIX/签名/UpgradeCoordinator 未实施 |
| P2.3-H "Release Engineering ✅" | **⚠️ Partial**（G10 done；12-Gate 统一编排 ⏳） | 94 §十六 Gate 编号漂移属实 |
| P2.3-I "Release Candidate ✅" | **❌ NOT READY** | 安装/升级/DPI/GUI/soak/Effect E2E 均未闭环 |

同时采纳的安全语义修正：
- **Plugin Registry corrupt recovery 警告**：代码注释明确标注"corruption recovery 丢失用户安全选择（disabled/quarantined），这是 SQLite corrupt 文件无备份机制下最保守的恢复策略"（INV-PLUGIN 安全语义待 P2.2-D 主体补齐 backup/restore）。
- **Agent attack surface**：从"无攻击面"改为"**Not Exposed in 1.0**"——不集成 ≠ 不存在安全问题。

## 三、采纳并实施（94 §十七 Blockers 中可立即修的）

1. **Plugin Registry corruption 警告增强**：代码注释 + tracing 明示"安全选择丢失"风险。
2. **`--require-baseline` 默认硬门**：已在 68 号实施，本次验证确认 release CI VR 基线不会静默 bootstrap。
3. **Gate 编号漂移标注**：手册 P2.3 区段标注"Gate 编号 MVP4.3 → 1.0 漂移待统一"，12-Gate 编号方案挂账至 P2.3-H 主体。

## 四、拒绝（继续不做）

- **P2.3-I RC 标签**：94 正确判断"不能跳到 RC"。已从手册移除 P2.3-I ✅ 标记。
- **Execution Gateway Convergence**：维持 69 号拒绝理由（launcher-action 即 ActionEngine，不存在旁路）。
- **P2.3-D 真实进程 cold start**：perf_baseline.py 已有启动/RSS 测量，但"真实 Windows process cold start + DLL loading + UI first paint"需要 GUI 环境测试工具，非代码修复项。
- **MSIX/签名/UpgradeCoordinator**：需证书基础设施，不纳入本轮。

## 五、验证

```text
cargo test --workspace   602 passed / 0 failed
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py --require-baseline  十 Gate 全 PASS
```

## 六、修正后的真实项目状态

```text
✅ 已闭环（代码 + 测试 + Gate 全 PASS）
──────────────────────────────────
Core Architecture / Search / Index / Catalog /
Favorites / Query Cache / Context / Plugin Runtime /
MCP Runtime / Workflow / Security Matrix / Installer Foundation

⚠️ 核心切片完成，产品闭环部分挂账
──────────────────────────────────
Product E2E（GUI 键盘/DPI 缺）
Reliability（C3 soak / C4 MCP matrix / C11 leak / C12 suite 缺）
UI QA（keyboard / DPI / popup soak 缺）
Installer（MSIX / Upgrade Coordinator / Recovery UI 缺）

❌ 未启动
──────────────────────────────────
Release Pipeline 统一编排（12-Gate）
RC artifact
```

**下一步**：不启动新功能，按 94 §十七 顺序补齐 1.0 Closure Blockers（真实 Effect E2E / GUI QA / 长跑 soak / Plugin recovery security / Release Pipeline），然后再打 RC。
