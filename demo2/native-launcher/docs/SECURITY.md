# SECURITY.md

## 信任模型（v0.1.x，依据 05-mvp2-advice 评审）

插件是**不可信代码执行环境**，不只是"防把 Core 弄崩"。信任分级（签名机制为 v0.2 预留，当前全部按 Local User Plugin 处理）：

```text
Trusted Builtin（编译进 Core 的 Provider）
  ↓ （未来）Signed Plugin（带签名校验）
Local User Plugin（当前唯一外部形态：%LOCALAPPDATA%\native-launcher\plugins\）
  ↓ （未来）Untrusted Plugin（需要 WASM 沙箱才可启用）
```

## 隔离边界（已实现）

| 机制 | 实现 |
|---|---|
| 进程隔离 | 每个插件独立进程（Tier 2），崩溃/挂起不波及 Core |
| 进程树隔离 | Windows **Job Object**（KILL_ON_JOB_CLOSE）：kill 插件时孙进程一并回收，不留孤儿（ADR-0005） |
| 超时 | manifest `timeout_ms`（Host 校验 ≤60s），超时 kill |
| 结果有界 | `sanitize_results` schema 校验 + 截断到 100 条 |
| **可执行路径约束** | `resolve_executable`：仅允许插件目录内的相对路径；绝对路径、`..`、UNC（`\\server\...`）拒绝；存在性文件经 canonicalize 防 symlink/junction 逃逸（ADR-0005） |

### 子进程环境（当前策略）
- `current_dir` = 插件目录（不泄漏宿主 CWD）
- 环境变量：继承父进程（MVP 简化；已知妥协——后续引入网络/任意能力插件时改为最小环境注入，见 ADR-0005 "重新评估条件"）
- stdio：stdin/stdout 为协议通道，stderr 丢弃

## Capability 模型（ADR-0006 规划）

当前：manifest 声明 + Host allow/deny 查询（声明即全部）。目标链路：

```text
Manifest(Declared) → Host Policy(Granted) → Runtime Enforcement(每次能力调用 Broker 检查 ALLOW/DENY)
```

运行期强制（如 `network.request()` 经 Broker 检查）在插件 RPC 中新增能力调用方法后实施；在此之前 capability 仅是元数据——这一点已作为已知限制记录在 ADR-0006。

## 输入处理

- 所有路径经 `launcher_action::normalize_path`（拒绝空串/NUL）后才执行
- Indexer LIKE 查询转义（`%`、`_`、`\`）
- SQLite 全程 prepared statements
- 插件请求解析：坏 JSON 回 `-32700` 并继续服务（不 panic）

## unsafe 清单（全部有论证）

| 位置 | 论证 |
|---|---|
| `launcher-action` ShellExecuteW | Win32 shell API 天然 unsafe；所有打开行为的唯一入口 |
| `launcher-app` foreground::take_foreground | 热键唤起后的前台修复；代码内注释说明 |
| `launcher-plugin-host/src/job.rs` | Job Object 创建/赋值/终止；仅操作本模块创建的句柄 |
| `launcher-context/windows_source` | GetForegroundWindow/OpenProcess 只读查询，PROCESS_QUERY_LIMITED_INFORMATION |

## MVP4.3 Phase 10 — MCP Security Hardening（已封版，评审 46）

Phase 10（对抗验证）结论：面对恶意 MCP Server / Tool Metadata / Tool Result /
AI Proposal / 协议输入，authority 边界不可穿透。要点：

- **威胁模型**：MCP server、tool name/description/annotations/inputSchema、
  tool result、structuredContent、server metadata、AI proposal 全部 UNTRUSTED；
  唯一 authority 路径 = Host config → policy → ActionResolver → ActionEngine →
  Effect routing。
- **总原则**：No external artifact may increase authority——外部世界只能增加
  信息（DATA → PROPOSAL），不能改变权限；authority wall 之后只有 Resolver/Engine。
- **关键修复**：tool substitution（Runner 绑定点 `identity_mismatch` 校验）、
  cross-server mismatch → ProtocolViolation、tools/call 信封语义校验、
  有界帧读取（256KB 分配边界防护）、崩溃/截断与 Timeout 分类分离、
  canonical identity 字段词法约束。
- **全量矩阵**：SEC-ID / SEC-META / SEC-CAP / SEC-CONF / SEC-PROTO / SEC-AI /
  SEC-CD / SEC-RESOURCE / SEC-ARCH（约 80 case，含 fuzz 语料与 property 测试），
  位于 `crates/launcher-core/tests/security*` 与
  `crates/launcher-mcp/tests/security_transport.rs`。
- **全局不变量**：INV-AUTH-001..006（见 INVARIANTS.md）。

## Phase 11 — Compatibility

协议/传输兼容性隔离在 launcher-mcp（`McpProtocolProfile` 双 profile、2026
stateless、`server/discover`、缓存元数据、确定性排序、错误码矩阵）；契约见
`docs/MCP-COMPATIBILITY.md`。Phase 10 安全不变量对每个 profile 重放成立
（compat contract rule 10）。Streamable HTTP 与多语言 SDK interop 为
Phase 11 backlog（落位时 auth 设计进 `launcher-mcp::auth`，绝不进 domain）。
