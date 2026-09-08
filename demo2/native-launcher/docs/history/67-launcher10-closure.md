# 67 — Launcher 1.0 Product Closure 实施记录（MUST-1..4）

日期：2026-09-06。依据：66 号验收审计的 MUST 清单（用户已批准）。范围铁律：**只收口，不扩张**。

---

## MUST-1 Workflow 触发源产品化 ✅

- 新增 `launcher-core::providers::workflows::WorkflowCatalogProvider`：加载 `<data>/workflows/*.json`（启动时 seed 一份 `demo.json`），每个有效定义 = 一条可搜索命令（`provider_id="workflows"`，`Category::Command`，action id `run`，payload = 定义文件路径）；损坏定义告警跳过。
- 执行路由：`execute_action_by_id` 按 host-owned 命名空间拦截 `workflows` → `load_definition`（解析+validate）→ `workflow_service::start_workflow`（真实 WorkflowRunner，含 confirmation 暂停/恢复）。引擎永远不会把 workflow 文件当 Open 目标——与 `mcp:`/`plugin:` 同一模式，INV-004/037 保持。
- 触发闭环：搜索 → Enter → 运行 → Workflow Surface 实时进度。**"只有托盘 demo 能触发"的半接线状态结束**。
- 测试：有效定义加载、无效定义跳过 2 项。

## MUST-2 Settings 闭环 ✅

- 搜索内新增 **"Open Settings"** 命令（provider `settings`，关键词 settings/config）→ 记事本/系统默认编辑器打开 `config.toml`。
- **配置热重载**：每次 popup show 检查 config mtime，变更即重载——accent、UI scale、`result_limit`（查询闭包改读 ReloadState 的活值）立即生效；**hotkey 变更时旧 guard drop（注销）+ 新键重注册**。用户循环 = 编辑 → 保存 → 再呼出 → 已生效。
- 结构代价：热键注册块重构为 `HotkeyDeps` + `register_hotkey()`（可重复注册），`mem::forget` 泄漏守卫的做法移除。
- 说明：完整设置编辑面板涉及 UI-CONTRACT v0.2 立项（P1-E 范畴），本批不做；上述闭环已消除"只能手编 TOML 且改动不生效"的产品缺陷。

## MUST-3 Installer / Upgrade ✅

- `scripts/package.py`：release 构建 → `artifacts/dist/NativeLauncher-<ver>-win64.zip`（exe + `install.ps1` + `workflows/demo.json` + README）。
- `install.ps1`：复制到 `%LOCALAPPDATA%\Programs\NativeLauncher`，创建开始菜单快捷方式，seed demo workflow；**升级安全**：`%LOCALAPPDATA%\native-launcher`（索引/工作流）与 `%APPDATA%\NativeLauncher`（配置/插件/日志）永不触碰。
- 卸载 = 删除程序目录 + 快捷方式 + 两个数据目录（README 说明）。

## MUST-4 性能 / 内存基线 ✅

- `scripts/perf_baseline.py` → `artifacts/perf-baseline.json`：snapshot 全场景总时长（冷启→渲染→退出上界）、idle RSS。本次记录：**idle RSS ≈ 18.8 MB**（符合"原生低内存"定位），15 场景捕获 ≈ 18.6 s（含每场景固定 settle 延迟，文档已注明是回归观察上界而非 SLA）。
- 定位遵 66 §29："no perceptible lag" 是闸门，数字用于回归对比。

## 验证

```text
cargo test --workspace   514 passed / 0 failed（MUST-1 新增 2 项）
cargo build --workspace  zero warnings（release 打包另过）
check_topology.py        PASS
release_gate.py          十 Gate（MUST 改动不触碰 VR 表面，G7 维持 15/15）
package.py               产出 dist zip；perf_baseline.py 产出基线 JSON
```

## 冻结声明

至此 66 号清单的 MUST 全部关闭。Launcher 1.0 功能面冻结：主链路 + 插件 + MCP + 安装型 Workflow + Settings 闭环 + 安装包。SHOULD/DEFERRED 项（Agent 接线、隐私分级、增量索引、App Identity、Canonical ID 等）按 66 §三 顺序进入 1.0 后第一波，**在用户给出真实使用反馈之前不再新增功能面**。
