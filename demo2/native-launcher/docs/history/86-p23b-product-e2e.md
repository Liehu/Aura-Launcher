# 85 — P2.3-B Product E2E 实施记录

日期：2026-09-07。基线：574 tests / 零警告 / Release Gate 十 Gate PASS（含 G10）。

---

## 一、交付内容

`crates/launcher-core/tests/product_e2e.rs` — 3 项真实链路 E2E，覆盖 84 号文档 §"End-to-End Product Closure" 的可自动化部分：

### PRODUCT-E2E-001 Golden Path（搜索→排名→选择→执行→历史→重排→收藏→空查询）
- 真实 FileProvider（temp 索引）+ app-registry 静态目录 + FavoriteService；
- 查询 "report" → 文件结果带 Copy path action（INV-SEARCH-003）；
- 查询 "chrome" → **模拟宿主执行成功**（record_use_with_title，与 execute_action_by_id 相同调用）；
- 再查 "chrome" → **usage boost 保持 #1**；
- toggle_favorite → 空查询 recents 含 Chrome 且全带可执行 actions（INV-SEARCH-003）；
- 无匹配查询 → 空结果（INV-SEARCH-002：boost 不复活候选）。

### PRODUCT-E2E-002 文件索引 + rescan
- Indexer rebuild → search → 新文件 → rescan_root → search 命中。

### PRODUCT-E2E-003 Workflow 目录发现 + 定义往返
- 安装目录 workflow → 搜索可发现 → target 定义文件 → re-validate → 名称一致。

### 补充：plugin_registry 隔离持久化（P2.2-D 核心）
- 3 项：阈值隔离+成功重置、未知插件默认 enabled、enable 跨重启。

## 二、验证

```text
cargo test --workspace   574 passed / 0 failed（+3：golden path、file index、workflow catalog）
cargo build --workspace  zero warnings
release_gate.py          十 Gate 全 PASS（G10: release app p95 7µs）
```

## 三、覆盖对照（84 §"End-to-End Product Closure"）

| 链路 | E2E |
|---|---|
| 搜索→排名→选择→执行→历史→重排 | ✅ golden_path |
| 收藏→空查询→可执行 | ✅ golden_path Act 4-5 |
| 文件索引→搜索→rescan | ✅ file_index + incremental_e2e（P2.1-B） |
| Workflow 目录→发现→验证 | ✅ workflow_catalog_discovery |
| 插件链（搜索→engine→broker→进程） | ✅ calculator-plus e2e（既有）|
| MCP 链 | ✅ G6 MCP E2E 16 项（既有）|
| 收藏持久化 | ✅ favorites::tests（既有）|
| Context 代际失效 | ✅ p22f context_semantic_change（既有）|
| GUI 键盘/物理交互 | ⏳ P2.3-F（需要人工或 computer-use 工具）|

## 四、P2.3-B 结论

**核心可自动化部分 = CLOSED**。剩余 GUI 物理交互与 DPI 抽查归 P2.3-F。下一批 P2.3-C 可靠性（崩溃/超时/损坏/中断矩阵）按 87 号文档推进。
