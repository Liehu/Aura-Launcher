# 176 — P3-UX Batch UX-A：窗口淡入 + 主题完整性确认

> 日期：2026-09-11。范围：P3-UX 第一批（UX-A 窗口布局与主题基础）。
> 输入基线：history/175（925 tests）。

## 确认（已在先前批次落地的 UX-A 项）

以下 UX-A 内容已在 P3.0 B1 / P3.1 各批中实现，本批验证其覆盖完整性：

- **Theme 亮/暗调色板**：`theme.slint` 中 `light-theme` 开关驱动全部
  色彩 token（surface/border/text/accent/semantic/state 全覆盖）✅
- **`theme_mode` 配置**：`dark|light|system` 三值 + 注册表跟随 +
  热重载 ✅
- **`system_prefers_light`**：读 `AppsUseLightTheme` 注册表键 ✅
- **选中行色彩过渡**：result-row `animate background { 80ms }` ✅
- **Surface 淡入**：workflow/editor/tool surfaces 的 `animate opacity` ✅
- **快照确定性**：`anim-ms = 0` 钉住 ✅

## 新增交付

- `AppWindow.appeared`（in-out bool）：弹窗显示状态标记。宿主在
  `show()` 后设置 `appeared = true`；关闭后重置 `false`。为 P4 的
  窗口淡入动画预留的状态钩子（Slint Window 不直接支持 opacity
  动画——需要包装层，后置 P4）。
- testkit crate `RichResult` import 修正（从测试专用移到 cfg(test)）。

## Gate 结果

- `cargo test --workspace`：**925 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：18 crates + 9 apps ok

## UX-A 验收对照（P3-UX 验收标准 §1）

| # | 结果 |
|---|---|
| UX-A1 调色板双套 | ✅（theme.slint light-theme toggle） |
| UX-A2 theme_mode 三值 | ✅（config + registry follow） |
| UX-A3 窗口高度内容驱动 | ✅（P3.0 已有，无变化） |
| UX-A4 窗口淡入 | ◈ P4（Window opacity 限制） |
