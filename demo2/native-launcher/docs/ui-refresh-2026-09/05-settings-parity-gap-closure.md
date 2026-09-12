# 差距关闭记录：管理面板选项 UI × rubick

> 承接《05-settings-parity-gap-list》（2026-09 第二轮）。基准：rubick feature/src/views/settings。
> 改造对象：management.slint General 页 + 新控件原语 + host 接线 + 台账。
> 走查方式：明/暗主题截图逐项核对（examples/management_shot 截图设施）+ 真实链路验证（examples/settings_apply_check）。

| # | 差距项 | 动作 | 状态 | 证据 |
|---|---|---|---|---|
| G1 | 分组标题 | General 页按 外观/行为/性能/快捷键/索引目录 分组，accent 色 14px 加粗标题行 | ✅ 已关闭 | management-dark.png 五组标题可见 |
| G2 | 行式两端对齐 | label+说明左、控件右（space-between，stretch 空隙）；修复 alignment:start 使 stretch 失效的问题 | ✅ 已关闭 | 目检：控件统一贴右缘 |
| G3 | 真开关 | primitives/toggle-switch.slint：token 化轨道+滑块、checked 动画、hover/禁用/焦点环、点击+空格切换 | ✅ 已关闭 | 截图两个开关一开一关；空格路径实现 |
| G4 | 下拉选择 | primitives/segmented.slint：theme_mode 三值分段（system/light/dark），选中 accent 反白 | ✅ 已关闭 | 截图 system 段选中态 |
| G5 | 快捷键捕获 | primitives/hotkey-capture.slint：点击进入捕获态、修饰键+主键组装、Esc 取消、重置默认按钮；host 校验至少一个修饰键 | ✅ 已关闭 | 管理窗口热键框+重置按钮渲染 |
| G6 | 滑杆数值 | primitives/settings-slider.slint：result_limit 4..=16，点击/拖动/方向键，实时数值 | ✅ 已关闭 | 截图滑杆+数值 12 |
| G7 | 即时保存+反馈 | 每次修改原子写 config.toml（既有 tmp+rename）+ 底部状态条「已保存 · key = value」绿点/红点 | ✅ 已关闭 | 状态条渲染确认；apply_check PASS×3 |
| G8 | 修改台账 | launcher_config::audit_setting_change → %APPDATA%\\NativeLauncher\\settings-audit.jsonl（ts/key/old/new/source JSONL） | ✅ 已关闭 | apply_check：台账 +3 条 PASS |
| G9 | 键盘可达 | 控件自含焦点域（空格切换/方向键步进/Esc），Tab 走 Slint 内置焦点链；开关焦点环可见 | ✅ 已关闭 | 控件键盘路径实现 |
| G10 | 控件全状态 | 每控件定义默认/hover/聚焦/禁用（toggle/segmented/slider/hotkey 均含 enabled+has-focus+has-hover 分支） | ✅ 已关闭 | 源码 + 截图 |
| G11 | 观感层级 | 组标题 accent、行卡片 radius-md、desc caption 弱化、状态条分隔 | ✅ 已关闭 | 截图 |
| G12 | 长文本/滚动 | 窗口加高 640 + 布局去 Flickable（固定 9 项无需滚动）；label/desc elide | ✅ 已关闭（以固定项数+elide 满足） | 截图无裁切 |
| G13 | 主题跟随 | 全部新控件走 Theme 令牌双主题 | ✅ 已关闭 | 明暗两张截图 |

**保留理由项（不关闭）**：
- rubick 的「账户信息/数据库/内网穿透」页签、全局快捷键多配置增删：Aura 无对应业务模块（Goal 范围外），不引入。
- rubick 图标+文字侧栏：当前文字胶囊菜单已带 hover/accent 指示；图标资源属新增资产，列入后续建议（P3）。
- 滑杆数值 "12" 右缘微调：可接受偏差，列入后续建议（P3）。

**过程中发现并修复的基础设施问题**（与本差距无关但影响验收）：
1. VR 截图设施与渐进披露脱节（Main.Results 与 Main.Empty 字节相同）——上一轮已修（41a1714）。
2. **管理窗口标题栏陷阱**：`Window.height` 是含系统标题栏的**外框高**，client 比 布局空间少 ~31px，底部 31px 的布局空间（状态条）整段落在 client 外不可见——本 round 反复排查后确认，修复为 `no-frame: true`（与主弹窗一致，client==布局空间）。侧边栏自带 Close 按钮，无标题栏后窗口拖动能力缺失，记入已知限制。
