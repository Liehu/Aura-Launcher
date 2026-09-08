# 75 — 对 74（Batch 1 验收 + P2.1-B 开工指引）的评审与实施记录

日期：2026-09-07。基线：527 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审结论

74 是 Batch 1 的验收书（ACCEPTED / CLOSED）+ P2.1-B 的设计指引，**没有新增 bug 主张**。其全部"立即冻结语义"的小项采纳实施；P2.1-B 引擎本体严格按其批次定义留给下一批。

## 二、本批实施（4 项语义冻结 + 1 个 helper）

| 74 条目 | 实现 |
|---|---|
| §3 设备/卷命名空间 | 新增 `launcher_domain::is_device_namespace()`（`\\.\` 前缀判定）：设备路径**不是文件系统身份**——`normalize_path_identity` 对其原样保留（仅大小写折叠），永不与真实路径身份冲突。关键实现点：设备检查必须在普通折叠**之前**（`\\.\` 的 `.` 段会被空段折叠误吞）。测试 1 项 |
| §11 INV-IDENTITY-003 | 冻结：身份解析必须 PURE——只读元数据/解析/规范化，不得产生或执行 Effect（launch/shell/network/install）。已写入 `executable_identity` 文档与手册 |
| §7 Generation 提交语义 | 核实：现有实现的事务内递增恰好满足"generation = 已提交的索引版本"（失败回滚不涨）。语义入册，并预记 P2.1-B 规则：按 coalesced batch commit 递增，而非每事件 +1 |
| §1 INV-SEARCH-004 措辞升级 | 手册补齐精确表述：单 provider 失败不得压制无关 provider 结果；"targeted provider unavailable" 语义（查询显式定向该 provider 时）随 P2.1-B/Provider v2 落地——挂账不实现 |
| §29/§30 状态入册 | 手册：**P2.1 Foundation Batch 1 = CLOSED**；**P2.1-B Incremental Index Engine = NEXT** |

## 三、后置（与 74 一致）

- **P2.1-B 全部内容**（§12-27）：FileChange contract（事件 ≠ 数据库真相，事件只触发 re-stat）、ReadDirectoryChangesW（同样不跟随 reparse point）、bounded queue（满 → coalesce + mark dirty，绝不阻塞事件线程也不无限增长）、coalescer（按 `normalize_path_identity` 身份合并——这正是先做 Path Identity 的回报）、rebuild/watcher 串行化（rebuild 期间事件保留回放）、DirtyRootSet（路径包含去重，只留最上层）、root 消失降级、WatcherStatus/Health、batch commit、单一 writer。
- §31 的 B1-B12 验收标准与 §32 的确定性随机最终一致性测试：作为 P2.1-B 的完成定义。
- §33 `index-check` 诊断工具（输出 filesystem/index/watcher 三方对照）：其输出依赖 watcher status/health，随 P2.1-B 一起做。
- Typed SearchIdentity、MSIX/UWP/Portable、Ranking 调权、Query Cache、Context ranking、Favorites：维持冻结。

## 四、验证

```text
cargo test --workspace   527 passed / 0 failed（+1：设备命名空间）
cargo build --workspace  zero warnings
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical）
```

## 五、下一步

唯一优先项：**P2.1-B Incremental Index Engine** 专门批次。完成定义 = 74 §31 的 B1-B12 验收 + §32 最终一致性测试 + INV-INDEX-005~010 全部有回归测试。批次之外零改动。
