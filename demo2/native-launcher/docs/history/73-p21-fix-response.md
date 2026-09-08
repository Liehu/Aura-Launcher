# 73 — 对 72（P2.1 Batch 1 验收 + 收口建议）的评审与实施记录

日期：2026-09-06。基线：526 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审结论

72 的定性正确：**P2.1 Foundation Batch 1 = ACCEPTED**，推迟 Typed SearchIdentity 的判断也正确。其"下一批 = P2.1-B Incremental Index Engine（单独批次）"的排序建议采纳——本批只做 72 列出的**收口小项**（全部低风险、即做即固化的），引擎本体不做。

## 二、采纳并实施（5 项）

| 72 条目 | 实现 |
|---|---|
| §2 INV-SEARCH-003 | 冻结进手册：空查询结果必须对当前 live catalog 解析，历史记录本身不构成可执行权威。实现即 Batch 1 的 catalog join，无需新代码；不变量文本已入册 |
| §31 INV-SEARCH-004 | 冻结进手册 + **新增回归测试**：一个 provider 报错（模拟 MCP 不可达）时，健康 provider 的结果必须保留且失败必须可见（`provider_error_does_not_discard_other_results`）。核实结论：Core::search 的错误隔离本就成立，现在有测试钉住 |
| §8 Windows 路径身份规则 | `normalize_path_identity` 正式定义：`\\?\` 设备前缀透明剥离；`\\?\UNC\server\share` ≡ `\\server\share`；UNC 保持双前导分隔符且**永不等于**盘符路径；尾分隔符折叠；`.`/`..` 词法处理。新增 3 组测试（含 UNC 与盘符路径互不相等） |
| §5 归并保留 discovery sources | `AppEntry` 增加 `merged_sources`：身份归并不再丢弃被发现来源，`source` 仍为主来源——将来"注册表来源消失但 Start Menu 残留"的 staleness 判断有了数据基础。测试 1 项 |
| §25/26 IndexGeneration 预留 | `index_meta.generation`：每次 rebuild +1（recovery rescan 同样），`Indexer::generation()` 读取，首建前为 0。查询缓存与 diagnostics 以后按 generation 失效即可。测试 1 项 |

另按 §33 落了 **ApplicationIdentityResolver 接缝**：v1 身份算法收敛为具名 `executable_identity()` 函数——未来 AUMID/MSIX/Portable 解析器实现同签名并入，Search 永不感知来源类型（正是"不要硬编码 ApplicationId = exe path"的最小落实）。

## 三、拒绝 / 后置

- **P2.1-B Incremental Index Engine 本体**（Watcher/Queue/Coalescer/Coordinator/Overflow/状态机/6 条 INV-INDEX-005~010）：完全按 72 的批次定义留待专门会话——这正是它反复强调的"不要把最大工程揉进收口批"。
- §36 Search Explain DTO、§11/12 语料扩张（Golden Dataset/MRR/NDCG）：等 Typed SearchIdentity 与增量索引稳定后再做，避免评测基准跟着候选集一起漂移。
- §46 冻结清单（Agent Confirmation/memory、Workflow loops、MCP 新协议、Embedding、ML、全文检索、OCR、Cloud、Icon/Query Cache、Favorites）——**全部同意，继续冻结**。

## 四、验证

```text
cargo test --workspace   526 passed / 0 failed（+6：路径规则 3、来源保留 1、generation 1、provider 隔离 1）
cargo build --workspace  zero warnings
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 五、下一步

按 72 §45/§49 的路线：**P2.1-B Incremental Index Engine** 作为独立批次（FileChange → Watcher → BoundedQueue → Coalescer → Coordinator → IncrementalWriter → DirtyRoot/Overflow → Generation/Health），批次内自带其 6 条新 invariant 的回归测试。该批完成后再进入 Batch 3（Typed SearchIdentity / SearchCoordinator / Provider v2）。
