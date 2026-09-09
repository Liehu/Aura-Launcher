# 152 — P2.7 Batch 19：E 线 Memory/Privacy（E01–E06）

> 日期：2026-09-10。范围：P2.7 第十九批（`P2.7 开发设计规范` §26/§30/
> §31/§32，151 号交接优先级 1）。输入基线：history/151（842 tests）。

## 交付

- `crates/launcher-ai/src/memory.rs`（新，**E01/E02/E03/E04**）——§26
  三类 memory 全部有界、显式、可见、可删、分域：
  - **E01 SessionMemory**：按 session 记录 goal/结果条目（每 session
    ≤20 条、全局 ≤8 个 session，FIFO 逐出最旧 **session**——永不跨域
    遗忘）；条目控制字符清洗 + 512 字符截断（memory 永不把协议噪声
    带回 prompt）；
  - **E02 RunMemory**：按 run 记录 goal/status/turns（FIFO ≤20），可按
    run_id 删除；
  - **E03 UserPreferences**：显式 k/v（≤32 键，满员拒新增——绝不无限
    生长），`all()` 全量可见；
  - **E04 管理**：delete_session/delete_run/remove/clear 全套。
- `crates/launcher-ai/src/privacy.rs`（新，**E05/E06**）：
  - **E05 local-first（§30）**：`RemoteAiPolicy` 默认**禁止**远程；启用
    必须显式（config `allow_remote_data = true`），`ensure_remote_allowed`
    返回的错误文案即 §30 要求的"data leaves machine"提示；红线清单
    `NEVER_SENT`（credentials/tokens/全盘文件清单/进程内存）成为契约；
  - **E06 注入防御（§32）**：`sanitize_untrusted`（fence 终结符 ` ``` `
    中和、控制字符剥离、长度截断——fenced 数据节唯一已知逃逸通道封死）；
    `looks_like_instruction_override` 确定性检测器（G05 注入测试矩阵的
    诊断底座）。
- **接线（launcher-app/agent_service.rs）**：
  - 远程门：`[llm]` 存在但未 `allow_remote_data` → agent 命令回显提示，
    绝不发远程请求；
  - E01 接入 loop：最近 5 条 goal 经 `sanitize_untrusted` 后作为
    run_agent 的 context（真实进入 prompt，且有界清洗）；每次 run 结束
    写入 E02 RunMemory。
- launcher-config：`LlmConfig.allow_remote_data`（serde default false =
  local-first）。
- 测试 8 条：session/run/prefs 有界 FIFO、跨域遗忘、控制字符清洗、
  偏好满员拒绝与删除、远程默认关闭/显式开启提示、红线清单契约、
  fence 逃逸中和、override 检测。

## Gate 结果

- `cargo test --workspace`：**850 passed / 0 failed**（842 → 850，+8）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## 待续（更新后优先级）

1. P2.7 F 线（Product UX：AI 搜索面/会话面——多数 UI 基建已备）
2. P2.7 G 线 QA（G04/G05 已随 D/E 批内建核心用例）+ H 线 Release
   （MSIX 签名等外部证书）
3. Pinyin 完整拼音表（优化项）
