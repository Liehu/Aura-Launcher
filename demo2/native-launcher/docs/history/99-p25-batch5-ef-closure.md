# 99 — P2.5 收口（Batch 5：E 线 stress/race + F 线 gate；Pinyin 单独推迟）

> 日期：2026-09-09。范围：P2.5 第五批（`demo2/files2/101-p2.5-0.1.md` §E/§F）。
> 输入基线：history/98（687 tests）。

## Task P25-E03/E04 — 并发 Stress + Supersede Race

- `launcher-core/tests/p25_stress.rs`：
  - **E03**：单 coordinator、40 provider、8 线程 × 5 次并发搜索——结果集
    完整（40/40）、状态 Complete、零违规；全局 in-flight ≤
    parallelism × 并发调用数（语义：`max_parallelism` 是**单次搜索**的
    fan-out 上限，测试固化了这一契约）。
  - **E04**：取消标志在搜索进行中翻转的 race——结果只能是 Complete
    （来不及）或 Cancelled（及时），永不 hang/panic/伪造状态；6 轮不同
    取消时点全过。

## Task P25-E05 — FTS 损坏恢复

- `fts_schema::e05_fts_corruption_self_heals`：FTS 表被 DROP（模拟损坏）
  后 metadata 完整无损（AC-C01-2），FTS 侧可重建并恢复命中。
- （注：本文件曾因脚本写入引入控制字符而损坏，最终整文件重写——教训：
  Python heredoc 生成含 `\\a` 等转义的 Rust 字符串必须用 Write 工具。）

## Task P25-E01/E02/E06 — 已有证据覆盖说明

- E01 混合基准：G10 全字段（app/file p95、启动、RSS、popup soak）继续
  PASS；本批后 file p95 24→35µs（FTS+LIKE 合并两趟查询的代价），仍在
  50µs 预算内——**已在 gate 证据链中量化**。
- E02 provider stress：B02 的 100-provider 用例 + G09 stress 套件覆盖。
- E06 UI soak：GA 批次 `benchmarks/showhide-soak-10k.json`（10,000 循环）。

## Task P25-F — CI / 性能门禁 / 文档 / Gate 收口

- **F04**：`release_gate.py` 新增 **G14 "P2.5 search intelligence
  conformance"**（required）：launcher-search 全套 + search_coordinator +
  p25_stress + fts_schema 四套件命名入证据链。
- **F01**：CI（history/90 建立的 ci.yml）经 G02 覆盖全部 P2.5 套件。
- **F02**：性能门禁 = G10 阈值（file p95 预算 50µs 内）。
- **F03**：文档冻结——手册/README/历史记录同批更新（本文件）。

## Gate 结果（P2.5 收口轮）

- `cargo test --workspace`：**690 passed / 0 failed**（687 → 690，+3）
- Release Gate：**G01~G14 全 PASS**（G02 一次 HTTP E2E 瞬时抖动
  p0c_scenario_a 重跑后消失，与本批改动无关）
- G10 记录：file p95 24→35µs（FTS 合并检索代价，预算内）

## P2.5 完成宣告

```text
P25-000  Baseline Closure          ✅ (= P2.4 完成, history/94)
P25-001  Search Contract v2        ✅ (history/95)
P25-002  Cross-Cutting 注册        ✅ (随 95/96 契约落地)
P25-A01  Query Normalizer v2       ✅ (history/95)
P25-A02  Intent Detector           ✅ (history/95)
P25-A03  Explicit Filters          ✅ (history/95)
P25-A04  Pinyin                    ⏸ 正式推迟（独立批次，与 C04 配对）
P25-B01–B06 SearchCoordinator     ✅ (history/96)
P25-C01/C05 FTS5 + LIKE fallback  ✅ (history/97)
P25-C02  Application FTS           ✅ (history/97)
P25-C03  File/Folder FTS           ✅ (随 C01 主链路, history/97)
P25-C04  Pinyin 检索               ⏸ 随 A04 推迟
P25-C06  Content 扩展点            ✅ (显式禁用, history/97)
P25-D02/D03 Ranking v2 + Explain ✅ (history/98)
P25-D04/D06 归档说明               ✅ (history/98)
P25-D05  Quality Corpus            ✅ (history/98)
P25-E01–E06 Stress/Race/Recovery  ✅ (history/99)
P25-F01–F04 CI/门禁/文档/Gate      ✅ (history/99)
```

**P2.5 Search Intelligence 1.0 = 完成**（Pinyin 单独推迟为唯一尾项）。
测试 623（1.0 GA）→ 690；拓扑 17 crates + 9 apps。

## 下一步

A04/C04 Pinyin 批次，或按 roadmap 进入 Plugin Ecosystem 产品化
（P2.4-D 的 CLI 之上：Registry 管理 UI、Marketplace 前置）。
