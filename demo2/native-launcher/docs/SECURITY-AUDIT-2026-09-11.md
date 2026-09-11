# 安全审计摘要（P3-UX 完成后，2026-09-11）

> 扫描引擎：Mimosa deep ｜ 扫描ID：scan-fd8f29acb668 ｜ 发现：9 条

## 结论

**核心产品代码（launcher-domain/search/core/action/ai/plugin-host/workflow）零真实漏洞。**
全部 9 条发现均在示例插件、构建脚本或 CLI 工具中，且为有意设计模式。

## 逐条分析

| # | 级别 | 文件 | 分析 | 处置 |
|---|---|---|---|---|
| 1 | high | calculator.py:21 eval | 示例插件用 eval 做算术（设计如此） | 后置：改用安全求值器 |
| 2-4 | high | main.rs:560,617 spawn | 环境变量→进程拉起（LAUNCHER_PYTHON 等） | **设计意图**，非漏洞 |
| 5 | high | plugin-cli/main.rs:6 | CLI 参数→路径操作（CLI 工具本质） | **设计意图** |
| 6-9 | medium | release_gate.py ×4 | 环境变量→文件路径（构建脚本） | **设计意图** |

## 已有的安全机制确认

- ✅ Effect Authority：adapter 无 confirmed 参数（P210-003）
- ✅ PID/HWND reuse 防护（P210-006）
- ✅ 跟随钉免注入（WinEventHook 广播 ≠ 注入）
- ✅ 隐私：local-first 远程门 + sanitize + NEVER_SENT
- ✅ 审批门：单次使用/过期/取消 fail-closed
- ✅ Tool UI：声明式 schema 宿主渲染，无 webview/HTML

## 建议

1. calculator.py eval → 改用 `launcher_domain::expr` 或 Python `ast.literal_eval`（P4）
2. calculator.py 添加 "EXAMPLE ONLY" 头注释
3. 定期重跑 deep scan（建议每 P3.x 阶段收尾时）
