# 91 — P2.3-E Security / Trust Boundary Closure 实施记录

日期：2026-09-07。基线：599 tests / 零警告 / Release Gate 全 PASS。

---

## 评审结论

89 号文档 E1-E14 十四项安全审计，其中大部分已被既有安全基础设施覆盖：

- **G4 Security Matrix**（75 adversarial tests, S0=S1=S2=0）覆盖 E3-E9（执行权、身份、插件、MCP、Workflow、AI、进程安全）
- **G3 Architecture Guards**（70 条）覆盖 E12 Static Security Gate（依赖禁止 + SDK 边界 + runtime 隔离）
- **check_topology.py** 覆盖 §13 Dependency Scan（SDK/runtime/HTTP 库守卫）
- **INV-SEARCH/INDEX/IDENTITY/CACHE/CONTEXT/FAV** 不变量（P2.1-A~F 期间逐批固化）覆盖 §75 Security Boundary

**真正缺失且本批可修**的只有 **E11 Input / Resource Limits**。

## 本批实施

### E11 — Input / Resource Limits
- `Core::search` 查询长度 cap **512 chars**（Unicode 安全截断）——防止 OOM in tokenisation/ranking。
- 测试：100KB query 不 panic；Unicode query 截断安全。

### 已有覆盖（确认，不重复实现）
| 项目 | 来源 |
|---|---|
| portable_roots 有界扫描（depth ≤2, count ≤128） | P2.1-D1 |
| 路径身份规范化（lexical only, 不访问磁盘） | P2.1-A |
| reparse point 不跟随 | 65 号 FIX |
| config 损坏隔离 + 原子写 | 67 MUST-2 |
| index.db corrupt → 删除重建 | P2.3-C |
| favorites.db corrupt → 隔离 + 重建 | P2.3-C |
| plugin registry corrupt → 重建 | P2.3-C |
| icon cache corrupt → 删条目 + 重抽取 | P2.1-E |
| Query supersession | SearchSession（ADR-0004）|
| Stale action 保护 | context_is_stale 检查 |

## P2.3-E 十四项覆盖状态

| 项 | 覆盖 | 说明 |
|---|---|---|
| E1 Security Contract Freeze | ✅ | docs/P2-CROSS-CUTTING-CONTRACTS.md |
| E2 Trust Boundary Inventory | ✅ | 89 §3 表已对照实现确认 |
| E3 Action/Effect Authority Audit | ✅ | G4 security matrix 75 tests |
| E4 Execution Identity Audit | ✅ | ExecutionId 单源透传（65 FIX）|
| E5 Plugin Security Audit | ✅ | quarantine + disabled + protocol violation |
| E6 MCP Security Audit | ✅ | G5 compat matrix + G6 MCP E2E |
| E7 Workflow Security Audit | ✅ | identity_mismatch + generation guard |
| E8 AI/Agent Security | ✅ | Agent crate 隔离（不接宿主=无攻击面）|
| E9 Runtime/Process Security | ✅ | topology guard + ProcessSupervisor |
| E10 Persistence/Config Security | ✅ | 原子写 + 损坏隔离 + schema_version |
| **E11 Input/Resource Limits** | ✅ | query cap 512 + MAX_FILE_HITS 100 + MAX_INDEX_ENTRIES 500k |
| E12 Static Security Gate | ✅ | G3 70 architecture guards |
| E13 Adversarial E2E Suite | ✅ | G4 75 adversarial tests |
| E14 Security Closure | ✅ | S0=S1=S2=0 + 全 Gate PASS |

## 验证

```text
cargo test --workspace   599 passed / 0 failed（+2：query_length_capped、unicode_query_truncated_safely）
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py --require-baseline  十 Gate 全 PASS（含 G10 perf regression）
```
