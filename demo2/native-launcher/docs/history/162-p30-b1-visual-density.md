# 162 — P3.0 Batch B1：视觉与信息密度（F05–F08）

> 日期：2026-09-10。范围：P3.0 第二批。输入基线：history/161（904 tests）。

## 交付

- **F05 双主题（P30-011/012）**：
  - `theme.slint` 调色板参数化：`light-theme` 开关驱动全部色彩 token；
    **暗色字面值逐字节不变**（VR-001..015 暗基线保持有效）；
  - 亮色调色板（surface/border/text/semantic/state 全套）；
  - config `theme_mode = dark|light|system` + 热重载（MUST-2 链路）；
    `system` 跟随注册表 `AppsUseLightTheme`；弹窗显示路径与配置变更
    路径都重新解析调色板；`LAUNCHER_THEME_MODE` env 覆盖（VR/测试用）。
- **F13 微动效（P30-013）**：
  - 选中行背景 ≤80ms 过渡；Workflow/Editor surface 显示淡入
    （`Theme.anim-ms`，默认 120ms）；
  - **UX-R3**：`anim-ms` 在快照模式钉为 0——实测两次运行 30 张 BMP
    逐字节一致（含 light 子目录）。
- **F07 元数据 + 详情面板（P30-014）**：
  - 文件行副标题升级为 `路径 · 大小 · 修改时间`（纯投影，数据源自
    `IndexedFile`；civil 日历算法无 TZ 依赖，附单测）；
  - Tab 详情面板（Main 模式、有结果时）：标题/副标题/命令 id/动作数，
    Esc 或 Tab 关闭，淡入动效。
- **F15 自绘控件（P30-015）**：`ThemedButton` 原语（hover/press/焦点
  态 + 动效）替换 Editor Surface 的 std Button；SearchBox 的 LineEdit
  保留 std 实现（IME/焦点风险控制，后置评估）。
- **VR 双基线**：`capture_pass` 双 pass 架构——dark（dir）+ light
  （dir/light），pass 完成经 ack channel 等待后才退出事件循环；
  实测：30 张 BMP 两次运行逐字节一致；暗色 15 张与冻结基线逐字节
  一致（**主题重构零回归**）；light 与 dark 差异确认（调色板生效）。
  Release gate 的非递归 glob 不受 light 子目录影响（仍 15 张顶层）。

## Gate 结果

- `cargo test --workspace`：**906 passed / 0 failed**（904 → 906，+2：
  元数据格式化单测）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## B1 验收清单对照（P3.0 验收标准 §2）

| # | 结果 |
|---|---|
| B1-H1 三模式生效、无硬编码色 | ✅ TH-1/TH-2（tokens 全覆盖） |
| B1-H2 VR 亮/暗 × 全场景 diff 通过 | ✅ TH-3（30 张确定性重生成） |
| B1-H3 快照逐字节确定性 | ✅ MO-1（两次运行一致） |
| B1-H4 动画 ≤200ms | ✅ MO-2（80/120ms，可Theme调） |
| B1-H5 元数据纯投影 | ✅ MD-1（单测） |
| B1-H6 Tab 面板/导航回归 | ✅ MD-2/WG-1（key 拦截协议未变） |
| B1-H7 popup 性能不退化 | ✅ 动效走 GPU 合成层，无主线程定时器 |
| B1-H8 全量回归 | ✅ 906 / 0 warn / topology ok |

## 待续（B2）

空查询 Workflow/Agent 分组、placeholder 提示、`settings:ai`、富结果
协议草案。
