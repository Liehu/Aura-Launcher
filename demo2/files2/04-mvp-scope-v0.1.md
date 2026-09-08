# Native Launcher v0.1 MVP Scope

## 1. MVP 不是完整产品

第一版只用于验证架构，不追求功能覆盖。

## 2. MVP 功能

### P0

- 全局热键
- Native popup
- 搜索输入
- 应用 Provider
- 文件 Provider
- 键盘导航
- Enter 执行
- Esc 关闭
- 基础 Action
- SQLite
- 基础 Context
- External Plugin Host
- Plugin timeout/crash handling
- benchmark harness

### P1

- Quick Switch
- clipboard provider
- recent provider
- preview interface
- settings

### 暂缓

- AI
- MCP
- Workflow UI
- Marketplace
- Python/Node
- WASM
- 跨平台
- 云同步

## 3. 第一个可运行 Slice

必须优先完成一个垂直切片：

```text
Hotkey
  -> Slint popup
  -> App Provider
  -> Results
  -> Enter
  -> Launch app
```

只有这个 slice 工作后，才扩展 File Provider。

## 4. 推荐开发顺序

```text
01 repo bootstrap
02 domain types
03 slint popup
04 hotkey
05 app provider
06 action engine
07 search/ranking
08 sqlite history
09 file index
10 file provider
11 context
12 plugin host
13 crash/timeout tests
14 memory benchmark
15 performance regression harness
```

## 5. MVP 里程碑

### M0 基础工程

验收：

- workspace
- CI
- AGENTS.md
- docs
- hello UI

### M1 Launcher Core

验收：

- hotkey
- popup
- command
- action

### M2 Search

验收：

- app search
- file search

### M3 Context

验收：

- explorer current directory

### M4 Plugin

验收：

- external plugin
- IPC
- timeout
- crash recovery

### M5 Performance

验收：

- benchmark
- memory soak
- regression baseline

## 6. MVP Demo

最终演示只需要完成：

```text
Ctrl+Space
      ↓
打开原生 Launcher
      ↓
输入 "code"
      ↓
找到 VS Code
      ↓
Enter
      ↓
启动

Ctrl+Space
      ↓
输入文件名
      ↓
结果来自 Indexer
      ↓
Enter
      ↓
打开文件

Ctrl+Space
      ↓
插件命令
      ↓
启动 External Plugin Host
      ↓
返回 Native Result Schema
      ↓
执行 Action
      ↓
Plugin Host 自动退出
```

## 7. 第一版最重要的成功/失败判据

### 成功

即使只有：

- App Search
- File Search
- 一个 Plugin
- 一个 Context

但它：

- 快
- 稳
- 内存低
- 架构干净
- Agent 能持续迭代

就证明方向正确。

### 失败

出现以下情况应及时停下来重构：

- UI/Indexer/Plugin 强耦合
- 插件导致 Core 崩溃
- Python/Node runtime 常驻
- Idle memory 快速超过预算
- Search 越做越慢
- Agent 必须修改大量无关代码才能完成小需求
- 新功能只能继续堆 if/else
