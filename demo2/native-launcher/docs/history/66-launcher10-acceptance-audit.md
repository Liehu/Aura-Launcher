# 66 — Launcher 1.0 验收材料：真实接线审计 + MUST/SHOULD/DEFERRED/BUG 冻结清单

日期：2026-09-06。对应 64 §39 的要求。配套文件包：`files2/launcher-acceptance.zip`（apps/launcher-app 全部源码 + scripts/check_topology.py + scripts/release_gate.py + Cargo.toml + Cargo.lock）。

本文档回答一个问题：**这些 crate 是不是实际接成了一个完整 Launcher**。以下所有行号以 zip 内当前源码为准，可直接对照。

---

## 一、主链路逐环验证（Hotkey → History）

| 环节 | 接线状态 | 证据（main.rs） |
|---|---|---|
| Hotkey | ✅ 真实 | L1230 `parse_hotkey(cfg.hotkey)` → `GlobalHotkey::spawn`（独立线程 + WM_HOTKEY mpsc）→ 显隐切换（L918/L1225 附近的 toggle：park/park 恢复前台 + `recenter_and_repaint`） |
| Search 入口 | ✅ 真实 | L1032 `ui.on_query_changed` → `spawn_search`（L286）：后台线程 + `SearchSession` query-supersession → `st.core.search(...)` |
| Provider 注册 | ✅ 真实（6 类） | build_core：L65 ContextProvider；L72 AppRegistryProvider（Start Menu .lnk + Uninstall 注册表 + .lnk 目标解析）；L77 RecentFilesProvider；L95 FileProvider + L94 `set_history(indexer)`（SQLite 索引在启动时 rebuild，Documents/Desktop/Downloads + index_dirs）；L138/166 McpProvider（stdio / streamable-http，按 config）；L196 PluginProvider（`<data>/plugins/*/manifest`，Python runtime 解析） |
| Ranking | ✅ 真实 | `core.search` → provider 顺序 fan-out → `launcher_search::rank_with_boost`（词法 + 类型先验 + **usage boost**：频率/新近来自 history 表） |
| UI 呈现 | ✅ 真实 | `launcher_ui::to_result_items` 投影（UI 无 ActionKind/capability 语义，INV-035~037）；选择/面板/确认全部经 `on_selection_changed`/`on_panel_*`/`on_execute*` 回调（L1046–1206） |
| Action | ✅ 真实 | `execute_action_by_id`（L568，stable-id 解析 → Resolver 语义的 context 新鲜度检查 → confirmation 双 Enter 策略）→ `launcher_action::execute`（L622）；`Effect::PluginInvoked` → `core.execute_effect`（L646 附近：`mcp:*`/`plugin:*` 命名空间路由，execution_id 单源透传） |
| History | ✅ 真实（闭环） | L630/L674：system/plugin/MCP 执行**成功后** `record_use_with_title` 落库 → 下次查询 usage boost 生效。第一次 Chrome 排第 3、用完排第 1 的闭环成立（quality 测试覆盖 boost 语义；端到端手动可验） |

结论：**主链路没有任何一环是"只有实现没接线"**。

## 二、Agent / Workflow / MCP / Runtime：注册 vs 仅实现

| 子系统 | 状态 | 事实 |
|---|---|---|
| MCP | ✅ **产品级接线** | 按 `[[mcp.servers]]` 每服务器注册 Provider + executor（L132–166）；stdio 与 2026 streamable-http 双 profile；`persistent` runtime 经 `endpoint.with_runtime` 进入 launcher-mcp 的 RuntimeManager（进程持久化/会话恢复在 executor 内部真实生效） |
| Workflow | 🟡 **半接线** | `workflow_service` + WorkflowRunner 真实运行（runner/failure-policy/confirmation 暂停确认都是真代码），但**唯一触发源是托盘 demo 菜单**（L1006 `on_workflow_demo_requested` → `demo_definition()`）。没有用户可定义/发现的工作流入口 |
| Agent | 🔴 **仅 crate，未接线** | `run_bounded_agent`/`KeywordPlanner`/`LlmPlanner` 在 launcher-ai 内（本轮刚补了 Observation→Replan 闭环与 E2E），但 launcher-app **没有任何调用点**；UI 里的 "Agent" 表面只是 VR-013 的固定假象（visual_scenarios.rs L135/207）。64 评级 C+ 的根本原因属实 |
| Runtime（launcher-runtime crate） | ✅ 间接生效 | app 不直接依赖；launcher-mcp executor 用它做 persistent transport 缓存。对用户可见为 persistent MCP 行为 |

## 三、P2 状态冻结清单（收口后不再扩张）

**BUG（本轮已清零）**：64 的 7 项属实缺陷 + record_use 双插入 + Workflow v0.2 半成品编译错误 + VR 基线错位，全部修复；Release Gate 十 Gate PASS（512 tests / 零警告 / VR 15/15 / S0=S1=S2=0）。

**MUST（Launcher 1.0 出门前必须）**
1. Workflow 触发源产品化：至少"运行已安装工作流"入口（定义放在插件/配置目录），替代 tray-demo-only。
2. Config.settings UI 入口（哪怕只是 hotkey/index_dirs/result_limit 三个字段的编辑面板）——现在改配置只能手编 TOML。
3. Installer/升级：zip + Start Menu 快捷方式 + 升级保配置/索引/插件目录（单实例/自启动/数据目录已就位）。
4. 性能/内存基线：冷启/热启/popup/search p50/p95/idle RSS 各留一份数字（Release Gate G8 已有 soak，缺记录性基线）。

**SHOULD（1.0 后第一波）**
5. Agent 接线为可选开关（catalog+KeywordPlanner 起步，复用现有 budget/observation 闭环）；不接则保持 crate 状态并在文档声明。
6. Recent/历史隐私分级（sensitive command 不 boost）。
7. 增量索引 + ReadDirectoryChangesW watcher（P2-C.2/C.3）。
8. App Identity 归并（.lnk target 已解析，缺跨来源 identity 去重）。

**DEFERRED（明确不做进 1.0）**
9. Canonical Identity 全套新类型（26）；LLM Planner 产品化（需网络/密钥策略）；MCP OAuth 远程；多语言 SDK 矩阵；Marketplace/云同步/Enterprise（64 §34 同意全部不做）。

## 四、验收判定建议

- 主链路（搜索/启动/Action/历史/插件/MCP）：**可以标 1.0-Ready**。
- Workflow：MUST-1 完成后标 Ready（引擎本身已 Ready）。
- Agent：保持"实验性 crate"，从 1.0 宣传面中移除，评级争议即消失。
