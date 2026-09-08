# 82 — 对 82（Cross-Batch Consistency Review）的评审与 P2.2-F 收口实施记录

日期：2026-09-07。基线：566 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审结论

82 的核心判断成立：**骨架正确，问题是"契约已升级、实现停在上层"的断层**。其 6 个契约断点逐一核实与处置：

| # | 82 指出的问题 | 核实 | 处置 |
|---|---|---|---|
| 1 | CacheKey 缺 ApplicationGeneration | ✅ 属实（且真实影响：plugin enable/disable/quarantine 改变候选集但旧缓存继续命中） | ✅ 已修：`application_generation` 进入 SearchCacheEnvironment/Key |
| 2 | CacheKey 缺 RankingGeneration | ✅ 属实（结构性缺失） | ✅ 已修：`ranking_generation` 进入 Key；当前恒 0（权重尚不可运行时修改，§80 认可），字段已参与相等性 |
| 3 | Context Snapshot 生命周期（popup once ≠ per request） | ⚠️ 半属实：本产品 popup 打开期间前台必为本 Launcher（用户无法 Alt+Tab 走开而不关闭 popup），"popup show 捕获"在实际交互模型下等价于 request-time 捕获；但**语义代际确实未闭环** | ✅ 已修：`set_search_context` 改为语义比较（foreground+normalized folder），仅语义变化时 `context_generation++`——重复捕获同状态不再 bump（§64），变化即自然 miss（§51） |
| 4 | ContextGeneration 未真正闭环 | ✅ 属实 | ✅ 同上（INV-CONTEXT-004 落实） |
| 5 | Application Catalog 未成 Catalog | ✅ 属实（是 discovery aggregation） | ⏳ P2.1-D.1 挂账（SQLite Catalog + CatalogGeneration + reconcile），与本批 application_generation 互补：后者已覆盖 plugin 生命周期这一真实变化源 |
| 6 | Candidate dedup ≠ Candidate Merge | ✅ 属实 | ✅ 已修（最小版）：identity 收拢时**回填被折叠行的缺失 subtitle**，不再静默丢弃来源元数据；完整 canonical-candidate + merged actions 随 Contract v2 |

另采纳两项文档/状态修正：
- **§22/§23 状态模型**：手册里程碑改用"核心切片 ✅ / 产品闭环 ⚠️❌"标注，不再用单一 ✅ 混淆设计与实现完成度；
- **§26 横向契约文档**：State Generation Matrix（§20）已写入本文件 §三，后续新状态先入表再实现。

## 二、本批代码改动（P2.2-F 收口补丁）

1. `launcher-search/cache.rs`：SearchCacheEnvironment/Key 补 `application_generation` + `ranking_generation`（参与相等性——任一变化即 miss）。
2. `launcher-core`：
   - `application_generation` / `ranking_generation` / `context_generation` 三个计数器 + `last_context_state`；
   - `set_search_context` 语义比较 → 语义变化才 bump context generation；
   - `bump_application_generation()`（插件 enable/disable/quarantine 时由宿主触发）；
   - **Cache lookup 移至 provider dispatch 之前**（本批 E2E 抓出的顺序错误：原实现缓存查询在 provider 循环之后，缓存从未真正生效——正是 82 §55 担心的那类错误）。
3. `rank_with_boost` 身份折叠回填 subtitle。
4. 新测试：context 语义变化 → provider 重查；相同语义重复捕获 → cache hit（provider 调用数 = 2 而非 4）。

## 三、State Generation Matrix（§20，冻结）

| State | Owner | 持久化 | 影响 Cache Key | 当前实现 |
|---|---|---|---|---|
| File Index | Indexer | ✅ index.db | ✅ file_generation | ✅ |
| Application Catalog | （P2.1-D.1） | ⏳ | ✅ application_generation | ⚠️ 代际在、持久层缺 |
| Favorite/Pin/Usage | User State | ✅ favorites.db | ✅ user_state_generation | ✅ |
| Context | Context Manager | ❌（仅当前快照，§40） | ✅ context_generation | ✅ |
| Ranking Config | Ranking | config | ✅ ranking_generation（预留） | ✅ 字段 |
| Plugin Catalog | Plugin Manager | ⏳ | 经 application_generation | ⚠️ |
| MCP/Provider health | Provider Runtime | ❌ | ❌ 暂不纳入（§20） | — |

## 四、验证

```text
cargo test --workspace   566 passed / 0 failed（+1：context-generation 失效闭环 E2E）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 五、挂账（82 P1/P2，顺序照旧）

P1：Candidate Merge 完整版（canonical candidate + merged actions，随 Contract v2）、Icon UI 接线（随 VR 重生成）、Release Performance Baseline（Release 构建 + 固定数据集/机器）。P2：Plugin Registry/Package/Capability（P2.2-D 主体）、MSIX Upgrade/Recovery（P2.2-E 主体）。P2.1-D.1 Persistent Catalog 按 82 §四 作为独立小批。P2.3 在上述收口前不启动。
