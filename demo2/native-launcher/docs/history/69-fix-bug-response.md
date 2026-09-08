# 69 — 对 68（fix-bug）的核实与修复实施记录

日期：2026-09-06。基线：514 tests / 零警告 / Release Gate 十项 PASS。

---

## 一、评审主张核实（关键分歧 + 事实表）

### FIX-01/02 "Action 主链绕过 ActionEngine" —— **前提不成立，拒绝按其重构**

评审者把 `launcher_action::execute()` 称为"低层 executor"，认为宿主应该改调"Core/ActionEngine"。但按手册冻结定义（§2 表格）与 ADR-0014，**`launcher-action` 这个 crate 本身就是 ActionEngine**：`validate()`（disabled/confirmation 单一闸门，INV-041）+ `execute()`（Effect 唯一路径）就是引擎的公共 API。宿主调用它**就是**调用引擎；Workflow backend 同样；`Effect::PluginInvoked` 之后由 Core 路由到 broker 正是 ADR-0014 的原文设计（"broker is an effect executor under the engine"）。项目里不存在第二条更低层的执行路径，也不存在"Core 级引擎"可供改调。

因此：**不做** `Core::execute_resolved_action` 包装，**不立** INV-AUTH-007（禁宿主调 `launcher_action::execute`——那等于禁用引擎本身），**不做** Execution Gateway Convergence 大重构。

评审背后的合理关切（execution lineage 不统一）已采纳为 **FIX-02-lite**：UI system attempt 现在也铸造 `execution_id` 并在 attempt.begin 追踪，与 plugin/mcp/workflow 的 id 同源（`core.next_execution_id`）。这与 MVP3.2 冻结的"confirmation 是 host-owned policy（INV-041）"一致，无语义变更。

### 其余主张核实表

| 68 的问题 | 核实 | 动作 |
|---|---|---|
| FIX-03 Mutex HANDLE 函数返回即 Drop，单实例不成立 | ✅ **属实**（真实 bug：句柄关闭 → 内核销毁 mutex → 第二实例可再建） | ✅ 已修：`SingleInstanceGuard` 持句柄至 main 结束 |
| FIX-04 `start_menu_entries_from_existing()` 丢 resolved_target | ✅ **属实**，且该路径与 `AppRegistryProvider::collect_entries()` **重复扫描同一批 Start Menu 目录** | ✅ 已修：删除冗余合并路径，collect_entries() 为唯一扫描器（元数据全保留，RunAsAdmin 不再丢失，启动还少一遍扫描） |
| FIX-05 启动同步 rebuild 阻塞"秒开" | ✅ **属实**（build_core 在 UI 之前全量重建） | ✅ 已修：UI/查询立即用现有索引，全量重建转后台连接（WAL + busy_timeout 5s）；**重建线程在所有 DB 连接打开之后才启动**（首轮实现曾因 schema-init 写锁与后台写事务竞争导致启动失败，已修并经 VR 快照回归验证） |
| §15 `def.validate().expect` panic | ✅ 属实 | ✅ 已修：无效定义 → error 日志 + 放弃本次运行 |
| §23 release_gate "同一测试套件连续执行两次" | ❌ **不属实**（G5/G6 套件各不相同，无重复） | 拒绝；Gate 未改结构 |
| §23/§29 Gate 命名旧 + VR baseline 首跑自动接受 | ✅ 部分属实 | ✅ 已加 `--require-baseline`：release CI 必须显式钉基线；开发环境保持 bootstrap。18-Gate 重写**后置**（现 Gate 十项内容仍与产品相符，重写属扩张） |
| §26/27 源码 guard 禁 `launcher_action::execute` | ❌ 前提同 FIX-01 | 拒绝（会禁掉引擎本身） |
| §9 第二实例激活已有窗口 | 真实需求 | ⏳ 后置（评审自己也说可后置） |
| §17/18 增量索引/Watcher 升 MUST | 方向认可 | ⏳ 已在 66 清单 SHOULD；本轮完成其中"启动不阻塞"的最关键一步，watcher 仍后置 |
| §13 Confirmation 两套 | 设计如此（host-owned UI 确认 + Workflow 暂停确认是两个层级） | 不改；统一引擎级 confirmation 属 P1-D 范畴 |

## 二、验证

```text
cargo test --workspace   514 passed / 0 failed
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py          十 Gate 全 PASS（G7 15/15 byte-identical）
后台重建验证             日志出现 "index rebuilt (background)"，启动路径无扫描
```

## 三、结论

采纳 3 个真实 bug 修复（Mutex 生命周期 / 重复扫描丢元数据 / 启动阻塞重建）+ 2 个低风险收口（execution lineage、workflow 校验防 panic、VR CI 旗标）；**拒绝**基于误读的网关重构与其派生的 invariant/guard 提案，理由如上并存档。项目保持 66 号审计定义的 1.0 冻结面，不重开扩张。
