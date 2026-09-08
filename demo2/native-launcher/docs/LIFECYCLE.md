# LIFECYCLE.md — 进程与状态生命周期

回答"X 发生时 Y 正在做什么"这一类状态机问题（v0.1.x 基线）。

## 系统主状态机

```text
BOOT
 │ Core Init（顺序：panic hook → 日志 → 配置 → autostart → Providers(apps/recent/index/plugins) → UI → 托盘 → 热键）
 ▼
IDLE（窗口隐藏，托盘常驻，空闲 Private ≈ 12.6 MB）
 │
 ├──────── 热键 / 托盘左键 ────────┐
 ▼                                │
POPUP（show → take_foreground → 清空查询 → context snapshot 后台采集）
 │
 ├── Search（每次击键: SearchSession.begin() 新代际 → 后台线程查询 → 仅最新代际回传 UI）
 ├── Enter / 点击 → Action Engine（后台线程）→ 成功则 hide
 └── Esc / Action 成功 → dismissed → hide
 ▼
POPUP HIDDEN
 ▼
IDLE（循环）
```

## 子生命周期

### Plugin 进程
```text
Discovered(<data>/plugins/*/plugin.json, 启动时)
→ Validated（manifest 校验 + executable 路径约束, ADR-0005）
→ Spawned（首次 query 按需拉起；立即Assign 进 Job Object）
→ Running（每次 query 刷新 idle 计时）
→ Idle 超过 idle_timeout_ms → kill（Drop → kill + job terminate）
异常路径：超时 → kill；crash → 错误返回；坏结果 → 拒绝；下一次查询重新拉起
```

### Indexer
- 内嵌模式：app 启动时 rebuild（有界扫描），查询走同一 SQLite（WAL）
- 独立模式：`launcher-indexer-service`（stdio JSON-RPC status/rebuild/search/shutdown）
- 已知限制：无增量 watch（Phase 2: USN/MFT）；rebuild 与查询并发由 SQLite WAL + bounded scan 保护

### 查询（并发模型，ADR-0004）
```text
谁拥有谁：Core/AppState 由 Mutex 保护；SearchSession 是全局代际计数器（lock-free）
可并发：多次击键的 search 线程可并发执行
必须 serial：对 Core 的访问（Mutex）；UI 属性更新（事件循环）
可 supersede：旧代际查询结果在回传前被丢弃（不 cancel 底层查询，因为单查询 ≤ 微秒级）
可 cancel：插件查询按 manifest timeout 超时 kill
```

## 边界情况（当前行为）

| 场景 | 行为 |
|---|---|
| Core shutdown 时插件在跑 | 进程退出 → PluginHandle Drop → child kill + Job close，树级回收 |
| 查询进行中 Indexer 关闭 | 查询返回错误 → FileProvider 容忍（空结果），Core 不崩 |
| Action 执行中窗口被关 | 后台线程继续执行 Action；hide 幂等 |
| 插件超时且回调挂起 | reader channel recv_timeout 超时 → kill；迟到数据留在 channel，handle 已置 None 丢弃 |
| popup 过渡期第二次 Ctrl+Space | visible 原子标志 toggle；事件都经 invoke_from_event_loop 串行执行 |
| Esc 与 Action 成功竞态 | hide 幂等 + visible 标志双写一致 |
