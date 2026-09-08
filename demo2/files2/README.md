# Native Launcher v0.1 文档包

这是一个面向 **Agentic Coding** 的原生桌面效率启动器项目 v0.1 设计基线。

目标不是复制某个既有产品，而是融合：

- Wox / Flow Launcher：插件、脚本、Command/Provider
- Listary / Lertaro：文件搜索、上下文、Quick Switch、文件索引
- Raycast：Command、Action、Workflow、Extension 思维
- Asyar：权限、Sandbox、AI/MCP 思维
- uTools：插件开发体验与功能组织

核心技术方向：

- Rust
- Slint Native UI
- Windows-first MVP
- 不使用 Electron / CEF / WebView 作为主 UI
- 插件运行时按需启动
- Core / UI / Indexer / Plugin Runtime / AI 解耦
- Agentic Coding 作为主要开发方式

## 文档

1. `01-design-spec-v0.1.md`：系统设计规范
2. `02-agentic-coding-development-spec-v0.1.md`：Agentic Coding 开发规范
3. `03-test-plan-v0.1.md`：测试与性能门禁
4. `04-mvp-scope-v0.1.md`：MVP 范围、里程碑与验收清单

## v0.1 冻结原则

### 冻结

- Rust 为核心实现语言
- Slint 为 MVP 原生 UI
- Windows 10/11 优先
- Core / UI / Indexer / Plugin Host 分进程设计
- Command / Provider / Action 作为统一领域模型
- Script Runtime 默认按需启动
- 插件不得直接访问 UI 内部状态
- UI 使用 Native UI Schema，不允许插件嵌入 HTML/WebView
- 性能预算进入 CI/验收门禁
- 所有 Agent 修改必须通过测试与审查门禁

### 暂不冻结

- 最终插件 ABI
- WASM Runtime 具体实现
- Python/Node 宿主的长期方案
- 跨平台详细实现
- Marketplace
- AI Agent 完整能力
- Workflow 完整 DSL
- 自动更新和云同步

## MVP 成功标准

在普通 Windows 开发机上完成：

- 全局热键唤起 Launcher
- 原生搜索框 + 结果列表
- 应用启动
- 文件搜索
- 基础上下文探测
- Action 执行
- 一个内置 Provider + 一个外部进程插件
- Indexer 独立进程
- 100 个测试插件/虚拟 Provider 压测时 Core 不发生线性内存增长
- 空闲常驻目标：Private Bytes <= 80 MB；目标值 <= 50 MB
- 热键到可见窗口 P95 <= 35 ms（目标，不作为首轮绝对硬门槛）

> 注：这些数值是工程预算，不代表任何第三方产品的实测数据。
