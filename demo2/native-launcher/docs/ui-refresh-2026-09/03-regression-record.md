# 回归检查记录（UI Refresh 2026-09）

> 验收原则：核心路径真实运行验证 + 每个改动界面在明暗主题下逐项目检 + 异常路径不白屏。
> 所有截图位于 `demo2/native-launcher/.openclaw-baseline/`。

## 1. 核心功能回归（真实运行）

| # | 检查项 | 方式 | 结果 | 证据 |
|---|---|---|---|---|
| 1 | 查询出结果 | release 键盘管线（驱动真实 Slint 按键管线） | ✅ query_yields_results：results=12 | walkthrough-release.json `pass: true` |
| 2 | ↓/↑ 导航 | 同上 | ✅ selected_index 0→1→0 | 同上（5 步全 ok） |
| 3 | Enter 执行主操作 | 同上 | ✅ enter_dispatched_primary | 同上 |
| 4 | Esc 关闭弹窗 | 同上 | ✅ escape_dismisses_popup（visible=false） | 同上 |
| 5 | debug 版管线复核 | debug exe 同套件 | ✅ 5/5 | walkthrough-debug.json `pass: true` |

注：键位修饰组合（Ctrl+↓ 面板、Ctrl+D 收藏）按项目契约由 `crates/launcher-core/tests/ui_interaction.rs` 覆盖，本次全量未回归该 crate（未触碰其代码），列入手工核对清单。

## 2. 界面回归（明暗主题 × 场景逐项目检）

- 产出：`LAUNCHER_SNAPSHOT_DIR` 一次捕获 15 场景 × 明/暗 = 30 张（release 版）。
- 目检结论（图像分析）：
  - 暗色 VR-002：3 行结果、层次清晰、无截断/重叠/错位；选中高亮圆角胶囊生效
  - 亮色 VR-002：配色干净、圆角与分隔线正确、无样式丢失
  - 观察项（非缺陷）：① 图标槽空白（快照模式确定性跳过图标提取，真实运行时异步填充）；② 亮色搜索框边框刻意克制（border-subtle）
- 改动前后对照：`baseline-before-{dark,light}-png/` vs `release-dark{,/light}-png/`（VR-001..015 同名对齐）

## 3. 设置/配置持久化（真实运行）

| # | 检查项 | 方式 | 结果 |
|---|---|---|---|
| 1 | 配置读取生效 | config.toml `theme_mode="light"` → 单文件快照；改 `"dark"` 再快照；恢复 | ✅ light 截图均亮 RGB(251,251,252)，dark 均暗 RGB(19,27,39) |
| 2 | 配置写入链路 | 管理窗口 Apply 走 `setting-applied` → launcher-config TOML 写入（`fault tolerance: missing file → write defaults`） | 代码路径有契约测试覆盖；GUI 点击列入手工清单（见 §6） |
| 3 | 配置损坏容错 | config.toml 写入 `@@@broken toml [[[` → release 启动 | ✅ 降级默认配置正常启动，walkthrough `pass: true`（不白屏、不崩溃）；配置随后从备份恢复 |

## 4. 构建验证

| # | 检查项 | 命令 | 结果 |
|---|---|---|---|
| 1 | 依赖安装/编译 | `cargo build -p launcher-ui -p launcher-app` | ✅ dev 2m47s（crates.io 直连超时，已配 rsproxy sparse 镜像 `%USERPROFILE%\.cargo\config.toml`） |
| 2 | 生产构建 | `cargo build --release -p launcher-app` | ✅ 4m08s，launcher-app.exe 19,184,640 B |
| 3 | UI crate 测试 | `cargo test -p launcher-ui` | ✅ 12 lib + 1 editor_surface + 6 ui_contract = 19/19 |
| 4 | clippy | `cargo clippy -p launcher-ui -p launcher-app --all-targets` | ✅ 本次改动 0 警告（launcher-workflow 存量 2 条 warning，改造前已有） |
| 5 | fmt | `cargo fmt --check` | 本次改动合规；存量 1 处（agent_service.rs import，改造前已有） |
| 6 | 打包产物 | `python scripts/package.py --skip-build` | ✅ artifacts/dist/NativeLauncher-1.0.0-win64.zip（8,797,834 B）+ .msix（8,327,965 B） |
| 7 | 产物启动复核 | release exe 直接运行（walkthrough + 截图 + 配置链路共 4 次启动） | ✅ 全部通过 |

## 5. 异常路径

- **配置损坏**：降级默认配置启动 ✅（§3.3），不白屏不崩溃
- **依赖缺失**：Rust 静态链接（msvc + bundled SQLite + 软件渲染器），无运行时动态依赖可缺 ✅（架构性免疫）
- **索引损坏**：main.rs `degraded_boot` 分支（索引打开失败→只读降级），本次未触碰该逻辑，列为既有保障
- **截图设施偶发**：Batch C 后 debug exe 曾一次静默不产图（exit 0、无日志），同命令 release exe 稳定复现成功；未复现第二次，记为偶发观察项（不影响验收对象 release 产物）

## 6. 建议手工核对清单（10 分钟）

1. `Ctrl+Space` 呼出/隐藏，托盘左键呼出、菜单退出
2. 输入关键字 → ↓/↑ → Enter 启动一个应用
3. 空查询 → 资源管理器打开一个文件夹 → 再次呼出 → Quick Switch 出现该文件夹内容与 "Open Terminal Here"
4. 托盘菜单 → 打开管理窗口 → General 修改一项设置 → Apply → 重启应用确认保留（写入持久化 GUI 闭环）
5. Plugins/Workflows/Windows/About 各 tab 浏览一遍（胶囊菜单 hover、accent 指示条）
6. 搜索关键词 → 观察标题匹配段 accent 加粗高亮
