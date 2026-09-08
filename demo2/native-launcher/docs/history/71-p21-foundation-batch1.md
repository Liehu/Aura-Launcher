# 71 — 对 70（P2.1 Search Foundation）的评审与第一批实施记录

日期：2026-09-06。基线：520 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审结论：方向认可，范围必须再切一刀

70 是一份完整的 P2.1 规划（Search/Identity/Index 三契约、8 Phase、约 120 新测试）。总评：

- **方向全部认可**：Identity 优先于算法（§2）、Retrieval 与 Ranking 分离（§6）、watcher≠indexer（§33）、bounded queue/coalescer（§34/39）、不做 embedding/ML/云索引（§3）、不加新 crate（§68）、Watcher 不进 launcher-runtime（§94）——这些判断都对。
- **但 112 节的 MUST 清单实际是一个 2-3 个会话的工程阶段**（仅 Watcher+Coalescer+IndexCoordinator+overflow 恢复就是 1000+ 行和 20+ 测试），不应在"评审即优化"的一批里吞下。按其自己的排序原则（先 Identity/Contract，再 Coordinator/Watcher，最后 Ranking），本批实施**顺序最前端、且不依赖 Watcher** 的部分，其余按阶段挂账。

## 二、本批已实施（P2.1 第一批）

| 70 条目 | 实现 |
|---|---|
| §57/58 空查询视图（MUST） | `Core::recent_commands(limit)`：usage_map × action_catalog join——最近/高频使用的命令**带完整可执行 actions**（历史行只有 id+title 的问题解决）；`spawn_search_or_recent` 接线，空查询不再显示 "no results"。测试 2 项 |
| §21/22/76 应用身份归并（INV-IDENTITY-001 第一版） | `merge_by_executable`：Start Menu Chrome.lnk（resolved exe）与注册表 chrome.exe 归并为一条（源优先级 start-menu > uninstall，身份键 = `normalize_path_identity(resolved exe)`）。测试 2 项（合并 + 不误合） |
| §14/29 路径身份规范化 | `launcher_domain::normalize_path_identity`：纯词法（小写 + 分隔符折叠 + `.`/`..` 收缩），**绝不访问磁盘**（防 junction traversal），仅用于身份比较。测试 1 项 |
| §52 RankingWeights | 评分权重全部收敛为 `launcher_search::RankingWeights`（关键词/模糊/多 token/history 频率与 recency），core 的 usage_boost 同源消费——魔法数字时代结束。既有语义零变化（默认值 = 原冻结值） |
| §80/81/82 Ranking 回归语料（起步） | quality.rs 新增 mixed-provider 语料测试：Top-1 = 应用（词法+类型先验）、结果集 Duplicate Rate = 0 |
| §71/106-108 Invariants | 手册冻结 INV-SEARCH-001（搜索永不同步无界扫描——68 FIX-05 已达成）、INV-SEARCH-002（boost 不制造候选——已有测试固化）、INV-INDEX-001/003（有界/不跟随 reparse——已实现）、INV-INDEX-004（部分）、INV-IDENTITY-001（部分） |

## 三、挂账（70 的其余 MUST → 专门阶段）

- **P2.1-D IndexCoordinator + P2.1-E Watcher + P2.1-F 增量索引**（§30-46/91-92/99-101）：ReadDirectoryChangesW、bounded queue、coalescer、generation、dirty-root、overflow 恢复、rebuild 与增量互斥。这是最大的单体工程，其中价值最高且最急的"启动不阻塞"已于 68 FIX-05 完成；其余需要专门一批（预计 20+ 测试）。
- **P2.1-B 完整 typed SearchIdentity**（§12-24/96-97）：SearchCandidate/SearchIdentity/SearchCoordinator + 新 Provider trait 是跨全部 provider 的契约重构，等应用合并与 watcher 落地后再统一换型，避免两次返工——正是 70 §112 自己警告的顺序问题。
- MSIX/UWP 发现（§25-D3）、Portable provider、icon cache（§87-88）、query cache（§48）、context ranking（§54）、Favorites（§60）、性能门限冻结（§84-85）、完整语料库（§81）。

## 四、验证

```text
cargo test --workspace   520 passed / 0 failed（+6：recents 2、身份归并 2、语料 1、路径规范化 1）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```
