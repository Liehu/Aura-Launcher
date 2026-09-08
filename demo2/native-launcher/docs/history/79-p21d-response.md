# 79 — P2.1-D Application Discovery 2.0 第一批实施记录

日期：2026-09-07。基线：547 tests / 零警告 / Release Gate 十项 PASS。
按 78 号文档（P2.1-D 冻结契约）实施，范围聚焦本机最大功能缺口与低风险项。

---

## 一、交付内容

### 1. MSIX/AppX/UWP 打包应用发现（76/78 的核心缺口，本机此前完全缺失）

- `launcher-providers/src/packaged.rs`：
  - **PackageManager 是权威枚举源**（`FindPackages`，WinRT via windows crate，新增 `Foundation`/`Foundation_Collections`/`Management_Deployment`/`ApplicationModel` feature）——绝不扫描 `WindowsApps` 目录（INV-APP 语义 / §9）；
  - 跳过 framework/resource package；读取已安装包的 `AppxManifest.xml`，解析 `<Applications><Application Id= DisplayName=>`——**Package 与 Application 分离建模**（§7）：一个包的多个 Application 各自成条目；
  - 身份 = **PackageFamilyName + ApplicationId**（§6/§42/INV-APP-004）：版本/架构/全名只是 metadata；`packaged_identity_key` 版本稳定、ApplicationId 包内作用域（测试覆盖 §79 两组用例）；
  - Launch = `shell:AppsFolder\<family>!<appid>` 别名（§11/§13：激活属于 Action 层，Catalog 只记录"如何启动"）；display name 取 manifest 字面值，`ms-resource:` 回退包名（§63 确定性清理，无 AI/网络）；
  - 有界（≤512），枚举失败降级为 WARN + 空结果（Gate D7 源失败隔离）。
- **实时集成测试**：本机 PackageManager 枚举 → 非空 catalog、别名格式 `shell:AppsFolder\*!` 校验（Gate D3）。

### 2. Portable 应用发现（配置驱动，默认关闭）

- `app_registry::portable_entries(roots, max, max_depth)`：**只扫配置根**（§29），depth ≤2、数量 ≤128 有界（§31，Gate D6）；确定性 display 清理（`toolA.exe` → "ToolA"，§63）。
- 配置新增 `portable_roots = []`——默认关闭，用户显式配置才启用，绝不全盘扫描。
- **Gate D2/D12**：portable exe 与 Start Menu shortcut 同 exe 归并为一条（identity merge 复用，来源保留 merged_sources）——测试覆盖。

## 二、有意未做（与 76/78 的排除清单一致）

- Catalog 数据库四表 ERD/SQL（§55-57）：当前 exe-merge + 语义去重已在内存层满足；等 packaged/portable 数量证明需要 SQLite catalog 再引入。
- ApplicationCatalogCoordinator / 独立 generation / 增量 reconcile（§34-37/§51-53/§70-73）：当前目录源启动时一次枚举即可（成本 ~百 ms 级），打包源权威枚举本身有内部超时；等出现真实性能证据再结构化。
- AUMID/GetApplicationUserModelId 运行时关联（§14/§15）、IconKey/Icon Cache（§64）、App Paths 注册表扩展（§19）：后置。
- ApplicationState(Broken/Stale) 状态机（§25/§26/§60/§61）：现实现"解析失败即跳过"，等有持久化 catalog 再有意义。

## 三、验证

```text
cargo test --workspace   547 passed / 0 failed（+8：packaged 5（含 1 实时集成）、
                          portable 2、manifest/identity/alias 纯函数）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 四、定位说明

本批只完成了 P2.1-D 的 **D1-D3 + D6 + D7**（Win32 统一目录、打包应用发现、portable 有界发现、源失败隔离）；D4/D5 已由 manifest 解析与 identity key 测试覆盖其纯函数部分，**D8-D10（Catalog coordinator/后台 refresh/全语料）** 需要先有 SQLite catalog 持久层，按 §67"先暴露现有 catalog、后台 reconcile"的原则留给下一批。Application Discovery 的核心架构约束（来源不进 Search、身份解析确定性无副作用、Package≠Application、family+appid 身份）已全部生效。
