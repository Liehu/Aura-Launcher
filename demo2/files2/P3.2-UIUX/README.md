# P3 UI/UX Agent Tasks

本目录包含 Aura Launcher P3 UI/UX Design Specification v1.0 拆分后的 10 个独立 Agentic Coding Task。

## 执行顺序

P3-A → P3-B/C/E → P3-D → P3-F/G/H → P3-I → P3-J

## Agent 通用规则

1. 开始前先阅读当前仓库真实代码，不得假设文件路径。
2. 优先复用已有 domain model、service、trait、router 和 command。
3. 不得为了完成 UI 任务复制一套新的业务逻辑。
4. UI 不得绕过 ActionResolver 直接执行命令、进程或插件。
5. 每个 Task 必须运行规定测试并提交截图证据。
6. 若发现现有架构与 Task 假设不一致，应在报告中记录，而不是擅自扩大修改范围。
7. 完成后按 Task 中的最终报告模板提交结果。

## Task Index

- P3-A: UI State Architecture
- P3-B: Launcher Shell
- P3-C: Search Result Contract + Ranking
- P3-D: Result Grouping + Keyboard Navigation
- P3-E: Unified Action + Context Menu
- P3-F: Plugin Context
- P3-G: Plugin Center
- P3-H: Settings + System Tray
- P3-I: Visual System + Accessibility
- P3-J: UX Integration + Regression Gate
