# Launcher 1.0 Release Notes

## Launcher 1.0.0

**发布日期**：2026-09-07
**状态**：Release Candidate

---

### 新功能

- **搜索**：确定性评分 + 路径搜索 + 语义身份去重 + 历史频率/新近 boost + 空查询最近使用视图
- **应用发现**：Start Menu .lnk + Uninstall 注册表 + MSIX/UWP（PackageManager 权威枚举）+ Portable（配置根有界扫描）
- **Action**：Open / Run as administrator / Copy path / Reveal / Paste / Run as admin（.lnk 解析 exe）
- **收藏**：SQLite 持久化 + 语义身份绑定 + pin⊆fav + Ctrl+D 切换
- **Query Cache**：generation-aware LRU（file/application/user_state/context/ranking 五代际）
- **Context Ranking**：FolderProximity + foreground 匹配附加分
- **Incremental File Index**：ReadDirectoryChangesW watcher + bounded queue + coalescer + overflow recovery
- **Workflow**：安装目录 workflows/*.json 可搜索触发 + WorkflowRunner 全生命周期
- **Plugin**：quarantine 状态机 + disabled 无候选 + 持久化状态
- **MCP**：stdio + streamable-http 双 profile + persistent runtime
- **Icon 管线**：SHGetFileInfo 抽取 + LRU 字节预算 + PNG 原子磁盘缓存
- **Settings**：Open Settings 命令 + 配置热重载（含热键重注册）
- **可恢复性**：全持久层 corrupt → 隔离/重建；崩溃环 → DEGRADED BOOT

### 基础设施

- **Release Gate**：十一 Gate 自动化（build/test/topology/arch/security/compat/E2E/VR/perf/docs/G10 perf budget）
- **安装包**：`scripts/package.py` → zip（install.ps1 + uninstall.ps1 + workflows/demo.json + README）
- **性能基线**：`benchmarks/baseline-release.json` + `benchmarks/thresholds.json`（budget 结构）
- **文档**：`docs/P2-CROSS-CUTTING-CONTRACTS.md` + `docs/P2.3-A-ARCHITECTURE-BASELINE.md` + `docs/P2.3-D-PERFORMANCE-BASELINE.md` + `docs/P2.3-C-RELIABILITY.md`

### 架构

- **唯一 ActionEngine**：`launcher-action`（validate → execute）
- **四层数据面**：File Index + Application Catalog + User State + Context（各自独立 generation）
- **搜索面**：identity dedup → RankingWeights → global rank → Top-K
- **二十八个 crate**：17 crates + 8 apps + 3 benches
- **不变量**：INV-AUTH / INV-SEARCH / INV-INDEX / INV-IDENTITY / INV-CACHE / INV-CONTEXT / INV-ICON / INV-PLUGIN / INV-FAV / INV-RECOVERY

### 明示不在 1.0

- Agent 宿主接线（实验性 crate 保留）
- MSIX 签名 / Store 分发
- Recovery Mode UI
- TimeBucket / Semantic Search / ML Ranking
- Cloud Sync / Marketplace / Enterprise
