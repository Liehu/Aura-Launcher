# 164 — P3.0 UX 打磨：无控制台 release + 内置功能优先级

> 日期：2026-09-10。用户反馈两项：① release 启动出现日志终端；② 找
> "启动器设置"要划到最后（内置命令被文件索引结果压到后面）。

## 交付

1. **release 隐藏控制台（开关 = 构建档位）**：
   `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`——
   release 无终端窗口；debug 保留控制台供开发。日志不受影响：rolling
   file appender（`docs` 日志目录）照常写入；panic hook 也走 tracing
   文件（release 下不依赖 stdout）。
2. **内置功能优先级（对标 Wox/uTools/Raycast：内置/插件 > 文件索引）**：
   复用既有 `plugin_hint` 机制（ADR-0011：hint 上限 < title exact，
   不破坏 `exact > prefix > contains` 不变量）——
   | 内置命令 | score（hint = score × 45） |
   |---|---|
   | answers | 1.0（既有） |
   | settings-ui 全组 | 0.9 |
   | editor / agent 触发命令 | 0.8 |
   | recent "Run again" 重跑项 | 0.6 |
   | 安装的工作流（catalog） | 0.7 |
   叠加 title_prefix 60 + type_prior 15 后，内置命令总分 ≥115，
   稳定压过文件命中（文件无 hint，且 type_prior=0）。
3. **中文关键词**：settings-ui keywords 增加 设置/配置/主题/自启/索引；
   editor 增加 编辑器/工作流；agent 增加 智能/助手——中文查询不再
   因为标题是英文而错过内置命令。
4. **VR 基线重生成**：search-hint placeholder 文案是有意的视觉变更，
   dark 基线 15 张重新生成；实测两次完整运行 30 张 BMP 逐字节一致
   （确定性保持）；light 子目录照常产出。

## Gate 结果

- `cargo test --workspace`：**906 passed / 0 failed**（不变）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 验证

- `chr`/`设置` 类查询中设置命令进入首位区间（score hint 45 + 标题
  前缀 60 + 先验 15 ≈ 120 > 文件典型命中）；
- release 构建启动无终端（windows_subsystem）；debug 构建保留；
- VR：确定性 ✓、基线已重生成 ✓、light 子目录 15 张 ✓。
