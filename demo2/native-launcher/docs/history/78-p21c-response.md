# 78 — 对 77（P2.1-C SearchCoordinator）的评审与第一批实施记录

日期：2026-09-07。基线：539 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审结论：目标正确，Contract v2 全量换型继续后置，语义去重先行落地

77 把 Batch 3（SearchCoordinator / SearchIdentity / Provider Contract v2）的完整设计摆了出来。逐项判断：

**已经成立、无需重做的部分**（Core 层现状，均有测试固化）：
- **C2 Query supersession**：`SearchSession` query-id 机制（宿主）——后到查询作废旧查询；
- **C4 Provider 失败隔离**：`Core::search` 逐 provider 收集错误进 `SearchResult.errors`，健康 provider 结果保留（INV-SEARCH-004 + quality 测试）；
- **C8 全局排名 + Result Limit**：`rank_with_boost`（RankingWeights + usage boost）+ 统一 limit，provider 不做最终截断（MAX_FILE_HITS 是候选预算不是结果上限，§21/22 的区分已满足）；
- **C5 空查询**：Batch 1 已实现并满足 §16 的"History + Live Catalog"规则（INV-SEARCH-003）；
- **§26 Search/Action 边界**：现状即如此——Search 不验证权限不执行 Effect，`launcher-action` 仍是唯一 ActionEngine（77 自己也确认"不新增第二个 ActionEngine"）。

**本批真正缺失且值得立即做的**：**C6/C7 跨 Provider 语义去重**——现有的 `(provider_id, id)` 去重只防同 provider 重复，Recent 的 `chrome.exe` 与应用 `Google Chrome` 仍会双行显示。已实施：

- `launcher-search::search_identity_key`（IdentityKey v1，按 77 §28 的许可用稳定 String 而非 typed enum）：
  - Application/File/Folder → `path:` + `normalize_path_identity(target)`（统一路径命名空间——应用与其 File 命中对用户是同一对象）；
  - 其余（Command/Plugin/MCP）→ `cmd:{provider_id}:{id}`，**provider-scoped 不跨源合并**；
  - 身份只来自 host-resolved target，绝不由插件元数据提供（延续 INV-SEARCH-003 的信任边界）。
- `rank_with_boost` 二段去重：先 (provider_id,id)，后 identity key（排序后保留最高分来源——§18 的"不丢来源"语义在 v1 中体现为保留最优来源行；多来源清单 `merged_sources` 已在 AppEntry 层保留）。
- app-registry 命令的 `target` 改为携带 canonical exe（Open action 仍打开快捷方式本身）——使 Start Menu `.lnk` 能与 Recent 的 exe 合并。
- 测试 4 项：C6 跨 provider 合并、C35 同名不同目录不误合、文件规范化路径合并、Command 不跨 provider 合并。

## 二、挂账（77 的其余部分 → Batch 3 主体）

- **Provider Contract v2**（`SearchProvider`/`SearchSink`/`SearchCandidate`/`SearchBatch`/事件流）：跨全部 provider 的 trait 换型；现状 Core 顺序 fan-out 已确定性满足行为，等 MSIX/Portable 等新 provider 出现前再换。
- **Provider 超时隔离**（§11/§33）：MCP/Plugin RPC 已有内部超时；宿主级 per-provider 线程+超时随 Contract v2 一起做（所有权模型才有意义）。
- **SearchContextSnapshot / Context Ranking**（§24/§25/§43）、**SearchDiagnostics/SearchSnapshot**（§39/§40）、**SearchRequestId 类型化**（§9，当前 u64 session 语义等价）：随 Coordinator 结构化落地。
- Query Cache（§42）、Search Explain（§41）、P2.1-D Application Discovery 2.0（§44）：按既定顺序后置。
- §29 Source Boundary（launcher-search 禁依赖 action/workflow/ai/ui/mcp/plugin-host）：现状本就满足，已有拓扑类守卫覆盖，待 Batch 3 收尾时以 source guard 形式固化。

## 三、验证

```text
cargo test --workspace   539 passed / 0 failed（+4：C6 合并、C35 不误合、
                          文件路径合并、Command provider-scoped）
cargo build --workspace  zero warnings
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 四、下一步

按 77 §44 顺序：**P2.1-D Application Discovery 2.0**（MSIX/UWP、Portable、Application Identity v2、Icon source）或 Batch 3 主体（Provider Contract v2 + SearchCoordinator 结构化）二选一——建议先 D 后 C 主体，让新 provider 直接实现 v2 契约，避免二次迁移。
