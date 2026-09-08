# TESTING.md — 测试情况（2026-09-08，1.0 RC1 → GA Closure）

> 以下各节是按时间序积累的覆盖明细；**当前权威基线：623 tests 全绿 /
> zero warnings / topology 通过**（release_gate G01~G12 + 逐条验收见
> `demo2/files2/97-launcher-1.0-final-audit.md` §B）。历史快照数字（62/62 等）
> 保留原样，不代表当前状态。

对应测试基线 `demo2/files2/03-test-plan-v0.1.md`。

## 运行方式

```bash
cargo test --workspace       # 全部单元 + 集成 + 插件契约 + 压测
cargo run -p launcher-bench  # 性能基准（record/check 子命令见 README）
```

## 单元测试（各 crate 内嵌）

| crate | 覆盖 |
|---|---|
| launcher-domain (4) | Command 序列化 roundtrip、QueryContext 规范化（多空白/大小写/中文）、Manifest 解析与校验（必填字段/版本/timeout 范围）、能力声明 allow/deny |
| launcher-search (6) | 空查询 0 分、exact > prefix > contains 层级、大小写不敏感、Unicode/中文、subsequence 模糊、去重/排序/截断、输入顺序无关（确定性） |
| launcher-ipc (6) | 请求/响应 roundtrip、坏 JSON 拒绝、空行拒绝、flood 截断到 100、错误 schema 拒绝、error response roundtrip |
| launcher-indexer (2) | 索引+搜索（含中文文件名、隐藏目录跳过、LIKE 转义、空查询、元数据字段）、结果数上限与 status 计数 |
| launcher-context (3) | 信号采集与置信度、前台窗口缺失、过期判定（含无 meta 快照） |
| launcher-action (3) | 目标缺失校验、路径规范化（空串/NUL）、Copy 效果 |
| launcher-core (9) | Provider 合并与排名、空查询、上下文命令生成、历史写入可选且安全（另有 tests/stress.rs 压测） |
| launcher-config (4) | 缺文件写默认、字段缺失填默认、坏 TOML 回退默认、save/load roundtrip |
| launcher-hotkey (6) | Alt+Space/Ctrl+Space 解析、修饰键任意顺序与大小写、F 功能键、垃圾输入报错、Win 键 |
| launcher-providers (8) | DisplayIcon 解析、去重保留先出现者、真实 Start Menu 枚举、命令携带 Open action、伪造 Recent `.lnk` 扫描、目录缺失报错、空查询、write-read-query 冒烟 |
| launcher-plugin-host (3) | 非法 manifest 拒绝、可执行缺失报 IO 错误、能力检查 |
| launcher-app (1) | autostart 注册表值比较逻辑 |

## 集成测试

### 插件契约测试（`apps/example-testplugins/tests/contract.rs`，6 个）
真实进程级验证，测试插件为同包 5 个 binaries：

| 契约 | 插件行为 | 预期 | 结果 |
|---|---|---|---|
| normal | query → 返回结果 | 结果转换为 Command，provider_id/action 正确 | ✓ |
| slow | 延迟 5s 应答 | Host 在 manifest timeout(1.5s) 超时并 kill，不阻塞 | ✓ |
| crash | 收到请求即 exit(1) | Host 得到错误，**随后重新 spawn 正常工作**（Core 存活） | ✓ |
| malformed | 返回非法 JSON | `PluginError::Malformed` 拒绝 | ✓ |
| flood | 返回 100,000 条 | 截断到 `MAX_PLUGIN_RESULTS`(100) | ✓ |
| soak | spawn→query→kill × 50 | 每轮子进程被回收，无累积 | ✓ |

### Provider 压测（`crates/launcher-core/tests/stress.rs`）
- 100 个 mock Provider × 10 条命令：注册数断言、热身后 20 轮平均查询延迟 < 50ms、200 轮不同查询无退化。

### 独立索引服务（手动/冒烟已验证）
`launcher-indexer-service` stdio JSON-RPC：status / search（8.8 万文件实测）/ rebuild / shutdown / 未知方法 -32601 / 坏 JSON -32700。

## 真实 GUI 端到端冒烟（人工 + 屏幕自动化，release 构建）

- Ctrl+Space（配置驱动）唤起原生 popup ✓
- 输入即时搜索：文件（21.4 万条索引）、应用（Start Menu + Uninstall 注册表来源均命中）、外部插件（echo 进程按需拉起并返回结果）✓
- ↑↓/Enter/Esc 键盘路径、托盘常驻、Esc 后热键可再次唤起 ✓
- 插件安装目录 `%LOCALAPPDATA%\native-launcher\plugins\echo\` 热发现（重启后注册）✓

## 性能与内存（详见 docs/PERFORMANCE.md）

- 搜索基准（`benchmarks/baseline.json`）：app p95 0.88ms（预算 10ms）、file p95 1.17ms（预算 50ms）
- 空闲内存（release）：Private 12.6MB（硬预算 80MB / 目标 50MB 均达标）

## 尚未覆盖（对应测试计划，留待后续）

- 热键→可见窗口延迟埋点（P50/P95 目标 20/35ms）
- 10,000 次 show/hide UI soak 自动化、1000 次 UI 循环无响应检查
- IME 组合输入场景、全屏/锁屏下热键行为
- DB 并发写读、DB 损坏注入、Indexer 进程崩溃重启的自动化故障注入
- Chinese/Pinyin 检索（设计预留，未实现）

## Plugin SDK Conformance（ADR-0010，2026-09-04）

测试分三层，SDK 演进只跑同一套 Test Kit：

| 层 | 内容 | 位置 |
|---|---|---|
| Protocol Conformance | Plugin Contract Test Kit：normal/slow/crash/malformed/flood、握手/版本协商、query_id 回显、manifest profile、stdout 红线、frame 限制、优雅关闭、进程树回收（16+ 项） | `apps/example-testplugins/tests/contract.rs` |
| SDK Conformance | 每个 SDK 用真实 PluginHost 跑契约链：Rust calculator E2E、Python SDK E2E（`LAUNCHER_PYTHON` 可指定解释器） | `apps/calculator-plugin/tests/e2e.rs`、`crates/launcher-plugin-host/tests/python_sdk_e2e.rs` |
| Reference Plugins | calculator（Rust，Canonical Reference，协议变更硬门禁）+ calculator（Python，跨语言对照） | `apps/calculator-plugin`、`plugins/python/example-calculator` |

Runtime 解析测试：`crates/launcher-plugin-host/src/runtime.rs`（process/python 计划、env 展开、未知 runtime 拒绝）。

## Domain Contract Test Kit（ADR-0011，review 17 §16）

Command/Action Contract v0.1 的 conformance 套件，与 Plugin Contract Test Kit 并列；所有 SDK 以后须同时通过两套。

| 用例 | 语义 | 状态 |
|---|---|---|
| CAT-001 | command.id 会话内稳定、不含查询实例身份 | ✅ |
| CAT-002 | provider_id Host 权威（INV-029） | ✅ |
| CAT-003 | resolver 保持插件 action 声明顺序 | ✅ |
| CAT-004 | primary = 第一个 Ready action | ✅ |
| CAT-005 | capability monotonicity + 组合 fallback（Copy 被拒→History 成 primary） | ✅ |
| CAT-006 | requires_context eligibility | ⏸ context supply 未实现，语义已冻结 |
| CAT-007 | unknown secondary 隔离（INV-031） | ✅ |
| CAT-008 | unknown type 永不执行（INV-030） | ✅ |
| CAT-009 | input 校验失败不执行 | ✅ |
| CAT-010 | `actions=[]` → informational command | ✅ |
| CAT-011 | ResolvedAction 边界：engine 只见合法 domain Action（INV-033） | ✅ |

位置：`apps/calculator-plus/tests/domain_contract_kit.rs`。

## AI Planner 验收（MVP4.2，ADR-0016）

`crates/launcher-workflow/tests/ai_planner.rs`（6 项）：planner 只提案 Ready catalog 匹配；伪造 `authorized/confirmed/granted_capabilities/trust_level` 反序列化即丢弃；内嵌 ResolvedAction 拒绝；缺路由字段拒绝；proposal 经冻结编排路径执行（含 StaleContext→ReResolve）；未知 command → CommandNotFound 且引擎零执行。真实链路：`apps/calculator-plus/tests/ai_planner_e2e.rs`。

## UI Presentation 收尾（MVP4.1 UI，review 31 顺序）

- F2/F6：selected ▸ 非颜色标记 + `selection-changed(command_id)` 身份信号。
- F3/F4：Main.Searching/Error 状态行 + Context Hint 底栏（`context-hint`，Core 派生）。
- F5/Workflow Surface：`WorkflowRunView/WorkflowStepView` 呈现模型（`to_workflow_run_view`）、Slint Runtime Surface（Enter=Confirm / Esc=Keep paused）、`show/hide_workflow_run`。应用内触发集成待 workflow 触发功能。

## Workflow 触发集成（MVP4.1 closeout，review 32）

`apps/launcher-app/src/workflow_service.rs`：最小触发链（托盘菜单触发 → 服务层创建 WorkflowRun → 后台 runner → Runtime Surface 实时进度 → Paused 等待确认通道 → re-resolve 继续）。演示定义含 confirmation 步骤，全生命周期可观测。通用 Trigger Framework（hotkey/plugin/AI/schedule 触发源）按 review 32 明确推迟。

## Visual Regression（Spec 实现顺序 ⑧，review 39 十状态基线）

- `LAUNCHER_SNAPSHOT_DIR=<dir>`：依次应用 10 个冻结状态（VR-001~VR-010）并以 GDI BitBlt 捕获 640×420（逻辑）/ Dark / 100% client 区为 BMP。
- 覆盖：Main Empty / Results / Selected / Error；Action Normal / Disabled / Confirmation；Workflow Running / Paused(ConfirmationRequired) / Failed(含 Level 3 诊断行)。
- 基线比对：对同目录两次运行做逐字节 diff（BMP 无压缩，确定性渲染下应完全一致）；不一致即视觉回归。
- 单张模式保留：`LAUNCHER_SNAPSHOT=<path.bmp>`（确定性 demo 数据单帧）。

场景定义：`apps/launcher-app/src/visual_scenarios.rs`；捕获：`workflow_service`/`snapshot.rs`（GDI BitBlt）。已实测 10/10 写出。

## MCP Adapter 测试（MVP4.3 Phase 1-4，ADR-0018）

- 协议/身份/目录/投影/失败映射单测：`crates/launcher-mcp`（13 项：DTO roundtrip、route identity scoping、cross-server confusion、catalog discovery/refresh、投影、failure mapping）。
- 集成 E2E：`apps/example-mcp-server/tests/mcp_provider_e2e.rs`（6 项，经真实 stdio server 进程：empty-query discovery / route identity / pre-Phase-7 disabled / 文本过滤 / 不可用 server 报错 / fixture 存在性）。

## MVP4.3 Release Gate（Phase 12，评审 48）

统一命令：

```bash
python scripts/release_gate.py            # 全量（含 visual baseline diff）
python scripts/release_gate.py --quick    # 跳过 visual/soak/stress
python scripts/release_gate.py --skip-visual
```

九个 Gate：build 零警告 → workspace 全测试 → topology → 架构守卫（ARCH/INV-COMPAT
源码级）→ 安全矩阵（S0/S1/S2=0 零容忍）→ 兼容矩阵（2025 + 2026 profile）→
真实 MCP E2E（Scenario A–H）→ Visual Regression（VR-001..010 逐字节 diff，
baseline 首跑建立，更新须 documented reason + review）→ 性能/soak/资源边界
（stress + quality + MCP 100×2 profile 循环）→ 文档一致性。
配置迁移 Gate 内嵌于 `launcher-config` 单测（`release_config_*`）。

产物（`artifacts/mvp4.3/`）：`test/cargo-test.txt`、`test/build.txt`、
`visual/{baseline,current}/VR-*.bmp`、`MVP4.3-RELEASE-MANIFEST.json`、
`MVP4.3-RELEASE-REPORT.md`。最终输出只有两种：`RELEASE GATE: PASS` 或
`BLOCKED`（S0/S1/S2、架构、功能、协议、topology、零警告、visual 任一失败即 BLOCK；
deferred feature 属 backlog 不 BLOCK，但必须 documented）。

已实测（RC）：355 tests 全绿、零警告、topology PASS、S0/S1/S2=0、
10/10 VR byte-for-byte identical（重跑可复现）、MCP soak 100×2 profile 无失败。
