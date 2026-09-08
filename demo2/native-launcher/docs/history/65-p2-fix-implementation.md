# 65 — 对 64（MVP Review）的核实与修复实施记录

日期：2026-09-06。基线：512 tests / 零警告 / topology 通过 / MVP4.3 Release Gate 十项全 PASS。

---

## 一、评审结论核实（64 所列问题的真伪判定）

64 是对 crates.zip 的评审，部分结论基于旧快照。逐项到当前源码核实：

| 64 的问题 | 核实结果 | 本批动作 |
|---|---|---|
| §2 Plugin ExecutionId 重新铸造（P1-FIX-01） | ✅ **属实**：`PluginHandle::execute_action` 用自己的 `EXEC_SEQ` mint，`Core::execute_effect` 的 execution_id 在 plugin 分支被丢弃 | ✅ 已修 |
| §3 Agent replans 计数两次（P1-FIX-02） | ✅ **属实**：`replans += 1` 出现两次，与 `BudgetController` 漂移 | ✅ 已修 |
| §4/§5 Agent 无 Observation 闭环（P1-FIX-03） | ✅ **属实**：`run_bounded_agent` 只调 `planner.plan(goal, catalog)`，失败结果从未回到 planner | ✅ 已修（最小闭环） |
| §6 Agent Confirmation 闭环未接线（P1-FIX-04） | ⚠️ 属实但**本批不修**（见下"后置"） | ⏳ 后置 |
| §8 Search 无 dedup（P2-FIX-02） | ✅ **属实**：注释承诺 dedup，代码从未实现；且旧测试用不同 (provider,id) 无法证明 | ✅ 已修+测试 |
| §9 Provider 候选集截断过早 | ✅ 属实：`MAX_FILE_HITS=30` → 全局 Top-K | ✅ 提至 100 |
| §10 无路径搜索（P2-FIX-04） | ✅ 属实：只 LIKE name | ✅ 已修+测试 |
| §11 Indexer 全局上限失效（P2-FIX-01） | ✅ **属实**：`count` 是每层递归局部变量，N 个分支可扫 N×500k | ✅ 已修+测试 |
| §12 junction 防护是空壳 | ✅ **属实**：`is_junction_like` 恒 false | ✅ 已修（REPARSE_POINT 检测，默认不跟随） |
| §13 record_use 双插入（P2-FIX-03） | ✅ **属实**（上一批引入的回归）：INSERT + delegate 各插一条 | ✅ 已修 |
| §17 Recent 按名字排序 | ✅ **属实** | ✅ 改为按 .lnk mtime 降序 |
| §20/§21 Config 直接覆盖写 | ✅ 属实 | ✅ 原子写（temp+flush+rename）；invalid config 隔离为 `config.invalid-<ts>` 并写默认，不再静默丢失 |
| §15/§16 .lnk 无目标解析 → RunAsAdmin 对 Start Menu 永不出现 | ✅ 属实 | ✅ 目录扫描时解析 .lnk target，`resolved_target` 为 .exe 时提供 RunAsAdmin |
| §19 PluginProvider 身份别扭、§26 Canonical Identity、§30/§31 增量索引/Watcher、§14 History 隐私分级、§24/§25 评测语料 v2、§28 产品级 E2E、§29 内存基线、Installer | 真实缺口，但都是**功能扩张**而非缺陷 | ⏳ 按 64 §36 的 P2.1→P2.10 排序另行排期 |

## 二、修复实施明细

1. **P1-FIX-01（ExecutionId 单源）**：`Provider::execute_action` 与 `Core::execute_plugin_action` 增加 `execution_id` 参数；`Core::execute_effect` plugin 分支透传调用方铸造的 id（"registry never re-mints" 落地）；`PluginHandle::execute_action` 改收调用方 id 并删除 `EXEC_SEQ` 铸造点；launcher-app 与 calculator-plus 的 workflow backend 在 attempt 边界用 `core.next_execution_id()` 铸造一次并随 `ExecutionOutcome` 返回。echo 校验不变。
2. **P1-FIX-02**：删除本地双计数，`AgentRunOutcome.replans` 直接读 `budget.replans()`。
3. **P1-FIX-03（最小闭环）**：`ActionPlanner` 新增 `replan(input, catalog, &ReplanContext)`（默认实现 = `plan()` 过滤掉上一轮失败的 (command_id, action_id)）；`run_bounded_agent` 在失败后构造 `ReplanContext` 并走 `replan`，LLM planner 可 override。新增 `tests/agent_loop_e2e.rs`：断言 turns=2 / replans=1 / attempts=3 / 失败可见于 observation / E1≠E2 / 固执 planner 不会无限重提失败项。
4. **P2-FIX-01**：`scan_into` 改为共享 `remaining: &mut i64` 预算，rebuild 全局 ≤ `MAX_INDEX_ENTRIES`。
5. **P2-FIX-02**：`rank_with_boost` 排序后按 `(provider_id, id)` 真实 dedup（保留最高分）。
6. **P2-FIX-03**：`record_use` 仅委托 `record_use_titled(…, "")`。
7. **P2-FIX-04**：indexer `search` 改为 name OR path 匹配，name 命中优先排序；`MAX_FILE_HITS` 30→100。

## 三、有意后置（理由）

- **P1-FIX-04（Agent Confirmation/Resume E2E）**：需要 UI/host 侧的 WaitingForConfirmation→Resume 状态机接线，是跨 crate 的语义工作；且按 60 号文档的优先级，Agent 属"可选增强"。已在 65 号文档挂账，不阻塞 Launcher 1.0 核心。
- **增量索引/Watcher、App Identity 2.0、Settings 系统、Installer、内存基线、评测语料 v2**：均为 64 §32-33 的扩张项，按 §36 顺序进入 P2.2+，避免在同一批混入行为性大改。

## 四、验证

```text
cargo test --workspace   512 passed / 0 failed（新增 agent E2E 2 项、dedup 1 项、path/budget 2 项）
cargo build --workspace  zero warnings
check_topology.py        PASS（17 crates + 8 apps）
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical、S0=S1=S2=0）
```
