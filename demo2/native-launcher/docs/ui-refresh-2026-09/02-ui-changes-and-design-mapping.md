# UI 改动清单与设计对照（rubick-referenced UI Refresh 2026-09）

> 基线 `4025841` → 终点 `HEAD`。四批提交，每批独立可回滚。
> 改造前截图：`.openclaw-baseline/baseline-before-{dark,light}/`；改造后：`.openclaw-baseline/release-dark{,/light}-png/`。
> 设计风格：柔和科技精致（preset 17 Takram）—— 圆角柔影、谦逊克制的层次，契合启动器工具类观感。

## Batch 0 — `41a1714` fix(vr): VR capture honors progressive disclosure

- **改了什么**：app.slint 新增 `public function set-query(t)`（与既有 reset-query 对称）；visual_scenarios.rs 的 DemoState 增加 `query` 字段并在有结果的场景（VR-002..007、014、015）预填 `"demo"`。
- **为什么**：VR 截图设施与现行渐进披露逻辑脱节——结果渲染要求非空查询，场景从未设置，导致 Main.Results 截图与 Main.Empty 字节相同（回归设施失效）。修复后 VR-001≠VR-002，回归验证重新可信。
- **对照**：修复前基线截图的 VR-002 即空态；修复后 VR-002 可见 3 行结果。

## Batch A — `37e4bf3` feat(theme): Takram soft-tech refresh + rubick-aligned row metrics

- **改了什么**（全部通过既有令牌，无组件级字面量）：
  - 圆角层级：small 4→6、medium 6→10、large 8→14（Takram 柔和圆角）
  - 暗色表面整体柔化一档：bg-surface #111822→#121A26、elevated #16202C→#18222F、input #0F1721→#131C28、选中 #173F73→#1D4478（hover #1B4A86→#215084）；边框对比降低
  - 亮色同步：选中 #D6E8FF→#DCEDFF、边框 #D8DEE6→#DCE2EA
  - 行度量对齐 rubick：搜索行 36→52px、结果行 44→56px、Action 行 40→44px
- **为什么**：rubick 的搜索区（60px）与结果行（70px）明显更舒展；Takram 圆角层级消除原 4/6/8px 的生硬感。窗口高度内容驱动，令牌变更自动适配，无布局硬编码风险。
- **对照**：before/after 的 VR-001/002/005 并排即可见搜索框加高、行距舒展、圆角柔和。

## Batch B — `6ff8277` feat(ui): rubick-referenced search panel

- **改了什么**：
  - SearchBox 重写为自绘无框输入（52px、radius-medium、聚焦态 accent 描边 + 左侧 3px accent 起笔条——rubick logo 槽位的克制演绎）。公共 API（text/query-changed/accepted/key-pressed/focus-input/set-text）不变，AppWindow 零改动
  - ResultRow 改为 rubick `a-list-item-meta` 式两行结构：28px 图标槽 + 标题上/副标题下（原：标题左 + 副标题右 320px）；选中高亮从通栏改为**内缩圆角胶囊**；行底 1px 分隔线（60% 透明度）
  - ResultList 空态两行文案（主提示 + 指导 caption）
  - ContextHint 字号 11→13px、左右留白增加
- **为什么**：rubick 的列表信息组织（两行 meta）在窗口宽度内可读性显著更好；内缩圆角胶囊与 Batch A 的圆角语言呼应（基线目检曾指出选中高亮与搜索框圆角不一致）。
- **对照**：VR-003（选中态）before 通栏高亮 → after 悬浮胶囊；VR-002 行结构完全不同。

## Batch C — `8176546` feat(ui): management window polish

- **改了什么**：左菜单项改为内缩圆角胶囊 + hover 态 + 当前项 3px accent 指示条；内容卡片行加高（General 48 / Plugins 56 / Workflows 52 / Windows 52 / targets 40px）与 20px 横向留白；ThemedButton 26→30px + radius-sm；About 页标题下加分隔线。
- **为什么**：对齐 rubick（ant-design）设置页的行式卡片观感与 hover 反馈；原菜单项 40px 无圆角无 hover，与主弹窗的新视觉语言脱节。
- **对照**：管理窗口 General/Plugins/Windows 各 tab 截图（见回归记录）。

## 兼容性与红线核对

- ✅ 未复制 rubick 任何源码/资源（仅布局与交互观感参照）
- ✅ UI 契约测试 `tests/ui_contract.rs` 全绿（无执行器词汇/无禁用依赖/viewmodel 只读）
- ✅ 未动 Rust 端业务逻辑（唯一 host 改动是 Batch 0 的 VR 场景预填查询，属验证设施）
- ✅ 明暗两主题同步调整；`anim-ms` 动画机制未动（快照确定性保持）
- ✅ 品牌保留：应用名 Native Launcher / 托盘图标未变
