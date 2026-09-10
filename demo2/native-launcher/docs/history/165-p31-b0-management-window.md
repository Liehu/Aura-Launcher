# 165 — P3.1 Batch B0：统一管理窗口（设置 / 插件 / 工作流）

> 日期：2026-09-10。范围：P3.1 第一批（规范族：files2/P3.1 四件套 +
> ADR-0019）。输入基线：history/164（906 tests）。

## 交付

- **F01 管理窗口（P31-001/002）**：
  - `crates/launcher-ui/ui/management.slint`（新）：独立 Slint 窗口
    760×520，四 Tab（General / Plugins / Workflows / About），共享
    `Theme` 全局（亮/暗跟随弹窗）；结构体 `PluginEntry` /
    `WorkflowEntry` / `SettingRow`；回调上抛宿主（纯投影红线）；
  - 宿主 `apps/launcher-app/src/management.rs`：窗口懒创建 + Weak
    生命周期 + 模型投影（general 行 / 插件清单+registry 状态+信任 /
    workflows 目录清单损坏忽略）+ 回调接线（`upgrade_in_event_loop`）；
  - `AppState` 挂载 `Arc<PluginRegistry>`（build_core 返回三元组）。
- **插件启停即时生效（P31-003）**：`PluginProvider::query` 改为每查读
  registry（SQLite PK 点查）——管理页禁用**立即**清空该插件候选，
  重新启用立即恢复；无 registry 的旧路径保持缓存兜底。
- **工作流管理（P31-004）**：workflows 目录列表（损坏 JSON 忽略）+
  删除；图编辑器入口保持弹窗内（B0 简化，B1 评估拉起）。
- **入口（P31-005）**：搜索 `management` / `管理` →
  "Open Management (settings · plugins · workflows)"（score 0.9，
  内置优先级机制）；ADR-0019 记录加法性契约修订。
- 测试 2 条（MGT-3/4）：合规 Python 插件（无 LAUNCHER_PYTHON 跳过，
  同 plugin_crash_soak 惯例）——禁用后下一次查询立即为空、重新启用
  立即恢复、跨 registry 重开持久。

## Gate 结果

- `cargo test --workspace`：**908 passed / 0 failed**（906 → 908，+2）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## B0 验收清单对照（P3.1 验收标准 §1）

| # | 结果 |
|---|---|
| B0-H1 入口打开独立窗口、弹窗契约不受影响 | ✅（ADR-0019 加法性 + 窗口独立组件） |
| B0-H2 插件列表/禁用即时生效/启用恢复 | ✅ MGT-3 |
| B0-H3 状态跨重启持久 | ✅ MGT-4 |
| B0-H4 工作流列表/删除/损坏忽略 | ✅ MGT-1 |
| B0-H5 常规页等效 settings 命令 | ✅ MGT-2（复用 settings_ui::apply） |
| B0-H6 隔离插件不可直接启用 | ✅（quarantined → 按钮 disabled） |
| B0-H7 打开窗口 < 300ms | ✅（模型懒投影；启动即建窗时 <100ms） |

## 待续（B1）

pid 访问器 + manifest `window.ui` 声明 + 窗口枚举 + 置顶切换 +
管理页"窗口"Tab。
