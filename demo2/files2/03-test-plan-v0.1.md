# Native Launcher v0.1 测试方案

---

## 1. 测试目标

MVP 测试重点不是功能数量，而是验证三件事：

1. 原生 UI 是否足够顺滑。
2. Core/Indexer/Plugin 隔离是否真的降低常驻成本。
3. Command/Provider 模型是否足以承载后续扩展。

---

## 2. 测试层级

```text
Unit
  -> Component
  -> Integration
  -> IPC
  -> Plugin contract
  -> UI smoke
  -> Performance
  -> Memory soak
  -> Fault injection
```

---

## 3. Unit Tests

### Domain

测试：

- Command serialization
- Action validation
- Provider metadata
- score calculation
- deduplication

### Search

测试：

- exact match
- prefix
- fuzzy
- multi-token
- empty query
- Unicode
- Chinese/Pinyin（预留）

### Context

测试：

- context normalization
- stale context
- missing foreground window

### Plugin

测试：

- manifest parse
- required fields
- version validation
- capability validation
- timeout config

---

## 4. Integration Tests

### App Provider

给测试环境创建：

```text
TestApp1
TestApp2
TestTool
```

验证搜索、排序、启动。

### File Provider

创建：

```text
100 files
10 directories
Unicode names
duplicate names
large names
```

验证：

- index
- search
- result metadata

### Context

模拟 Explorer context，验证：

```text
open terminal here
search current folder
copy path
```

---

## 5. IPC Tests

必须验证：

- request/response
- invalid message
- malformed JSON
- unknown method
- timeout
- partial write
- disconnect
- server restart
- duplicate request id

所有 IPC 契约必须有 contract test。

---

## 6. Plugin Contract Tests

测试插件：

### Normal

```text
query -> result
```

### Slow

```text
query -> 5s delay
```

预期：Host timeout，不阻塞 UI。

### Crash

```text
process exit 1
```

预期：Core 保持运行。

### Malformed

返回无效 JSON。

预期：Host 拒绝并记录错误。

### Flood

返回 100,000 results。

预期：Host 限流/截断，不允许 UI OOM。

---

## 7. UI Smoke Test

最小路径：

```text
Start
-> press hotkey
-> type query
-> select result
-> Enter
-> execute
-> Escape
```

重复 1000 次，不应出现：

- 无响应
- 窗口泄漏
- 焦点失效
- 内存持续增长

---

## 8. Performance Tests

### 8.1 Hotkey latency

指标：

```text
Hotkey -> visible UI
```

目标：

```text
P50 <= 20 ms
P95 <= 35 ms
```

### 8.2 Search

App search：

```text
P50 <= 5 ms
P95 <= 10 ms
```

File search：

```text
P50 <= 20 ms
P95 <= 50 ms
```

这些是 MVP 设计目标。

### 8.3 IPC

本机请求/响应目标：

```text
P50 <= 1 ms
P95 <= 5 ms
```

---

## 9. Memory Tests

### 9.1 Idle Baseline

启动：

```text
No search
No plugin
No preview
No AI
```

观察 5 min。

目标：

```text
Private Bytes <= 50 MB target
<= 80 MB hard budget
```

### 9.2 Search Soak

执行：

```text
100,000 random queries
```

要求：

```text
final memory <= baseline + 20%
```

### 9.3 Open/Close UI Soak

循环：

```text
show -> hide
```

10,000 次。

不能出现持续线性增长。

### 9.4 Plugin Soak

循环：

```text
spawn -> query -> shutdown
```

1000 次。

要求：

- 子进程全部退出
- handle 不持续增长
- Core 内存稳定

### 9.5 Provider Stress

加载 1000 个 mock providers。

验证：

- registry 内存
- query latency
- plugin lifecycle

---

## 10. Fault Injection

必须测试：

- Indexer 崩溃
- Plugin 崩溃
- Plugin hang
- Plugin spam
- DB lock
- DB corruption（最小模拟）
- IPC disconnect
- Explorer 不存在
- 当前目录被删除
- 文件权限拒绝

目标：

> 一个子系统故障不导致 Launcher UI / Core 进程崩溃。

---

## 11. Concurrency Tests

测试：

- 100 concurrent queries
- UI show + search + hide race
- plugin shutdown during query
- index update during search
- DB writer/reader concurrency

必须使用：

- loom（适用时）
- tokio test
- deterministic scheduler strategy
- stress tests

---

## 12. Fuzz / Property Tests

重点：

- Query parser
- Manifest parser
- IPC parser
- Ranking
- Path normalization

性质示例：

```text
parse(serialize(x)) == x
```

以及：

```text
ranking 不应因输入顺序随机变化
```

---

## 13. Regression Benchmark

建立 baseline：

```text
benchmarks/baseline.json
```

每次性能 PR 比较：

```text
current / baseline
```

规则建议：

```text
> 5% regression -> warning
> 10% regression -> fail
```

除非 PR 明确更新 baseline 并记录原因。

---

## 14. CI Gates

Pull Request 必须通过：

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Performance PR 额外：

```bash
cargo bench
```

Release Candidate 额外：

- smoke test
- memory soak
- plugin crash test
- startup benchmark

---

## 15. Manual UX Test

测试人员至少验证：

1. 热键是否瞬时。
2. 输入是否无卡顿。
3. 键盘导航是否连续。
4. Esc 行为是否自然。
5. 启动应用是否没有明显延迟。
6. 文件搜索是否可信。
7. Quick Switch 是否符合当前上下文。

不要只做自动化测试。

---

## 16. Release Acceptance

MVP Release Candidate 必须满足：

### Functional

- App search
- File search
- Hotkey
- Action
- Context
- External plugin

### Reliability

- No crash in 1000 UI loops
- No Core crash from plugin crash
- Indexer restart works

### Performance

- Idle memory <= 80 MB
- Search benchmark within target
- hotkey latency within target or有明确已知限制

### Engineering

- docs updated
- benchmark recorded
- known issues listed
- no architecture red-line violations
