# 147 — P2.7 Batch 16：C03–C07 宿主接线（Agent Loop → launcher-app）

> 日期：2026-09-09。范围：P2.7 第十六批（146 号交接优先级 1）。
> 输入基线：history/146（824 tests）。

## 交付

- `apps/launcher-app/src/agent_service.rs`（新）——`run_agent` 编排核心
  正式接入 launcher-app，闭环 C03–C07：
  - **触发源（C03）**：`AgentCommandProvider`（`agent <goal>` 查询前缀），
    经 host 侧 namespace 路由（provider_id `agent`，与
    workflows/settings 同模式）进入 `start_agent_run`——引擎永远看不到
    agent 命令（不落 Open target）。
  - **执行边界（C03/C04）**：`CoreTurnExecutor` 把 plan step 的
    `action_ref`（`provider|command|action`，目录投影同 key 回显）解析为
    ActionProposal，经 `CoreAgentHost`（launcher-core）走冻结
    Resolver→Engine 链——loop 决定 WHAT，Core 决定 HOW。
  - **取消（C06）**：`ACTIVE_CANCEL` 槽位 + Esc（workflow dismiss）置位，
    步间轮询生效；在途效果从不被打断。
  - **持久化（C07）**：`<data>/agents.db`（AgentSessionStore，SQLite/WAL，
    损坏重建）；运行前 save(running)、终态 finish。v1 持久化为
    observation-only（store 失败不阻断运行）。
  - **LLM（§22）**：`OpenAiCompatibleProvider`，端点来自 config 新
    `[llm]` 节（base_url/model/timeout_secs）；API key 只从
    `LAUNCHER_LLM_API_KEY` env 读取（review 55 §38，永不入配置）。
    无 `[llm]` = agent 命令降级为明确 status 提示，绝不 panic。
  - **UI**：复用 Workflow Runtime Surface（VR-013 投影形态）显示
    Plan/Execute 两相进度，终态回写 status 行并关闭 surface。
- `crates/launcher-config`：`LlmConfig` + `AppConfig.llm: Option<LlmConfig>`
  （serde default None，向后兼容；Default/roundtrip 测试同步）。

## Gate 结果

- `cargo test --workspace`：**824 passed / 0 failed**（接线为集成层，无新
  单测——loop/序列化语义已由 launcher-ai 覆盖）
- `cargo build --workspace`：零警告；`check_topology.py`：ok（17 crates + 9 apps）

## 待续（更新后优先级）

1. P2.7 B01 确认（Tool Catalog 投影已有 catalog.rs 基础）
2. P2.9 Windows Adapter（逐 Adapter 接真实 Win32）
3. P2.6 E02–E05 Slint 画布 VIEW
4. P2.7 D–H（触发源深化/安全/质量/Release；含交互式 clarify 循环 §18）
5. MSIX 签名（等外部证书）；Pinyin 完整拼音表（优化项）
