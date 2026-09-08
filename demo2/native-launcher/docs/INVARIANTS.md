# INVARIANTS.md — Architecture Invariants

Agent 与开发者的**机器可验证**约束清单。每次改码前后对照检查；带 ✅ 的条目有对应自动化测试或 CI 检查。

| ID | Invariant | 验证方式 |
|---|---|---|
| INV-001 | UI thread MUST NOT perform blocking IO / CPU-heavy work / process waits. | code review + 架构约定（查询/Action 均在后台线程） |
| INV-002 | Core MUST NOT embed Python/Node/WASM runtime or any resident third-party runtime. | dependency review（cargo tree） |
| INV-003 | Plugins MUST NOT access UI internals; they only return Native UI Schema data. | `launcher-ipc::sanitize_results` schema 校验 ✅ |
| INV-004 | All external effects MUST go through the Action Engine; providers/plugins MUST NOT ShellExecute directly. | code review（providers 只产出 Action，执行只在 `launcher-action`） |
| INV-005 | Plugin results MUST be bounded (≤ `MAX_PLUGIN_RESULTS`) and schema-valid. | contract test `flood_is_truncated` ✅ |
| INV-006 | Provider query MUST NOT mutate global state; ranking is deterministic. | `ranking_stays_deterministic_on_corpus` ✅ |
| INV-007 | Plugin crash/hang/malformed output MUST NOT terminate Core. | contract tests（crash/slow/malformed）✅ |
| INV-008 | No unbounded global cache or collection（app/recent/history/index 全部有上限）。 | code review（MAX_APPS / MAX_RECENT / history 10k / MAX_INDEX_ENTRIES） |
| INV-009 | Every public protocol change (plugin RPC, IPC, manifest schema) requires a version bump and ADR. | ADR review |
| INV-010 | Every architecture boundary change requires an ADR first. | ADR review（AGENTS.md 红线） |
| INV-011 | Context capture MUST be read-only, best-effort, and never block or fail the popup. | code review（spawn_context_snapshot 在独立线程） |
| INV-012 | Stale query results MUST NOT overwrite newer results (query supersession). | `search_session_supersedes_stale_queries` ✅（ADR-0004） |
| INV-013 | Plugin executables MUST resolve inside the plugin directory; absolute/`..`/UNC paths MUST be rejected. | `executable_path_confinement` ✅（ADR-0005） |
| INV-014 | Killing a plugin MUST terminate its whole process tree (Job Object). | code review + plugin spawn soak；Windows Job Object（ADR-0005） |
| INV-015 | `workspace.members` / README / ARCHITECTURE 拓扑描述 MUST 一致。 | `scripts/check_topology.py`（CI）✅ |
| INV-016 | UI MUST NOT own a second copy of executable results：UI 只持有 `command-id` + 渲染数据，执行时由 Rust 侧唯一结果集（`AppState::current_results`）按 id 解析 Command/Action。禁止按 index 取结果。 | code review（`on_execute(string)` 按 id 查找；MVP2.1 `last_results` 漂移 bug 的架构修复） |
| INV-017 | `MAX_CONTEXT_SUGGESTIONS`（15）是 Core/展示层的 UX/延迟上限，NOT Provider API 限制：`Provider::query` 可返回任意数量 Command，由 ranking 决定展示。禁止把该常量下沉进 Provider 契约。 | code review（常量定义于 `launcher-core/providers/context.rs` 文档注释） |
| INV-018 | `schema_version`（plugin.json 格式）、`version`（插件自身版本）、`api_version`（SDK/API 契约）、`protocol_version`（IPC wire 协议）四者 MUST 保持语义独立，禁止合并或互换。（review 11 §1；编号避开既有 INV-013） | code review + `version_negotiation_rejects_unsupported_protocol` ✅ |
| INV-019 | Plugin stdout MUST 只承载协议帧（NDJSON JSON-RPC）；诊断日志 MUST 走 stderr。stdout 出现非协议文本即协议违规。 | `stdout_noise_violates_protocol_and_fails_handshake` ✅ |
| INV-020 | 单帧 ≤ `MAX_FRAME_BYTES`（256KB）、结果 ≤ `MAX_PLUGIN_RESULTS`（100 条）；超帧是违规（kill），超条数是截断。 | `oversized_frame_is_a_violation_not_truncated` + `flood_is_truncated` ✅ |
| INV-021 | 一个 PluginHandle 同时最多 1 个 in-flight query；旧 query 的过期结果由 Host 侧按 query_id/supersession 丢弃，不要求插件实现 cancel。 | code review（PluginHandle 同步 request）+ query_id 回显校验 ✅ |
| INV-022 | Legacy compatibility MUST be profile-scoped：裸数组等遗留行为仅对 `schema_version` 缺省的 Legacy Manifest Profile 容忍；v1 Profile 上一律违规。禁止全局兼容开关。 | `v1_manifest_rejects_bare_array_response` ✅（ADR-0007/0008） |
| INV-023 | 杀死插件 MUST 回收整个进程树（含孙进程），以真实行为测试验证而非仅代码审阅。 | `killing_plugin_reaps_whole_process_tree` ✅（ADR-0005/0008） |
| INV-024 | Runtime dispatch 只允许存在于 `launcher-plugin-host::runtime::resolve_launch`；上层编排禁止 runtime 分支。 | code review + `runtime.rs` 单测（ADR-0010） |
| INV-025 | 插件作者侧 SDK crate MUST NOT 依赖 host 内部 crate（core/ui/context/action/plugin-host/indexer）；domain/protocol 公共 crate 除外。 | `scripts/check_topology.py` SDK dependency guard（CI）✅（ADR-0010） |
| INV-026 | 插件提供的 ActionDescriptor 是不可信输入，MUST 经 Action Resolution（type/input/context/capability 校验）成为 ResolvedAction 后才能到达 Action Engine。 | code review（待 COMMAND/ACTION-CONTRACT 实现，ADR 待定；ACTION-CONTRACT §6） |
| INV-027 | Capability monotonicity：`Action.requires` MUST 是 Manifest 声明 capabilities 的子集；action 不得临时申请权限，Host MUST NOT 自动补权限（未声明即 UNSATISFIABLE）。 | `resolve_descriptor_capability_monotonicity` ✅（ADR-0011） |
| INV-028 | `Command.id` MUST 表示稳定逻辑身份（资源/逻辑动作），MUST NOT 含查询实例身份（如 query 序号）。 | code review（同上；COMMAND-CONTRACT §2） |
| INV-029 | Provider identity 为 Host 权威且永远不来自不可信来源（Plugin/AI/MCP/Remote 输出一律忽略）；全局命令身份 = (provider_id, command.id)。 | `acceptance_primary_secondary_and_fault_containment` ✅（ADR-0011） |
| INV-030 | 未知 Action type MUST NOT 被执行。 | code review（同上；ACTION-CONTRACT §6） |
| INV-031 | Action-level fault containment：畸形 Action 的爆炸半径必须最小——未知/非法 secondary action MUST NOT 使包含它的 Command 失效；`actions=[]` 的 Command 为 informational/non-executable。 | `acceptance_primary_secondary_and_fault_containment` ✅（ADR-0011） |
| INV-032 | `requires_context` 表达展示资格（eligibility），不是数据/效果权限（permission）；后者只能用 capability 表达。 | code review（同上；COMMAND-CONTRACT §4） |
| INV-033 | ActionEngine MUST 只接受 ResolvedAction；raw ActionDescriptor 永远到不了 Action Engine。AI/MCP 生成的 descriptor 与插件输出同级不可信。 | `resolve_descriptor_system_types` + host 集成路径 ✅（ADR-0011） |
| INV-034 | ActionResolver MUST 与 UI 无关（位于 domain/action 层纯函数），供 Plugin/AI/Workflow/MCP 共用。 | `resolve_descriptor_system_types` ✅（ADR-0011） |
| INV-035 | UI 是 projection 不是真相源：UI 只拥有 query 文本、selected command/action id、展示状态；禁止持有 provider/command/action 语义或权限状态（`last_results` bug 的永久性架构表述）。 | code review（INV-016 为其执行面；MVP3.1 Action Panel 实现时复查） |
| INV-036 | UI MUST NOT 解析 ActionDescriptor 或自行做 capability 判断：UI 只见 `ActionPresentation`（id/title/enabled/reason）。 | `action_presentation_projection` ✅（ADR-0011/MVP3.1） |
| INV-037 | ActionPanel MUST NOT 直接执行 Effect：一切执行经 ActionEngine（其 `validate` 拒绝 Disabled action，是唯一执行闸门）。 | `disabled_actions_are_never_executable` ✅ |
| INV-038 | Resolver 标记为 Hidden 的 action MUST NOT 可经 UI 执行（hidden 在 host 层已丢弃，永不进入 Command）。 | CAT-007/008 ✅ |
| INV-044 | Plugin-owned actions MUST execute through ActionEngine → PluginBroker；broker 是 Effect executor 不是独立执行网关。 | `mvp4_acceptance::execute_action_rpc_roundtrip` ✅（ADR-0014） |
| INV-045 | Workflow/AI/MCP 只是 ActionProposal 生产者/适配器，MUST NOT 绕过 ActionResolver；不得携带已授权/已确认状态。 | code review（ADR-0014；运行时属 MVP4.1+） |
| INV-046 | Context-bound 执行 MUST 携带 context generation；stale resolution 不得静默执行（执行前比对，失败提示重解析）。 | `execute.stale_context` 路径 + `context_is_stale`（MVP3.2）✅ |
| INV-047 | `plugin.<id>.*` 只解析到 producer 自身（manifest id 段边界绑定），跨插件调用一律拒绝；`plugin.invoke` 隐式必需；身份判断 MUST 复用纯函数 `owns_plugin_namespace`，禁止各路径自行实现字符串前缀判断。 | `owns_plugin_namespace_boundary_matrix` + `resolver_identity_and_capability_binding` + `plugin_action_routing_mvp4` ✅（ADR-0014 Addendum 2） |
| INV-048 | 插件级 action 业务失败是 EffectFailed（进程存活），不是协议违规；kill 仅用于 timeout/malformed/协议违规。 | `execute_action_unknown_action_fails_cleanly` ✅（ADR-0014） |
| INV-049 | Workflow MUST persist Action references/proposals, never trusted ResolvedAction 或可执行 Effect 对象。 | 生效（ADR-0015；实现验收 = CAT-WF-012） |
| INV-050 | Every Workflow ActionInvocation MUST be re-resolved before execution against current capability/policy/context state。 | 生效（ADR-0015；实现验收 = CAT-WF-003/009） |
| INV-051 | `Effect::PluginInvoked` 是执行域/路由分类，MUST NOT 演化为插件业务能力枚举；`system.*`/`plugin.*` 是开放 namespace。 | `owns_plugin_namespace_boundary_matrix` ✅（ADR-0014 Addendum） |
| INV-052 | WorkflowRun MUST NOT 包含可直接执行的 Effect 对象（只保存执行状态）。 | 待实现验收 CAT-WF-011/012（ADR-0015） |
| INV-053 | ActionInvocation 是 producer-agnostic 的：无 Workflow/AI/MCP 身份字段，所有 Producer 复用同一类型。 | 待实现验收（ADR-0015 §1） |
| INV-054 | Retry 产生新 execution_id 且是 Step 级状态；WorkflowRun 无 run 级 Retrying。 | 待实现验收 CAT-WF-005（ADR-0015） |
| INV-055 | ActionReference 解析来源冻结：Popup Results → Fresh Provider Query → CommandNotFound（默认 Stop）；fresh query ≠ 重放用户搜索词。 | 待实现验收（ADR-0015 / WF-006） |
| INV-056 | Inline ActionDescriptor 是持久化声明且 MUST 仅携带 `system.*`（Host-owned）；`plugin.*` 必须走 Reference 路径；Inline 永远作为不可信输入重解析。 | 待实现验收（ADR-0015 Addendum / WF-A2） |
| INV-057 | Workflow 无 Capability Authority、无 Effect Gateway；Failure Policy 只能决定编排行为。 | 待实现验收 CAT-WF-004/010（ADR-0015 / WF-008） |
| INV-058 | `WorkflowStep.input` 是唯一权威执行输入：Inline descriptor 的 input 是声明/默认值，被 step.input 覆盖；禁止第二输入源。 | 待实现验收（ADR-0015 Addendum / WF-A1） |
| INV-059 | `step_id` MUST 在 WorkflowDefinition 内唯一。 | 待实现验收（WF-A4） |
| INV-060 | Reference Resolution 的 fresh query = empty-text discovery（`text=""`，有界 limit）；provider 的 malformed 响应归类 ProtocolViolation，不得归为 CommandNotFound。 | 待实现验收（WF-A3/A5；未实现 discovery 的插件如实降级 CommandNotFound） |
| INV-061 | （保留号：原拟内容与 INV-060 Addendum 合并，未使用。） | — |
| INV-062 | Workflow UI 只表现 Runtime 状态，MUST NOT 执行 workflow 逻辑或持有可执行 Effect。 | 待 UI-CONTRACT UC-007 实现验收（ADR-0017） |
| INV-063 | MCP 来源能力 MUST NOT 需要独立 UI 执行路径；MCP 无专属 UI surface。 | 设计约束（ADR-0014/0017） |
| INV-064 | Context 变更 MUST 经 Core 提供的 Presentation 更新消费；UI 不自主变更当前 Action 选择（v0.1）。 | code review（UI-CONTRACT §10） |
| INV-065 | 失败呈现 MUST 消费分类后的运行时状态；UI MUST NOT 自行推断 FailureClass。 | code review（UI-CONTRACT §15；R5 非颜色通道 = UC-009） |
| INV-066 | MCP Tool 只能产生 ActionProposal，不能直接产生 Effect；MCP ≠ ActionEngine/Effect Gateway/Capability Authority/WorkflowRunner。 | `failure_mapping_table` + adapter 测试 ✅（ADR-0018） |
| INV-067 | MCP Server identity 由 config 拥有（stable/unique）；`server_id`/`tool_name` 等 metadata 绝不成为 authority identity；跨 server 路由经 namespace 天然隔离。 | `route_identity_scoping` + `cross_server_confusion_is_impossible` ✅（ADR-0018） |
| INV-068 | MCP metadata（annotations/description/instructions）只是 Policy 输入，不能授予 capability 或改变 authorization/confirmation。 | adapter 投影不携带 capability 字段（`projection_carries_no_authority`）✅（ADR-0018） |
| INV-069 | MCP raw errors MUST 经 `McpError::failure_class()` 单点映射到统一 FailureClass；Workflow/AI/UI 不得自行从原始错误字符串分类。 | `failure_mapping_table` ✅（ADR-0018） |
| INV-039 | Shortcut 执行 MUST 解析到稳定 action_id，不得直接绑定 Effect。 | CAT-012 ✅（ADR-0013） |
| INV-040 | Shortcut 分发不得绕过 ActionEngine（与面板/Enter 同一执行路径）。 | `execute_action_by_id` 单一路径 code review ✅ |
| INV-041 | Confirmation 是执行策略状态而非 UI 标志；未确认 action MUST 被 ActionEngine 拒绝。 | `unconfirmed_actions_are_refused_until_host_confirms` ✅（ADR-0013） |
| INV-042 | 确认后的 action 仍只能经 ActionEngine 执行（host 确认 = 清除策略标志）。 | 同上 ✅ |
| INV-043 | Action 解析 MUST 绑定 Context generation；过期解析不得静默执行。 | `context_is_stale` 守卫 code review（ADR-0013） |
| INV-044 | Context refresh 由 Core/Resolver 拥有；ActionPanel 只消费新 Presentation。 | code review（ADR-0013） |
| INV-045 | `system.paste` 是普通 Host-owned Effect，必须走与其它 effect 相同的 ActionEngine 路径。 | `resolve_descriptor` 映射 + engine `send_paste` ✅（ADR-0013） |

## Context Failure Matrix（INV-011 的系统化，来源 09-mvp2-0.5 §11）

原则：**Context failure 不得影响 Launcher 本身**（best-effort → 空结果，绝不 panic、绝不阻塞 popup）。

| 场景 | Expected Context | 现状 |
|---|---|---|
| Explorer 正常 | folder | ✅ 已验证 |
| 多 Explorer | foreground 对应窗口 | ✅ 按 fg_hwnd 解析 |
| 非 Explorer 前台 | app only（无 folder，Context Suggestions 退化为空/常规结果） | ✅ 已实现 |
| Explorer COM failure | best effort（空 folder） | ✅ `folder_for_foreground` 失败返回 None |
| Explorer 正在关闭 | empty（同 COM failure 路径） | ✅ best-effort |
| UWP 文件管理器 | TBD | Known Issue |
| Desktop | TBD | Known Issue |
| Save/Open dialog | TBD | Known Issue |
| 权限异常 | best effort | ✅ 捕获失败返回空 |

## Context 生命周期（09-mvp2-0.5 §5/§6）

- Popup Session 开始（hotkey/tray 触发）时 **capture 一次** ContextSnapshot（INV：capture MUST 先于 popup 取焦点）。
- Session 内 snapshot **冻结**：用户切走前台窗口不刷新当前结果；Esc / 新 invocation 才重新 capture。
- Context Suggestions（原 Quick Switch）= Popup Session 的初始结果状态（empty query），属于统一 `Provider → Command → Ranking` 模型，不是独立系统。

## Phase 10 全局安全不变量（INV-AUTH-001..006，评审 46 §13 冻结；来源 MVP4.3 Security Hardening）

这六条脱离 MCP 独立成立，适用于一切现有与未来的 Executor（Plugin / MCP / System / Remote）。

| 编号 | 不变量 | 验证锚点 |
|---|---|---|
| INV-AUTH-001 | 不可执行的 ActionResolution（Hidden/Disabled/Invalid/Denied/Unresolved/ConfirmationRequired/Stale）MUST NOT 到达任何 Effect Executor（executor_call_count = 0）。 | `sec_cap007_denial_zero_executor`、`sec_cap008_other_gates_zero_executor`、`sec_cap001..005` ✅ |
| INV-AUTH-002 | 外部 metadata（MCP annotations/description/schema/result/AI 输出）MUST NOT 增加执行 authority——只能增加信息。 | SEC-META 全组、SEC-AI 全组 ✅ |
| INV-AUTH-003 | 执行身份 MUST 从 Reference 解析到 Effect 执行保持一致（`identity_mismatch` 于 Runner 绑定点校验 `server_id`/`tool_name`；registry 校验 provider/server 一致）。 | `sec_id003_tool_substitution_rejected`、`sec_id002_provider_server_mismatch` ✅ |
| INV-AUTH-004 | Confirmation MUST NOT 授权过期或被替换的 Action——确认的是"当前解析得到的执行意图"，resume 一律 re-resolve。 | `sec_conf004_tool_replacement_during_confirmation`、`sec_conf005_input_mutation_revalidated`、`sec_conf008_resume_rebinds_identity` ✅ |
| INV-AUTH-005 | 每次执行 attempt MUST 恰有一个 execution_id（registry 不二次铸造；retry 必换新 id）。 | `mcp_wf013_retry_new_execution_id`、`mcp_wf009_execution_id_per_execution`、`p004_execution_ids_strictly_monotonic` ✅ |
| INV-AUTH-006 | 协议/传输失败 MUST NOT 被静默转换为成功的业务结果（per-method envelope 语义校验；崩溃/截断按退出状态归类，绝不计入 Timeout 之外的类别）。 | SEC-PROTO-001..012、`compat_stdio_both_profiles_same_call_result` ✅ |

补充冻结（评审 46 §1/§6/§11）：

- **Identity consistency rule**：resolved action 只有在权威执行输入身份与 Reference
  解析身份一致时才可执行（INV-AUTH-003 的契约表述）。
- **Canonical Identity Fields**：`server_id` / `tool_name` 是 Launcher 的规范化身份
  字段——词法受控（无 `/` `\` `:` `..`、无首尾空白、ASCII 可打印、大小写精确）、
  全子系统唯一解释；`McpInvokeInput::from_json` 是其执行边界（SEC-ID-007）。
- **Bounded retry = DoS defense**：retry 上限（默认 2 次总尝试）不只是可靠性策略，
  同时是资源耗尽防御，适用于所有 provider/executor（`sec_resource005`）。
- **Envelope semantics**：每个 MCP RPC method 必须校验 method-specific 结果判别字段
  （initialize→`protocolVersion`；tools/list→`tools`；tools/call→
  `content`/`isError`/`structuredContent`）；serde 结构合法 ≠ 协议语义合法。

### INV-AUTH-007 / INV-AUTH-008（MVP4.4 P0-C，评审 54 §49）

| 编号 | 不变量 | 验证锚点 |
|---|---|---|
| INV-AUTH-007 | 有效的远程凭据（OAuth token）MUST NOT 授予 Launcher 执行 authority——OAuth 只解决"能否访问该 HTTP resource"，`mcp.invoke` capability 仍由 Resolver 独立裁决。 | `p0c_remote_auth_does_not_grant_capability`、SEC-CAP-007 ✅ |
| INV-AUTH-008 | Launcher 执行 authority MUST NOT 换取 MCP 凭据——capability 授予/确认永不产生、注入或暴露 token；Authorization header 只由 `launcher-mcp::auth` 注入。 | `p0c_no_credential_channel_in_authority_chain`、`arch_auth002_config_cannot_hold_credentials`、reserved-header 守卫 ✅ |

配套规则：issuer validation（discovered == returned，mix-up 拒绝）、credential↔issuer
binding（跨 issuer 复用 = 硬错误）、401 refresh-once/retry-once、403 不重试、
SecretString 全格式 redaction、config.toml 禁止任何凭据字段。

### INV-RUNTIME-001..005（MVP4.4 P1-A，评审 56 §20 冻结）

| 编号 | 不变量 | 验证锚点 |
|---|---|---|
| INV-RUNTIME-001 | Runtime persistence MUST NOT grant execution authority——manager 存储的是调用方注入的不透明值（如 transport），不理解协议/权限/Effect。 | `rt_persist_sec_source_level` ✅ |
| INV-RUNTIME-002 | Runtime recovery MUST NOT replay an execution——crash 只触发 evict+respawn 记账，失败 attempt 永不自动重放（Workflow 决定 retry）。 | `p1a_crash_evicts_and_respawns_fresh`、executor `execute_persistent` ✅ |
| INV-RUNTIME-003 | A RuntimeId MUST NOT be used as an ExecutionId——manager 不铸 e-N，execution id 仍由 caller 铸造。 | `rt_persist002_runtime_id_not_execution_id` ✅ |
| INV-RUNTIME-004 | A persistent runtime MUST serialize protocol operations——同一 runtime 同时只允许一个 protocol operation，忙则 `RuntimeBusy` 拒绝（无队列）。 | `rt_persist012_006_busy_reject_and_no_sweep`、`rt_persist013_busy_does_not_queue` ✅ |
| INV-RUNTIME-005 | Runtime cleanup MUST terminate the complete process tree——Job Object 全树回收（P0-A 承接）。 | `rt_persist015_process_tree_reap`、RT-WIN-002 ✅ |

补充：持久化是显式 opt-in（config `runtime = "persistent"`，默认 ephemeral =
runtime-on-demand）；restart 有界 + backoff 槽位；crash 计数跨 stop 存续；
busy runtime 不可被 idle sweep / release 回收。

### INV-MCP-SESSION-001..010（MVP4.4 P1-B，评审 57 §22 冻结）

| 编号 | 不变量 | 验证锚点 |
|---|---|---|
| INV-MCP-SESSION-001 | RuntimeId、ProtocolSessionId、ExecutionId 是三个独立身份（`s-N` / `r-N` / caller-owned `e-N`）。 | `identities_are_distinct`、`rt_persist002` ✅ |
| INV-MCP-SESSION-002 | Persistent runtime 不蕴含有效 protocol session——session 可独立 Invalid/Failed 而 runtime 存活。 | `session_invalid_vs_runtime_alive`、`session003` ✅ |
| INV-MCP-SESSION-003 | Protocol session state 不能授予 execution authority。 | session registry 无 capability/Effect 词汇（ARCH 守卫）✅ |
| INV-MCP-SESSION-004 | Protocol session recovery 永不重放失败的 execution——失败即返回错误，新 session 只服务新 execution。 | `session003` + executor `execute_persistent`（无 replay 路径）✅ |
| INV-MCP-SESSION-005 | 替换 protocol session 不改变既有 ExecutionId（execution id 由 caller 持有，跨 session 稳定）。 | executor 调用方传入的 execution_id 不受 session 更替影响 ✅ |
| INV-MCP-SESSION-006 | 一个 protocol session 同时最多一个 active operation；并发 acquire = Busy，无内部队列。 | `session030_operation_acquire_states` ✅ |
| INV-MCP-SESSION-007 | Protocol failure 可使 session 失效而不使 runtime 失效（business error 保持 Ready；ProtocolViolation 只 invalidate session）。 | `session003_protocol_violation_invalidates_session_keeps_process`（process_start 不变）✅ |
| INV-MCP-SESSION-008 | Runtime shutdown MUST NOT 留下存活的 session registry 条目——`close_for_runtime` + registry purge。 | `session040_close_for_runtime` ✅ |
| INV-MCP-SESSION-009 | Session 复用要求精确 SessionKey 身份（namespace/endpoint/profile 全等，跨 profile/server 不复用）。 | `session_key_exact_identity`、`compat_profile_isolation` ✅ |
| INV-MCP-SESSION-010 | Session persistence = 进程生命周期 + 内存态；应用重启 ⇒ runtime 与 session 消失（无磁盘持久化）。 | 设计约束（ProtocolSession 无持久化路径）✅ |

### INV-WORKFLOW-001..010（MVP4.4 P1-C Workflow v0.2，评审 58 §54 冻结）

| 编号 | 不变量 | 验证锚点 |
|---|---|---|
| INV-WORKFLOW-001 | Variables cannot grant Capability——变量是数据不是权限。 | `wf_sec001_002_identity_templates_rejected` ✅ |
| INV-WORKFLOW-002 | Variables cannot authorize Confirmation——`confirmed` 等 claim 在 proposal/step input 中均为惰性数据。 | `wf_sec003`、AI-MCP-012 ✅ |
| INV-WORKFLOW-003 | Variables cannot construct Effect——proposal 恒为四字段形状。 | `wf_sec003` ✅ |
| INV-WORKFLOW-004 | Variables cannot alter canonical action identity（server_id/tool_name/provider_id/command_id/action_id definition-bound）。 | `wf_sec001_002`、`wf_sec009`、identity_mismatch（Phase 10 A-003）✅ |
| INV-WORKFLOW-005 | Condition evaluation is side-effect free——纯 Value→Result，无 fs/network/process/clock。 | `wf_sec004` ✅ |
| INV-WORKFLOW-006 | FailurePolicy cannot be bypassed by branching——Stop 类失败即使配置 on_failure 也终止。 | `wf_sec006_stop_policy_blocks_branch` ✅ |
| INV-WORKFLOW-007 | Workflow resume must re-resolve the current action——不可复用 stale ResolvedAction。 | MCP-WF-016、stale E2E ✅ |
| INV-WORKFLOW-008 | A failed step must not partially commit variable mutations。 | `wf_sec009_failed_binding_commits_nothing` ✅ |
| INV-WORKFLOW-009 | Branching cannot create implicit execution replay。 | `wf_sec010_cycle_rejected_before_execution` ✅ |
| INV-WORKFLOW-010 | Workflow control flow cannot bypass ActionEngine——Runner 只经 ReferenceResolver→ActionResolver→Engine 进入执行。 | CAT-WF 全套 + 架构守卫 ✅ |

配套（review 58 §36/§46/§58）：模板只允许填充普通 action input（identity 字段
由 definition 静态绑定）；ConditionEvaluator 纯函数无副作用；
WorkflowLimits（max_steps=1000 / max_vars=256 / max_var_bytes=1MiB /
max_expr_depth=32）是求值安全边界而非 runtime quota。
