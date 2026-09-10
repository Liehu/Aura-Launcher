# 154 — P2.7 Batch 21：G 线 QA 补强矩阵（G01–G05）

> 日期：2026-09-10。范围：P2.7 第二十一批（`P2.7 开发设计规范` §39-G，
> 153 号交接）。输入基线：history/153（853 tests）。

## 交付

- `crates/launcher-ai/tests/g_line_suite.rs`（新，10 条跨模块集成测试）：
  - **G01 Provider Stress**：BrokenProvider 50 轮连续失败——永不 panic、
    永不执行任何步骤、model_error_count 如实累计；非环回明文 http 端点
    拒绝策略 20 轮重复验证；
  - **G02 Planner Determinism**：相同输入 + 相同 LLM 文本，10 轮
    pipeline 输出逐位一致；
  - **G03 Agent State**：预算耗尽到达终态且状态机冻结（再次迁移报错）、
    可观测轨迹覆盖 SessionCreated→ApprovalRequested→RunCancelled 全弧；
    重规划恰一次（§25 v1）且 Replanned/StepFailed/RunFailed 事件齐全；
  - **G04 Approval Security**：伪造 id / 过期决策 / 过期后重放 /
    cancel 后应答 / 决策 id 与请求不匹配——全部不授权任何执行；
  - **G05 Prompt Injection**：含 ``` fence 逃逸与 override 语句的敌意
    catalog 无法翻转请求类型（A03 确定性 intent 覆盖 LLM 输出）；
    sanitize 对抗语料（fence/控制字符/超长）全过；override 检测器命中；
    E05 远程门默认关闭。
- **G01 抓出并修复真实缺陷**：`Planning → Failed` 不在状态机白名单，
  规划期失败（pipeline 错误/澄清拒绝）被 `let _` 静默吞掉、状态卡死在
  Planning——已加入白名单（附注释溯源）。
- **G04 抓出并修复真实缺陷**：`run_agent` 此前不校验 sink 返回的
  `decision.request_id`，伪造 id 的 Approve 会原样生效——现已强制
  decision id 与 pending 请求一致，不一致 = 取消（零步执行）。

## Gate 结果

- `cargo test --workspace`：**863 passed / 0 failed**（853 → 863，+10）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.7 状态

G 线核心矩阵完成；剩余为 H 线 Release 项（H01 CI AI tests——本套件即
CI 载体、H02 Security Gate、H03 quality corpus、H04 release gate、
H05 文档冻结、H06 完成宣告——多数依赖 MSIX 签名等外部证书与发布流程）。
非证书类 H 项可在后续批次推进。
