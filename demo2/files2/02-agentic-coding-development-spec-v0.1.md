# Agentic Coding 开发规范 v0.1

本规范用于让 AI Coding Agent 成为主要开发者，同时确保架构不会因为 Agent 的局部优化而失控。

---

## 1. 核心原则

Agent 不只是“写代码工具”，而是一个受约束的软件工程参与者。

每次任务必须遵循：

```text
Spec -> Plan -> Inspect -> Implement -> Test -> Review -> Record
```

禁止：

```text
Prompt -> Big Rewrite -> Hope
```

---

## 2. Agent 角色

推荐至少定义以下逻辑角色：

### Architect Agent

职责：

- 检查现有架构
- 编写/修改 ADR
- 识别跨模块影响
- 拒绝未经设计的大改

禁止：直接批量修改实现，除非任务本身就是架构迁移。

### Implementer Agent

职责：

- 小步实现
- 遵循现有 API
- 添加测试
- 不修改无关模块

### Reviewer Agent

职责：

- API/ABI 检查
- 并发检查
- 内存检查
- 错误处理
- 安全边界
- 测试充分性

### Performance Agent

职责：

- benchmark
- allocation hotspot
- startup latency
- memory growth
- regression comparison

### Test Agent

职责：

- 单元测试
- 集成测试
- fuzz/property tests
- failure injection
- soak test

---

## 3. Agent Task Contract

每个 Agent 任务必须包含：

```yaml
id: LAUNCHER-123
objective: "实现文件名搜索 Provider"
scope:
  include:
    - crates/launcher-search
    - tests/search
  exclude:
    - launcher-ui
    - plugin-host
constraints:
  - 不修改 public API
  - 不新增 runtime dependency
acceptance:
  - cargo test passes
  - search benchmark <= baseline * 1.10
  - no clippy warnings
artifacts:
  - code
  - tests
  - benchmark result
```

没有 Acceptance Criteria 的任务不得进入实现阶段。

---

## 4. Agent 工作循环

### Phase 1 Inspect

Agent 必须先读取：

- README
- 相关 crate
- 相关 tests
- ADR
- Cargo.toml
- 当前 benchmark

不得在没有阅读上下文时直接修改核心代码。

### Phase 2 Plan

输出：

```text
Goal
Assumptions
Files affected
API changes
Risk
Test plan
Rollback
```

### Phase 3 Implement

一次任务尽可能只解决一个逻辑目标。

### Phase 4 Validate

最小验证：

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Phase 5 Review

第二 Agent 或独立 review prompt 检查：

- 是否违反 architecture
- 是否引入 hidden coupling
- 是否产生无界 cache
- 是否把 IO 放进 UI thread
- 是否有 panic path
- 是否有 unsafe
- 是否增加 runtime

### Phase 6 Record

必须留下：

- commit
- test result
- benchmark result（性能相关变更）
- ADR（架构相关变更）

---

## 5. Coding Rules

### Rust

- `unsafe` 默认禁止；需要 ADR + review。
- `unwrap()` 在业务核心代码禁止；测试可例外。
- `expect()` 必须有明确 invariant。
- `clone()` 必须有理由，性能敏感路径禁止无意识 clone。
- public API 优先使用领域类型，不使用散落字符串。
- 线程边界明确 Send/Sync 要求。

### Async

不要把 CPU-heavy 工作扔进 async task 后长期占用 executor。

禁止：

```rust
async fn query() {
    huge_cpu_loop();
}
```

应该显式使用阻塞线程池/专用 worker。

### UI

UI thread 不做：

- 文件遍历
- 网络
- SQLite 大查询
- 模型调用
- 插件进程启动等待
- 图片大规模解码

---

## 6. Commit 规范

使用：

```text
feat(scope): ...
fix(scope): ...
perf(scope): ...
test(scope): ...
refactor(scope): ...
docs(scope): ...
perf-bench(scope): ...
```

一个 commit 尽量表达一个逻辑变更。

---

## 7. Pull Request / Agent Patch 门禁

任何 Agent patch 必须回答：

```text
Why
What
Risk
Tested
Performance impact
Memory impact
API impact
```

### 红线

以下变更必须人工确认/Architect Agent review：

- 增加后台常驻线程
- 增加第三方 runtime
- 引入 WebView
- 修改 IPC
- 修改 plugin manifest
- 修改 DB schema
- 引入全局缓存
- 引入长生命周期 Arc
- 增加跨 crate public dependency
- 大面积 clone/serialize

---

## 8. Agent Prompt Template

推荐使用固定系统 Prompt：

```text
You are an implementation agent for Native Launcher.

Primary goals:
1. Preserve architecture boundaries.
2. Preserve low-memory behavior.
3. Preserve deterministic tests.
4. Make the smallest correct change.

Before editing:
- inspect related code
- inspect tests
- inspect ADRs

During editing:
- do not introduce WebView/Electron/CEF
- do not add hidden background runtime
- do not add unbounded caches
- do not expand task scope

After editing:
- format
- check
- clippy
- test
- report changed files
- report performance/memory impact

If the requested change conflicts with an architecture rule,
stop implementation and propose an ADR instead.
```

---

## 9. Agent Context Files

建议仓库根目录保留：

```text
AGENTS.md
ARCHITECTURE.md
CONTRIBUTING.md
TESTING.md
PERFORMANCE.md
SECURITY.md
```

### AGENTS.md

内容必须包括：

- 架构红线
- build/test commands
- directory ownership
- naming rules
- task workflow
- benchmark workflow

### Architecture ownership

每个 crate 必须有明确职责：

```text
launcher-domain      pure model
launcher-search      search/ranking
launcher-context     OS context
launcher-action      action/effect
launcher-ipc         transport/protocol
launcher-ui          Slint presentation
launcher-indexer     indexing
launcher-plugin-host runtime boundary
```

---

## 10. 防止 Agent 架构腐化

每次迭代运行 Architecture Lint：

检查：

- UI crate 是否依赖 indexer implementation
- domain 是否依赖 OS API
- plugin-host 是否反向依赖 UI
- Core 是否直接依赖 Python/Node SDK
- 是否新增禁止依赖

可通过：

- cargo-deny
- cargo tree
- 自定义脚本
- grep/AST checks

实现。

---

## 11. Agentic Coding 的最小粒度

不建议一次让 Agent 实现：

> “做完整插件系统”

应该拆成：

```text
1. domain plugin manifest
2. manifest parser
3. validator
4. plugin registry
5. process launcher
6. IPC transport
7. query request
8. query response
9. crash handling
10. timeout
11. permission check
12. integration test
```

每一步都应可独立测试。

---

## 12. Definition of Done

任何任务必须满足：

- 编译通过
- clippy clean
- 所需测试通过
- 没有 debug println
- 没有临时 TODO 伪装成功
- 文档同步
- API 变更已记录
- 性能敏感变更有 benchmark

---

## 13. AI 生成代码特殊要求

禁止接受：

- 未解释的 unsafe
- 大片复制代码
- 未测试的并发代码
- catch-all error swallowing
- silent fallback
- 无限 retry
- 无限 cache
- 在 UI thread 中同步 IO

Agent 应优先复用已有 abstraction，而不是生成新的平行 abstraction。
