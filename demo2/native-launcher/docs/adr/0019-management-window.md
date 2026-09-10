# ADR-0019 — Management Window（独立管理窗口）

日期：2026-09-10 ｜ 状态：ACCEPTED

## 背景

P3.0-F04 的命令式设置面已就绪，但插件启停/信任、工作流管理需要成组的
列表化 UI。选项：弹窗第四 Mode（修订 FROZEN UI-CONTRACT，620px 太挤）
vs 独立窗口（uTools 模式）vs WebView（违反红线）。

## 决策

**独立 Slint 窗口**（`ManagementWindow`，760×520，四 Tab：
常规/插件/工作流/关于），同进程、共享 Theme 全局（亮/暗跟随）。

- 弹窗 UI-CONTRACT **零语义变更**；本 ADR 仅批准**加法性入口**：
  1. `management` / `管理` 搜索命令（provider_id `management`）；
  2. 设置面 `settings:manage` 命令；
  3. AppWindow 增补 `in-out property <string> search-hint` 与
     `in-out property <bool> detail-visible`（P3.0 已落地，本 ADR 追认）
     与 `ManagementWindow` 组件导出。
- 数据通道沿用纯投影红线：管理窗口不持逻辑，回调上抛宿主
  `management.rs`。
- 插件启停即时生效：`PluginProvider::query` 改为每查读 registry
  （SQLite PK 点查），替代启动时的 disabled 缓存。

## 后果

- 正面：管理面可自由扩展（Tab 加页）；弹窗契约冻结不动；
- 负面：第二窗口的 DPI/主题/生命周期需要独立接线（一次性成本）；
- 中性：管理窗口不进 VR 基线（手工清单验收）。
