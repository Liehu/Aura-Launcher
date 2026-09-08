# MVP4.4 P0-B — Streamable HTTP

**Status:** IMPLEMENTATION BASELINE  
**Scope:** Transport + HTTP security only  
**Prerequisite:** MVP4.4 P0-A RuntimeSupervisor  
**Target Profile:** MCP 2026-07-28  
**Compatibility:** 2025 legacy profile remains unchanged

---

## 1. 目标

P0-B 的唯一职责：

> 为现有 `McpExecutor` 增加 Streamable HTTP Transport，实现 MCP 2026-07-28 的 HTTP wire compatibility，同时不改变上层执行模型。

最终：

```text
ActionEngine
    ↓
Effect("plugin.mcp.invoke")
    ↓
McpExecutor
    ↓
McpTransport
    ├── StdioTransport
    └── StreamableHttpTransport
              ↓
          HTTP client
              ↓
          MCP Server
```

P0-B 不允许改变：

```text
ActionProposal
WorkflowAction
ActionResolver
ActionEngine
Effect
McpExecutor API semantics
AI Planner
RuntimeSupervisor
```

---

# 2. 当前与目标架构

当前：

```text
McpExecutor
    ↓
McpTransport
    ↓
StdioTransport
    ↓
ProcessSession
    ↓
launcher-runtime
```

P0-B：

```text
McpExecutor
    ↓
McpTransport
    ├── StdioTransport
    │      ↓
    │  ProcessSession
    │      ↓
    │ launcher-runtime
    │
    └── StreamableHttpTransport
           ↓
        HTTP Client
           ↓
        MCP Server
```

HTTP Transport 不依赖 `launcher-runtime`。

这是关键：

> `launcher-runtime` 是 External Process Mechanics，不是所有 MCP Transport 的父类。

---

# 3. Protocol Profile

继续使用：

```rust
enum McpProtocolProfile {
    V2025_06_18,
    V2026_07_28,
}
```

但：

```text
2025
    ↓
legacy handshake/session behavior

2026
    ↓
stateless Streamable HTTP
```

2026 profile 不发送：

```text
initialize
notifications/initialized
Mcp-Session-Id
```

现代 MCP 请求通过 `_meta` 自描述，`server/discover` 可以用于前置 capability discovery，但并非每个请求都必须先建立协议 session。

---

# 4. HTTP Transport API

保持现有 `McpTransport` 抽象。

不要新增：

```rust
McpExecutor::call_http(...)
```

不要：

```rust
McpExecutor::call_remote(...)
```

而是：

```rust
McpExecutor
    ↓
McpTransport::call_tool(...)
```

Transport 选择：

```text
Configured Server
    ↓
transport = stdio
    → StdioTransport

transport = streamable_http
    → StreamableHttpTransport
```

---

# 5. Server Configuration

扩展：

```toml
[[mcp.servers]]
id = "github"
transport = "stdio"
program = "github-mcp-server"
args = ["stdio"]
```

为 HTTP 增加：

```toml
[[mcp.servers]]
id = "remote-github"
transport = "streamable-http"
url = "https://example.com/mcp"
```

v0.2 P0-B 配置：

```text
stdio
    program + args

streamable-http
    url
```

禁止：

```toml
token = "..."
authorization = "Bearer ..."
password = "..."
client_secret = "..."
```

认证留给 P0-C。

---

# 6. URL Boundary

`StreamableHttpTransport` 只接受：

```text
https://
http://
```

其中：

```text
https://
    production

http://
    development / localhost policy
```

建议默认：

```text
http://
    只允许 loopback
```

例如：

```text
http://127.0.0.1
http://localhost
```

不要默认允许：

```text
http://10.x.x.x
http://172.16.x.x
http://192.168.x.x
```

除非用户显式配置。

P0-B 不实现 SSRF “万能防护”，但至少必须拥有明确的 URL policy boundary。

---

# 7. HTTP Client

P0-B 可以引入一个最小 HTTP client dependency，但必须隔离在：

```text
launcher-mcp
```

例如：

```text
launcher-mcp::transport::streamable_http
```

不得让：

```text
launcher-core
launcher-workflow
launcher-action
launcher-domain
```

依赖 HTTP library。

---

# 8. Request Envelope

现代 2026 请求：

```http
POST /mcp HTTP/1.1
Content-Type: application/json
MCP-Protocol-Version: 2026-07-28
Mcp-Method: tools/call
Mcp-Name: search
```

Body：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": {
      "q": "otters"
    },
    "_meta": {
      "io.modelcontextprotocol/clientInfo": {
        "name": "launcher",
        "version": "4.4.0"
      }
    }
  }
}
```

现代 MCP 已要求 HTTP 层携带 `Mcp-Method` / `Mcp-Name`，并允许网关直接依靠这些 header 做路由。

---

# 9. Header / Body Consistency

这是 P0-B 最重要的协议安全规则之一。

对于：

```text
Mcp-Method
Mcp-Name
MCP-Protocol-Version
```

必须验证 body 与 header 一致。

例如：

```text
Header:
Mcp-Method: tools/call

Body:
method = tools/list
```

结果：

```text
ProtocolViolation
```

同理：

```text
Mcp-Name: search
body.params.name = delete
```

必须拒绝。

官方 TypeScript SDK 的 2026 migration 也明确要求现代请求验证这些标准 header 与 body 的一致性。

---

# 10. `Mcp-Name` 适用范围

不要强行让每个 MCP RPC 都发送任意 name。

只在协议规定能够映射到：

```text
params.name
params.uri
params.taskId
```

的请求中发送对应名字。

Tools：

```text
tools/list
    no tool name

tools/call
    Mcp-Name = params.name
```

未来 Tasks：

```text
tasks/get
tasks/update
tasks/cancel
```

可以使用 task identity。

当前 P0-B 不实现 Tasks，但 Transport parser 设计不得阻塞其未来扩展。

---

# 11. `_meta`

2026 profile 每个现代请求可以自描述：

```text
protocolVersion
clientInfo
clientCapabilities
```

通过 `_meta` 提供。

Launcher 最小发送：

```json
"_meta": {
  "io.modelcontextprotocol/clientInfo": {
    "name": "launcher",
    "version": "4.4.0"
  }
}
```

并根据需要加入：

```text
protocol version
capabilities
```

但：

```text
_meta
```

不能进入：

```text
ActionProposal
WorkflowStep.input
Effect.input
```

它属于：

```text
Transport Metadata
```

---

# 12. No Session

2026 Streamable HTTP 禁止：

```text
Mcp-Session-Id
```

作为现代 protocol state。

因此：

```text
Request A
Request B
Request C
```

可以：

```text
A → Server instance 1
B → Server instance 2
C → Server instance 3
```

而不要求 sticky session。官方规范将这一 stateless 行为作为 2026 revision 的主要变化之一。

---

# 13. Request Lifecycle

```text
build request
    ↓
validate URL
    ↓
build JSON-RPC body
    ↓
build standard headers
    ↓
validate header/body consistency
    ↓
send POST
    ↓
bounded response reader
    ↓
validate HTTP status
    ↓
validate JSON-RPC envelope
    ↓
validate MCP method-specific result
    ↓
normalize McpResult
```

---

# 14. HTTP Status ≠ MCP Error

不能简单：

```text
HTTP 500
    → BusinessError
```

也不能：

```text
HTTP 400
    → ProtocolViolation
```

应该：

```text
HTTP transport semantics
        +
JSON-RPC semantics
        +
MCP method semantics
```

共同决定最终 `McpError`。

现代 SDK 对 2026 requests 还区分某些 HTTP 400 中的合法 JSON-RPC error response，使其可以作为 in-band protocol error 处理。

因此 P0-B 要保留：

```rust
HttpStatusError
JsonRpcError
ProtocolSemanticError
```

三个层次。

---

# 15. Response Content-Type

对于 JSON response：

```text
Content-Type: application/json
```

应允许必要的参数变化：

```text
application/json
application/json; charset=utf-8
```

无法识别：

```text
text/html
text/plain
```

如果 body 不是合法 MCP response：

```text
ProtocolViolation
```

不要把 HTML error page 当作：

```text
BusinessError
```

---

# 16. Streamable HTTP Response

虽然叫 Streamable HTTP，但 P0-B 第一阶段只要求：

```text
request
    ↓
response
```

不要求实现：

```text
long-lived event stream
server push
subscriptions
```

原因：

> Tools call 的最小闭环不需要先引入完整 streaming/event subsystem。

因此第一阶段：

```text
application/json
    ✅

text/event-stream
    unsupported-safe-reject
```

除非某个具体 MCP compatibility case 明确要求。

---

# 17. Timeout

HTTP Transport 单独拥有：

```text
connect_timeout
request_timeout
response_header_timeout
```

但最终仍转换到：

```text
McpError::Timeout
```

Workflow 不知道：

```text
HTTP timeout
stdio timeout
```

都归一为：

```text
FailureClass::Timeout
```

---

# 18. Connection Pool

P0-B 不建议立即引入复杂连接池。

第一阶段：

```text
request
    ↓
HTTP client
    ↓
request
```

允许底层 HTTP library 自己提供 TCP connection reuse。

但是：

```text
MCP protocol session
```

与：

```text
HTTP TCP connection reuse
```

必须明确分开。

可以复用 TCP：

```text
TCP connection
    ├── request 1
    ├── request 2
    └── request 3
```

但不能复用：

```text
MCP session state
```

2026 profile 本身是 stateless。

---

# 19. Redirect Policy

P0-B 默认：

```text
redirect = disabled
```

或者：

```text
redirect = same-origin only
```

不要默认：

```text
https://trusted.example
      ↓
http://attacker.example
```

自动跟随。

认证进入 P0-C 后尤其重要。

---

# 20. DNS / SSRF Boundary

HTTP Transport 必须至少避免：

```text
localhost
127.0.0.1
0.0.0.0
IPv6 loopback
private network
link-local
metadata endpoints
```

被“remote MCP URL”静默访问。

特别是：

```text
http://169.254.169.254/
```

这种典型云 metadata endpoint。

P0-B 先做：

```text
URL classification
```

然后：

```text
Local
Private
Public
Unknown
```

策略：

```text
loopback HTTP
    allowed

private HTTP
    explicit opt-in

public HTTPS
    allowed

public HTTP
    explicit opt-in
```

不要把这个策略放进：

```text
McpExecutor
```

而应属于：

```text
StreamableHttpTransport
```

---

# 21. URL Rebinding

DNS rebinding：

```text
example.com
    ↓
public IP
    ↓
after lookup
    ↓
127.0.0.1
```

不能只在第一次 hostname parsing 时判断一次。

至少：

```text
resolve
↓
classify
↓
connect
```

都必须符合 URL policy。

更严格的实现可以：

```text
DNS result pinning
```

但暂时不要求 P0-B 完成复杂 resolver。

---

# 22. Response Size Limit

与 Phase 10 MCP stdio 相同的原则：

```text
HTTP response
    ↓
bounded reader
```

默认建议：

```text
max_response_bytes = 1 MiB
```

实际值可以根据现有 MCP result contract 调整，但必须：

```text
allocation boundary bounded
```

不能：

```text
read entire response
    ↓
check length
```

否则 P0-A 已经解决的 DoS 问题会在 HTTP 重现。

---

# 23. Request Size Limit

同样：

```text
arguments
        ≤ existing 256KB
```

加上：

```text
full HTTP body
        ≤ bounded request size
```

二者都要检查。

因为：

```text
arguments <= 256KB
```

不意味着整个 HTTP request：

```text
headers
metadata
JSON envelope
```

也一定小。

---

# 24. Header Size Limit

必须限制：

```text
total header bytes
individual header length
number of headers
```

避免：

```text
header bomb
```

尤其 P0-C 未来加入：

```text
Authorization
WWW-Authenticate
OAuth metadata
```

之后 header 会更复杂。

---

# 25. Standard Header Injection

用户配置：

```text
custom headers
```

P0-B 如果允许，应明确禁止覆盖：

```text
MCP-Protocol-Version
Mcp-Method
Mcp-Name
Content-Type
Host
Content-Length
```

这些必须由 Transport 自己控制。

官方 SDK 也采用“保留标准/auth header 名称不允许随意覆盖”的思路。

---

# 26. Custom Header Policy

v0.2 可以：

```toml
[[mcp.servers]]
id = "internal"
transport = "streamable-http"
url = "https://example.com/mcp"

[ mcp.servers.headers ]
X-Organization = "foo"
```

但：

```text
Authorization
Cookie
Mcp-*
MCP-*
Host
Content-Length
```

必须保留。

Authorization 之后由 P0-C credential provider 管理。

---

# 27. Error Mapping

继续使用：

```text
McpError::failure_class()
```

HTTP-specific errors 先转：

```text
McpError
```

例如：

```text
connection refused
    → ServerUnavailable

DNS failure
    → ServerUnavailable

connect timeout
    → Timeout

response timeout
    → Timeout

malformed JSON
    → ProtocolViolation

wrong JSON-RPC id
    → ProtocolViolation

HTTP 401
    → AuthenticationRequired

HTTP 403
    → PermissionDenied

MCP isError=true
    → BusinessError
```

`AuthenticationRequired` / `PermissionDenied` 若当前 `McpError` 尚不存在，可以作为 transport-level error；最终是否增加统一 FailureClass 要在 P0-C 决定，不要现在修改 Workflow FailureClass。

---

# 28. Authentication 暂时只识别，不实现

P0-B 可以识别：

```text
401
403
WWW-Authenticate
```

但：

```text
OAuth flow
token refresh
issuer discovery
credential storage
```

全部进入：

```text
P0-C
```

这样 P0-B 可以先支持：

```text
unauthenticated public MCP
```

也能正确报告：

```text
auth required
```

而不会把 P0-C 偷偷塞进 Transport。

---

# 29. Certificate Policy

默认：

```text
TLS certificate validation = ON
```

禁止默认：

```rust
danger_accept_invalid_certs(true)
```

测试环境如果需要：

```text
explicit test-only opt-in
```

而且不能写进普通 release config。

---

# 30. Proxy Policy

P0-B 不建议自定义 proxy protocol。

默认：

```text
system/default HTTP proxy behavior
```

或者明确：

```text
proxy = none/system/explicit
```

但不要让 MCP server metadata 决定 proxy。

---

# 31. `server/discover`

2026 profile：

```text
server/discover
```

可以用于：

```text
MCP server capability discovery
```

流程：

```text
StreamableHttpTransport
    ↓
server/discover
    ↓
normalized discovery
    ↓
McpCatalog
```

Legacy 2025 profile：

```text
server/discover
    → explicit unsupported
```

---

# 32. Discovery 与 tools/list

不要把：

```text
server/discover
```

当成：

```text
tools/list replacement
```

两者职责：

```text
server/discover
    = server capability / discovery metadata

tools/list
    = tool catalog
```

因此：

```text
McpCatalog
```

依然从：

```text
tools/list
```

建立。

---

# 33. Cache

继续沿用 Phase 11：

```text
ttlMs
cacheScope
```

但 HTTP Transport 不直接决定：

```text cache hit
```

负责：

```text response → CacheMetadata
```

然后：

```text MCP Catalog
```

管理缓存。

2026-07-28 正式支持 list result cache hints 与 deterministic ordering。

---

# 34. Trace Context

2026 MCP 已标准化通过 `_meta` 传递 W3C Trace Context：

```text
traceparent
tracestate
baggage
```

P0-B 不要求接入完整 OpenTelemetry，但 parser 必须：

```text
preserve or safely ignore
```

不能：

```text
treat traceparent as authority
```

官方 2026-07-28 说明中已经把这些 key 纳入 MCP trace propagation。

---

# 35. Response Validation

每个响应都必须过：

```text
HTTP
 ↓
JSON-RPC
 ↓
method-specific MCP semantics
 ↓
normalized result
```

例如：

```text
tools/call
```

必须包含：

```text
content
或
structuredContent
或
isError
```

否则：

```text
ProtocolViolation
```

沿用 Phase 10 修复的 envelope semantic validation。

---

# 36. Unknown Fields

继续：

```text
unknown JSON fields
    → ignore
```

但：

```text
unknown semantic state
    → reject if required for safe interpretation
```

不能：

```text
serde ignores unknown critical field
→ accidental success
```

---

# 37. Notifications

2026 Streamable HTTP 的 client-to-server notification semantics 与旧模型不同。

P0-B 第一阶段：

```text
client notification
    unsupported-safe-reject
```

除非当前 operation 明确要求。

不要实现一个：

```text
generic notification dispatcher
```

增加复杂性。

---

# 38. Streaming

P0-B 第一阶段：

```text
tools/call
    request → response
```

不实现：

```text
SSE
server push
long-lived event stream
```

后续如果需要：

```text
P0-B.1 Streaming Extension
```

独立增加。

当前官方新模型已经把传统长期 SSE 依赖移除，现代 stateless request 通过普通请求/响应承载主要调用。

---

# 39. Tests — Core Compatibility

建议新增：

```text
HTTP-COMPAT-001
modern request envelope

HTTP-COMPAT-002
Mcp-Method generation

HTTP-COMPAT-003
Mcp-Name generation

HTTP-COMPAT-004
header/body match

HTTP-COMPAT-005
method mismatch reject

HTTP-COMPAT-006
name mismatch reject

HTTP-COMPAT-007
version mismatch reject

HTTP-COMPAT-008
_meta generation

HTTP-COMPAT-009
server/discover

HTTP-COMPAT-010
tools/list

HTTP-COMPAT-011
tools/call

HTTP-COMPAT-012
structuredContent
```

---

# 40. Security Tests

```text
HTTP-SEC-001
HTTPS certificate validation

HTTP-SEC-002
HTTP public endpoint requires explicit opt-in

HTTP-SEC-003
loopback HTTP policy

HTTP-SEC-004
private network policy

HTTP-SEC-005
metadata endpoint blocked

HTTP-SEC-006
redirect policy

HTTP-SEC-007
header override blocked

HTTP-SEC-008
oversized request rejected

HTTP-SEC-009
oversized response rejected

HTTP-SEC-010
header bomb rejected

HTTP-SEC-011
wrong method header rejected

HTTP-SEC-012
wrong name header rejected

HTTP-SEC-013
HTML error not treated as MCP success

HTTP-SEC-014
401/403 preserved as auth boundary
```

---

# 41. Response Matrix

```text
HTTP 200
 ├── valid JSON-RPC success     → Success
 ├── valid JSON-RPC error       → McpError
 ├── malformed JSON             → ProtocolViolation
 └── wrong semantic envelope    → ProtocolViolation

HTTP 400
 ├── valid JSON-RPC error       → MCP protocol error
 └── arbitrary body             → HTTP/Protocol error

HTTP 401
 → AuthenticationRequired

HTTP 403
 → Permission boundary

HTTP 404
 → ServerUnavailable / endpoint error

HTTP 408
 → Timeout

HTTP 429
 → Server-side throttling
    preserve for future policy

HTTP 5xx
 → ServerUnavailable
```

注意：

`HTTP status → FailureClass` 映射要集中在 `StreamableHttpTransport`/`McpError`，不要散落在 Workflow。

---

# 42. Redirect Attack

测试：

```text
https://trusted.example/mcp
    ↓ 302
http://127.0.0.1:8080/admin
```

默认：

```text
reject
```

以及：

```text
https://trusted.example
    ↓
https://evil.example
```

如果允许 redirect：

```text
origin policy
```

必须显式校验。

---

# 43. SSRF Test Matrix

至少：

```text
127.0.0.1
localhost
0.0.0.0
[::1]
169.254.169.254
10.0.0.0/8
172.16.0.0/12
192.168.0.0/16
```

结果：

```text
HTTP public
    allowed

HTTP private
    explicit policy required

HTTPS public
    allowed

metadata endpoint
    denied
```

---

# 44. HTTP → Executor Boundary

最终：

```text
McpExecutor
    ↓
StreamableHttpTransport
```

与：

```text
McpExecutor
    ↓
StdioTransport
```

必须得到完全一致：

```text
McpToolResult
McpError
FailureClass
```

Transport 不得影响：

```text
Workflow Retry
Confirmation
Capability
AI Proposal
```

---

# 45. Cross-Transport Test

同一个 MCP calculator：

```text
stdio
```

和：

```text
Streamable HTTP
```

分别执行：

```text
12 + 34
```

必须：

```text
McpToolResult normalized equal
```

然后：

```text
same Action semantics
same FailureClass
same Workflow outcome
same execution_id semantics
```

---

# 46. Cross-Transport Architecture Invariant

建议：

```text
INV-TRANSPORT-001

Transport implementation MUST NOT alter
Launcher authorization semantics.
```

以及：

```text
INV-TRANSPORT-002

McpExecutor MUST consume normalized MCP transport
semantics and MUST NOT branch on concrete transport type.
```

这样不会出现：

```rust
if transport == Http {
    bypass_confirmation();
}
```

---

# 47. Dependency Rules

新增 Transport 依赖：

```text
launcher-mcp
    └── HTTP client
```

禁止：

```text
launcher-core → HTTP library
launcher-workflow → HTTP library
launcher-action → HTTP library
launcher-domain → HTTP library
```

Topology：

```text
launcher-core
     ↓
launcher-mcp
     ↓
StreamableHttpTransport
     ↓
HTTP library
```

---

# 48. Performance

P0-B 不要求 HTTP 比 stdio 快。

重点：

```text
connect latency
request latency
response parsing
catalog refresh
large response
connection reuse
```

至少：

```text
cold HTTP
warm HTTP
100 × tools/call
100 × tools/list
```

并比较：

```text stdio
vs
HTTP
```

只要求 HTTP 不产生：

```text
unbounded memory
connection leak
thread leak
```

---

# 49. Phase P0-B Definition of Done

```text
[ ] StreamableHttpTransport
[ ] 2026-07-28 stateless request
[ ] no initialize for modern profile
[ ] _meta support
[ ] Mcp-Method
[ ] Mcp-Name
[ ] header/body consistency
[ ] server/discover
[ ] tools/list
[ ] tools/call
[ ] deterministic catalog
[ ] ttlMs/cacheScope
[ ] structuredContent
[ ] JSON-RPC semantic validation
[ ] bounded request
[ ] bounded response
[ ] bounded headers
[ ] certificate validation
[ ] redirect policy
[ ] SSRF policy
[ ] private-network policy
[ ] metadata endpoint protection
[ ] 401/403 handling
[ ] timeout
[ ] transport error normalization
[ ] stdio/HTTP normalized-result equivalence

[ ] McpExecutor unchanged
[ ] ActionEngine unchanged
[ ] Workflow unchanged
[ ] AI Planner unchanged
[ ] launcher-runtime unchanged

[ ] HTTP compatibility tests
[ ] HTTP security tests
[ ] cross-transport tests
[ ] real HTTP MCP fixture
[ ] zero warnings
[ ] topology pass
[ ] all previous tests green
```

---

# 50. P0-B 不做的内容

明确冻结：

```text
❌ OAuth
❌ OIDC
❌ credential store
❌ automatic token refresh
❌ Tasks
❌ MCP Apps
❌ Resources
❌ Prompts
❌ client-side SSE subsystem
❌ generic server push
❌ persistent MCP protocol session
❌ new Effect type
❌ new Workflow type
```

其中 OAuth/Auth 明确进入 P0-C。

---

# 51. 推荐实施顺序

```text
P0-B.1
HTTP URL + configuration

        ↓

P0-B.2
2026 request envelope

        ↓

P0-B.3
StreamableHttpTransport

        ↓

P0-B.4
HTTP response + JSON-RPC validation

        ↓

P0-B.5
header/body consistency

        ↓

P0-B.6
server/discover + tools/list

        ↓

P0-B.7
tools/call

        ↓

P0-B.8
security boundary
SSRF / redirect / size / TLS

        ↓

P0-B.9
McpExecutor integration

        ↓

P0-B.10
cross-transport E2E

        ↓

P0-B.11
performance / soak

        ↓

P0-B.12
release regression
```

---

# 52. 最终 P0-B 架构

```text
                         ActionEngine
                              │
                            Effect
                              │
                         McpExecutor
                              │
                         McpTransport
                     ┌────────┴────────┐
                     │                 │
                stdio               HTTP
                     │                 │
              ProcessSession      Streamable
                     │              HttpTransport
                     │                 │
              launcher-runtime      HTTP Client
                     │                 │
                     ▼                 ▼
                  OS Process      Remote MCP
```

上层完全不关心：

```text
stdio
HTTP
local
remote
```

只看到：

```text
McpToolResult
McpError
FailureClass
```

---

## P0-B 的核心成功标准

最终必须证明这两个调用：

```text
McpExecutor
    ↓
StdioTransport
```

和：

```text
McpExecutor
    ↓
StreamableHttpTransport
```

在 Launcher 内部的语义上**完全等价**：

```text
same tool identity
same input
same result model
same FailureClass
same capability behavior
same confirmation behavior
same Workflow behavior
same AI behavior
same Effect
```

唯一变化是：

```text
Transport
```

这正是 P0-B 的边界。

官方 2026-07-28 规范明确把 Streamable HTTP 定位为无 session 的 HTTP workload，并要求 `Mcp-Method` / `Mcp-Name` 进行 header-level routing；因此现在最值得做的是把这些 HTTP 规则锁死在 `launcher-mcp`，而不是继续扩大 Core API。