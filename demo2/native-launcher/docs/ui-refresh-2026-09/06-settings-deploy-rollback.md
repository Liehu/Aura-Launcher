# 管理面板选项 UI：部署说明与回滚步骤（2026-09 第二轮）

## 1. 发布步骤

```powershell
cd E:\Aura-Launcher\demo2\native-launcher
cargo test -p launcher-ui -p launcher-config        # 门禁：32 项测试全绿
cargo build --release -p launcher-app               # 生产构建
python scripts/package.py --skip-build              # 打包 zip + msix 到 artifacts\dist\
```

安装（升级安全，用户数据保留）：
```powershell
Expand-Archive artifacts\dist\NativeLauncher-1.0.0-win64.zip -DestinationPath <临时目录>
powershell -ExecutionPolicy Bypass -File <临时目录>\install.ps1
```

## 2. 上线后验证清单（抽查）

1. 启动后搜索「管理」→ 回车打开管理窗口
2. General 页：五组标题齐全；主题模式分段选择器点击 light → 状态条变绿「已保存」→ 弹窗与管理窗口立即变亮色
3. 开关「开机自启」点击 → 状态条确认 → 重启应用后开关状态保持
4. 滑杆「结果条数」拖动 → 数值跟随 → 重启后保持
5. 呼出热键框点击 → 按 Ctrl+Alt+K → 显示新组合 → 重启后生效（热键重注册需重启）
6. 重置按钮 → 热键恢复 Ctrl+Space
7. 检查台账：%APPDATA%\NativeLauncher\settings-audit.jsonl 每次修改新增一行（ts/key/old/new/source）
8. Plugins / Workflows / Windows / About 页签功能不变（零回归）

## 3. 回滚步骤（演练验证过）

- **整体回滚**：`git revert <本轮首提交>^..HEAD`（本轮提交粒度见 git log，均为普通顺序提交，无冲突风险）后重新 `cargo build --release` + 打包
- **仅回滚 UI 层**：`git checkout <上一稳定版> -- demo2/native-launcher/crates/launcher-ui` + host 侧 `git checkout <上一稳定版> -- demo2/native-launcher/apps/launcher-app/src/management.rs apps/launcher-app/src/main.rs demo2/native-launcher/crates/launcher-config` 后重构建
- **运行时回退**：直接用上一稳定版 zip 覆盖安装（install.ps1 幂等，用户数据/台账不受影响）
- **台账不可回滚**：settings-audit.jsonl 为只追加记录，回滚代码不影响既有台账（审计连续性是有意设计）

## 4. 已知限制与风险

| 项 | 说明 | 规避 |
|---|---|---|
| 管理窗口无标题栏 | no-frame 修复布局空间溢出；窗口不可拖动 | 左菜单 Close 按钮关闭；托盘菜单可重新打开；后续可加自绘拖动区 |
| 热键修改需重启 | 热键注册在启动时完成 | 状态条已明示「重启后生效」 |
| 台账无操作人字段 | 单用户桌面应用，source 固定 "management"/"settings_apply_check" | 多账户场景需先有账号体系（范围外） |
| example 截图/验证设施 | management_shot / settings_apply_check 为 examples，不进产物 | 保留作为回归设施 |
