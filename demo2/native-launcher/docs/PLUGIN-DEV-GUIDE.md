# PLUGIN-DEV-GUIDE — 插件开发指南（作者视角）

> 状态：**CURRENT**（P3.1 基线，2026-09-10）。本文是
> `PLUGIN-CONTRACT-v0.1.md`（FROZEN）的作者视角配套：契约定义"必须怎样"，
> 本指南讲"怎样做到"。全部内容来自当前代码事实——**未实现的能力明确标注
> 未实现**，不做前瞻承诺。
>
> 权威顺序：协议语义以 `PLUGIN-CONTRACT-v0.1.md` 为准；冲突时契约赢。

---

# 1. 插件是什么

一个插件 = **一个独立进程** + 一份 `plugin.json`。进程通过 stdin/stdout
上的换行分隔 JSON-RPC 与宿主通信（协议细节见契约，Rust 开发者用 SDK 封装
可完全不碰）。宿主保证：

- 插件崩溃/挂起/返回畸形结果**不影响启动器本身**（下次查询自动重新拉起）；
- 进程受 Job Object 管控（宿主退出即回收，无孤儿进程）；
- 结果经宿主校验并截断到 **100 条**（flood 防护）。

## 目录与安装

```text
%LOCALAPPDATA%\native-launcher\plugins\<你的插件id>\
    ├── plugin.json          ← manifest（唯一必需文件）
    └── <可执行文件>          ← manifest.executable 指向
```

放入后**重启启动器**生效（v1 无热安装；管理页可启用/禁用已安装插件，
见 §7）。

---

# 2. 快速上手：5 分钟最小插件（Rust）

参考实现：`apps/example-echo-plugin`（本仓库内，`cargo build` 即得）。

**第 1 步** — 新建 cargo bin crate，依赖：

```toml
[dependencies]
launcher-plugin-api = "*"
serde_json = "*"
```

**第 2 步** — `main.rs`：

```rust
fn main() {
    launcher_plugin_api::serve(|query| {
        serde_json::json!([
            {
                "title": format!("Echo: {query}"),
                "subtitle": "from my-first-plugin",
                "actions": ["echo"]
            }
        ])
    })
    .expect("plugin io");
}
```

`serve(handler)` 封装全部协议循环：initialize 握手、query 分发
（handler 收到查询文本，返回 Native UI Schema 条目数组）、shutdown。

**第 3 步** — manifest（`plugin.json`）：

```json
{
  "schema_version": 1,
  "id": "com.me.my-plugin",
  "name": "My Plugin",
  "version": "0.1.0",
  "api_version": "0.1",
  "runtime": { "type": "process", "executable": "my-plugin.exe" },
  "capabilities": [],
  "timeout_ms": 2000,
  "idle_timeout_ms": 10000
}
```

**第 4 步** — 构建并把 exe 与 plugin.json 放进 §1 的安装目录，重启启动器，
搜索任意文本即可看到 Echo 结果。

---

# 3. Manifest 字段参考

| 字段 | 必填 | 说明 |
|---|---|---|
| `schema_version` | 建议 | manifest 数据格式版本；有值时必须为 `1` |
| `id` | ✅ | 全局唯一（惯例：反向域名 `com.author.name`） |
| `name` | ✅ | 显示名 |
| `version` | 建议 | 插件自身版本（管理页展示） |
| `api_version` | ✅ | 协议版本，当前必须 `"0.1"`（不匹配 = 拒绝加载） |
| `executable` | 二选一 | 旧式平铺字段；`runtime.executable` 声明时优先生效 |
| `runtime` | 二选一 | `{ "type": "process" \| "python", "executable": …, "args": […] }`；`python` 类型的解释器由宿主按 config > `LAUNCHER_PYTHON` > PATH 解析（ADR-0009） |
| `capabilities` | ✅ | 能力点分名数组；**requires ⊆ capabilities 单调**（见 §5） |
| `timeout_ms` | ✅ | 单次请求超时，`(0, 60000]` |
| `idle_timeout_ms` | ✅ | 空闲自动 kill 阈值 |
| `window` | 可选 | P3.1：`true` = 插件自开顶层 UI 窗口，允许启动器的窗口管理（管理页置顶/钉住）枚举它；**默认 false，未声明不被枚举** |

校验失败（缺字段/版本不符/timeout 越界）= 拒绝加载，日志可见。

---

# 4. 生命周期（宿主管理，作者需知）

```text
Discovered(<plugins>/*/plugin.json)
  → Validated(manifest 校验)
  → Spawned(首次 query 按需拉起)
  → Running(每次 query 刷新 idle 计时)
  → Idle 超过 idle_timeout_ms 自动 kill
  → 超时/崩溃/坏结果 → kill，下次查询重新拉起
```

- 进程按需拉起：安装后不被搜索命中就不会运行；
- 空闲回收：长时间不用自动结束，省资源；
- 管理窗口（托盘右键 → 打开管理窗口）可**启用/禁用**插件，禁用**立即**
  生效（该插件不再出现在搜索结果）且跨重启持久；连续失败 3 次自动
  **隔离**（quarantined，管理页可见原因，需手动解除）。

---

# 5. UI 表达能力清单（作者最关心的部分）

### 5.1 查询结果（当前唯一受支持的呈现）

条目 schema = `launcher-ipc::PluginResultItem`（宿主逐条校验；**必须
原样回显 query_id**）：

| 字段 | 必填 | 说明 |
|---|---|---|
| `title` | ✅ | 显示标题 |
| `id` | 可选 | **稳定逻辑 id**（INV-028）：提供时宿主命令 id = `<manifest.id>:<id>`，否则从标题派生 |
| `subtitle` | 可选 | 副标题 |
| `actions` | 可选 | 字符串数组（legacy 简式）或动作描述对象；单个坏动作被丢弃，不影响所在条目（动作级故障隔离，INV-031） |
| `score` | 可选 | [0,1] 排名提示（解析时钳制）——让计算器类非词法结果得以浮现 |

- 用户 Enter/点击 → 宿主按 `actions` 生成 ActionPanel；
- 动作执行经宿主权威链（Resolver → Engine），**插件自己不执行 Effect**；
- 动作描述对象的 `requires` 声明所需 capability；超出
  `manifest.capabilities` 的请求被拒（CapabilityDenied，见 §6）。

### 5.1.1 富结果（P3.2，已实现）

给条目加可选 `rich` 字段即可——列表行不变，选中后的详情面板渲染富内容：

```json
{ "title": "= 80", "actions": ["copy"],
  "rich": { "blocks": [
    { "type": "text", "text": "= 80", "emphasis": "strong" },
    { "type": "key_value", "rows": [{"key": "expression", "value": "12+34*2"}] },
    { "type": "divider" },
    { "type": "table", "headers": ["supported", "ops"], "rows": [["basic", "+ - * / % ^"]] }
  ] } }
```

规则（RICH-RESULT-v1 契约）：未知 block 跳过；payload 非法只丢 rich
部分、条目照常返回；≤64 块 / 表 ≤200 行 / ≤256K 字符；intake 时逐串
脱敏。完整示例：`apps/calculator-plugin`（快速计算行 + 计算器页面）。

**没有的**：任意 HTML/WebView、Image block（后置 P4）、交互表格。

### 5.2 插件自开窗口（P3.1 起支持，实验性）

插件进程可以自己打开顶层窗口（如 Python tkinter、Rust winit）。声明
`"window": true` 后：

- 管理窗口（托盘右键 → 打开管理窗口）的 **Windows** 页会列出该插件进程
  的可见顶层窗口；
- 用户可对其执行 **置顶（Pin on top）/ 跟随钉（跟随指定应用前台）/
  钉在桌面**（WorkerW 重父化，实验性——tkinter/Electron/Qt 兼容性
  未逐一验证，失败会明确报错）。

边界与红线：宿主只枚举/操作**本宿主拉起的插件进程**的窗口；提权目标
拒绝并报错；未声明 `window` 的插件永不被枚举。

### 5.3 长任务/后台行为

不支持常驻后台任务——idle 超时即回收。需要定时任务的插件应外置调度
（如系统计划任务调用你的 CLI）。

---

# 6. 动作执行（execute_action）

用户在 ActionPanel 选中动作后，宿主发 `execute_action` 请求：

```json
{ "jsonrpc": "2.0", "id": 7, "method": "execute_action",
  "params": {
    "execution_id": "<host 生成，原样回显>",
    "action_id": "echo",
    "input": { },
    "context_generation": 0 } }
```

期望响应（execution_id 必须原样回显——两个 id 空间永不混用）：

```json
{ "jsonrpc": "2.0", "id": 7, "result": { "execution_id": "<同上>", "result": {} } }
```

Rust SDK：`serve_with_actions(handler, action_fn)`，action_fn 收到
`(action_id, input, query_text)` 返回结果 JSON；`serve_with_catalog*`
额外支持**空 query = 发现请求**（返回静态目录，使工作流/AI 可以解析
你的命令而无需用户输入）。

### 错误码（-32001..-32007 私有段）

| 码 | 含义 | 作者视角 |
|---|---|---|
| -32001 | CAPABILITY_DENIED | requires 超出 manifest——收紧 requires 或补 capabilities |
| -32002 | TIMEOUT | 处理超过 timeout_ms |
| -32003 | PLUGIN_CRASHED | 进程退出（非协议响应） |
| -32004 | RESULT_TOO_LARGE | 超 256KB 帧上限 / 超 100 条 |
| -32005 | RATE_LIMITED | 请求过频 |
| -32006 | PLUGIN_UNAVAILABLE | 进程拉起失败 |
| -32007 | VERSION_MISMATCH | api_version 不符 |

---

# 7. 调试与故障排查

- **日志**：`%LOCALAPPDATA%\NativeLauncher\logs\launcher.log`（每天滚动，
  `LAUNCHER_LOG=debug` 提级别；release 构建无控制台窗口属预期）；
- **禁用/启用**：管理窗口 Plugins 页（托盘右键 → 打开管理窗口）——禁用
  立即生效且跨重启持久；调试时可禁用其他插件隔离干扰；
- **隔离（quarantine）**：连续 3 次失败自动隔离，插件从搜索结果消失；
  解除 = 管理页处理（或手删 registry 记录后重启）；
- **协议冒烟**：不依赖启动器，直接 echo 一行 initialize 请求到你的进程
  stdout 验证握手（calculator-plugin 的契约测试即此模式）；
- ** Canonical 参考**：`apps/calculator-plugin` 是协议变更的硬门禁实现；
  写插件遇到歧义时以它的行为为准。

# 8. 打包清单（发布给其他用户）

```text
my-plugin/
  ├── plugin.json
  ├── my-plugin.exe          ← 或 .py（runtime.type=python）
  └── （其余运行所需文件）
```

收件人放置到 `%LOCALAPPDATA%\native-launcher\plugins\my-plugin\` 并重启。
签名/信任链路：P2.8 已有 SHA-256 完整性与信任状态机（管理页可见 trust），
Marketplace 分发与签名证书 **BLOCKED-EXTERNAL**（未开放）。

---

# 9. 能力与红线（作者速查）

- `capabilities` 是 allow-list：动作 `requires` 必须 ⊆ manifest.capabilities；
- 凭据/token **永不**进入查询结果或动作输出（宿主隐私模型红线）；
- 协议帧上限 256KB、结果 ≤100 条、无 shell 解释——需要外部命令时在
  插件进程内自行受控执行并自担安全审查；
- 协议语义变更须走 ADR（host 侧强制）；未知 method 回 `-32601` 即可
  向前兼容。
