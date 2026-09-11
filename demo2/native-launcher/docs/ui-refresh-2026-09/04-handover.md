# 交接手册（UI Refresh 2026-09）

## 1. 环境准备

- Windows 10/11 x64；Rust stable（msvc target）；Python 3（打包/脚本用）
- crates.io 镜像（本次为解决直连超时新增，全局生效）：
  `%USERPROFILE%\.cargo\config.toml` → rsproxy sparse（内容见 §5）
- Slint 1.17.1 由 cargo 自动拉取；无 Node/Electron 依赖

## 2. 常用命令（在 `demo2/native-launcher/` 下）

```bash
cargo build -p launcher-ui -p launcher-app      # 开发构建
cargo run -p launcher-app                        # 运行（Ctrl+Space 呼出）
cargo test -p launcher-ui                        # UI 契约+单测（19 项）
cargo build --release -p launcher-app            # 生产构建
python scripts/package.py                        # 一键 release+打包（zip+msix）
python scripts/package.py --skip-build           # 用现成 release exe 打包

# 验证设施
LAUNCHER_SNAPSHOT_DIR=<dir> launcher-app.exe     # 15 场景×明暗 BMP 截图后退出
LAUNCHER_KEYBOARD_WALKTHROUGH=<report.json> launcher-app.exe   # 键盘管线自检后退出
LAUNCHER_SNAPSHOT=<file.bmp> launcher-app.exe    # 单文件快照（demo 数据）
```

配置：`%APPDATA%\NativeLauncher\config.toml`（hotkey / theme_color / theme_mode / autostart / index_dirs）
日志：`%APPDATA%\NativeLauncher\logs`；数据：`%LOCALAPPDATA%\native-launcher`

## 3. 本次改动清单（git）

| commit | 内容 |
|---|---|
| `41a1714` | fix(vr): VR 截图设施适配渐进披露（set-query + 场景预填） |
| `37e4bf3` | feat(theme): Takram 柔和刷新 + rubick 对齐行度量（仅令牌） |
| `6ff8277` | feat(ui): 搜索面板重写 + 结果行两行结构 + 圆角胶囊选中 |
| `8176546` | feat(ui): 管理窗口打磨（胶囊菜单/卡片行/按钮） |
| 本次最后 | docs: 交接文档集（本目录 4 份） |

改造前基线 commit：`4025841`。工作区在改造开始时是干净的。

## 4. 回滚方式

- **回到改造前**：`git checkout 4025841 -- demo2/native-launcher/crates/launcher-ui demo2/native-launcher/apps/launcher-app/src/visual_scenarios.rs` 后重新构建；或整体 `git revert 41a1714..HEAD`（提交粒度干净，无 merge 冲突风险）
- **单批回滚**：`git revert <commit>`（四批相互独立：Batch 0 是验证设施、A 是令牌、B 是组件、C 是管理窗口）
- **视觉微调**：全部视觉值在 `crates/launcher-ui/ui/theme.slint` 令牌层，改令牌即全局生效，窗口高度自动适配
- **回滚后验证**：重跑 §2 的两个验证设施（walkthrough pass + 30 张截图目检）

## 5. 已知风险与规避

| 风险 | 影响 | 规避 |
|---|---|---|
| VR 基线字面量依赖注释（theme.slint 头部）已过时 | 新旧像素对比（byte-diff）不可用 | 本次已按新截图重建对照；如需恢复 byte-diff，用 release-dark 重新生成基线 |
| 亮色搜索框边框刻意偏淡 | 个别用户可能觉得"边框丢失" | 属设计克制；如需加强调 border-subtle → border-default 一行改动 |
| 快照模式图标槽空白 | 误判为图标缺失 | 设计如此（确定性）；真实运行异步填充 |
| debug exe 截图一次静默失败（未复现） | 验证干扰 | 用 release exe 跑验证设施；已记录 |
| 存量 lint：agent_service.rs fmt、launcher-workflow 2 条 clippy warning | CI 若开 -D warnings 会红 | 改造前已有，非本次引入；建议单独清理 |
| cargo 新环境直连 crates.io 可能超时 | 首次构建失败 | 已配 rsproxy 镜像（§2/本文件末尾附配置） |

## 6. 后续建议（排期建议供决策）

1. （P1，0.5 天）恢复/重建 byte-diff 视觉回归：以 release-dark 30 张为新基线接入 CI
2. （P1，1 天）管理窗口 GUI 设置闭环的自动化（UI 自动化点击 Apply → 验证 config.toml 落盘）
3. （P2，1 天）rubick 空查询历史网格的 native 等价物：需要 host 暴露 recent commands 空查询数据，建议单独立项（涉及 UI-CONTRACT 扩展）
4. （P2，0.5 天）清理存量 fmt/clippy 遗留，开启 `-D warnings` CI 门
5. （P3）亮色主题边框层次微调（border-subtle 在白底上偏淡的观感确认）

## 附：cargo 镜像配置（本次新增）

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
```
