# MCP COMPATIBILITY CONTRACT v0.1

> 冻结于 MVP4.3 Phase 11（评审 47）。兼容性被隔离在 `launcher-mcp`；
> Domain / Workflow / Action / AI Planner 永远不感知协议版本。

## 1. 兼容契约（十条，冻结）

1. **Wire differences are isolated to launcher-mcp.**
2. **Normalized `McpTool` is protocol-version independent.**
3. **Provider does not inspect protocol version.**
4. **Workflow does not inspect protocol version.**
5. **ActionResolver does not inspect protocol version.**
6. **AI Planner does not inspect protocol version.**
7. **Effect Executor consumes normalized invocation.**
8. **Unsupported extension never grants authority.**
9. **Unknown protocol feature fails closed or is ignored according to profile.**
10. **Phase 10 security invariants hold for every profile.**

## 2. Profile Matrix（§4）

| Profile | initialize | session | tools/list | tools/call | server/discover | 实现 |
|---|---|---|---|---|---|---|
| `2025-06-18`（legacy，默认） | ✅ | stateful handshake | ✅ | ✅ | ❌ fails closed | `StdioTransport`（Phase 1–10 已安全加固） |
| `2026-07-28`（current） | ❌（no-op） | stateless，`_meta` 自描述 | ✅（`ttlMs` 缓存提示） | ✅ | ✅ | `StdioTransport` + `profile=V2026_07_28` |
| Unknown | — | — | — | — | — | **fail closed**：config 加载即 WARN + 跳过该 server |

约定：新版本追加枚举变体（`V2027_…`），绝不重解释既有变体。
Profile 类型只存在于 `launcher-mcp::compat`；config 携带字符串，由 host 解析。

## 3. Transport Matrix（§5/§16）

| Transport | 状态 |
|---|---|
| stdio（双 profile） | ✅ REQUIRED，已实现 |
| Streamable HTTP（2026，P0-B） | ✅ `StreamableHttpTransport`（ureq 隔离在 launcher-mcp；redirect 禁用；证书校验开启；响应 1MiB / 请求 256KB 有界；URL policy：loopback HTTP ✅ / 私网与公网明文需显式 opt-in / metadata endpoint 永拒） |
| legacy HTTP+SSE | 🟡 compatibility-only，未排期 |
| WebSocket / custom | ❌ |

HTTP 实现约束（已生效）：独立于 stdio、无 MCP session 状态（stateless，每请求
`_meta` 自描述 + `MCP-Protocol-Version`/`Mcp-Method`/`Mcp-Name` header 与 body
同源生成并一致性校验，reserved headers 不可被自定义 header 覆盖）；`text/event-stream`
P0-B safe-reject；401/403 保留为 transport 级 auth 边界（FailureClass 决策留 P0-C）。
INV-TRANSPORT-001/002：transport 实现不得改变授权语义；executor 不按 transport
类型分支（stdio/HTTP 归一化等价由 `cross_transport_equivalence` E2E 证明）。

## 4. 归一化边界（§6/§27）

```text
MCP Wire（2025 stdio / 2026 stdio / 未来 2026 HTTP）
        ↓ Protocol Adapter（launcher-mcp）
Normalized McpTool { server_id, name, title, description, input_schema, annotations }
        ↓ 既有投影（adapter → Command → ActionCatalogItem）
Identical Command / ActionProposal / WorkflowAction::Reference /
ActionResolver 行为 / Effect 类型 / FailureClass 语义
```

同一定义：MCP 版本可以换、Transport 可以换、SDK 可以换，
`ActionProposal`、`WorkflowAction::Reference`、authority model 均不变。

Effect 语义同样 profile 无关：`plugin.mcp.invoke`（routing identity）、
`mcp.invoke` capability、execution_id 归属（caller per-attempt 铸造）在两个
profile 下完全一致。

## 5. Schema / Result / Error（§12–§15）

- **Schema**：完整 JSON Schema 2020-12（oneOf/anyOf/allOf/if-then-else/$defs/内部 $ref）
  作为 DATA 原样存储、永不求值；外部 `$ref` 不 dereference；深度由 JSON 解析器
  递归上限约束（SEC-META-006/007 已验证）。
- **Result**：`content` / `structuredContent`（任意 JSON）/ `isError` 三通道独立保留；
  `isError` = BusinessError（业务失败，server 存活）；信封必须含至少一个结果判别字段
  （E-003 hardening），否则 ProtocolViolation。
- **Error code matrix**（`McpError::from_jsonrpc_error`，单点映射）：
  `-32601`→CommandNotFound；`-32602`→InvalidInput（2026 将 resource-not-found 并入此码）；
  `-32700/-32600`→ProtocolViolation；其余（含未知码）→BusinessError，fail closed。

## 6. Cache / Ordering / Extensions（§10/§11/§21/§22）

- `ttlMs`/`cacheScope` 是 **Discovery 侧元数据**（`CacheMetadata`，默认
  Server-scoped、无 TTL——fail closed），MUST NOT 进入 ActionProposal /
  WorkflowStep / Effect。
- Catalog 刷新做确定性归一化（按 tool name 排序）；同一 server + 同一
  catalog state 永远产出相同顺序；不改变 identity。
- 未知扩展：parse → 忽略 → 永不授予权限、永不 crash。Tasks / MCP Apps /
  Resources / Prompts / Sampling / Elicitation 均为 **unsupported（safely
  ignored）**，属 extension backlog。

## 7. Security Replay（§28）

每个 profile 必须重放 Phase 10 核心不变量（不需要复制全部 73 个测试）。
已验证：`sec_crossprofile_identity_invariants_on_2026`（2026 路径上的
server mismatch / oversized input / 诚实调用），加上 profile 无关的
executor 输入校验与 registry 绑定校验天然覆盖其余矩阵。

## 8. Interop 目标（§23/§24，backlog）

Phase 11 已完成 self-fixture 双 profile 互操作。剩余 interop 矩阵
（Python/TypeScript/Go/C# 官方 SDK server、恶意 fixture、Streamable HTTP
server）需要外部 SDK/网络环境，作为 Phase 11 backlog / Phase 12 前置项。

## 9. 测试索引

| 组 | 位置 |
|---|---|
| COMPAT-PROFILE / CACHE / ERROR / RESULT / EXT / ORDER | `crates/launcher-mcp/tests/compat_profile.rs` |
| COMPAT-STDIO（2026 stateless、_meta 强制、discover 分裂、跨 profile 归一化/执行/安全重放） | `apps/example-mcp-server/tests/compat_e2e.rs` |
| 既有 2025 全量回归 | Phase 1–10 全部测试（329+）保持绿色 |
