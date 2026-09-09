# 127 — P2.6 Batch 8：触发源消费接线（D03-D06 消费端）

> 日期：2026-09-09。范围：P2.6 第八批（`P2.6 开发设计规范` §22）。
> 输入基线：history/126（784 tests）。

## 交付

- `launcher-app`：**trigger-service 后台线程**（5s 周期）消费
  `TriggerQueue`（history/110 的持久 FIFO）——dequeue 事件 → 在
  `<data>/workflows/*.json` 中按 workflow_id 匹配定义 → 经既有
  `workflow_service::start_workflow` 正常管线启动（校验/日志/UI 全套）；
  无匹配定义仅记日志不报错。
- 至此 D 线四类触发源（hotkey/plugin/schedule/ai-mcp）有了**统一消费端**：
  各源只需向 TriggerQueue enqueue（§22 队列契约不变）。
- 消费线程随进程退出自然结束（daemon thread）。

## Gate 结果

- `cargo test --workspace`：**784 passed / 0 failed**
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.6 剩余

E 线 Visual Editor（7 任务，最大 UI 块）、F/G QA+Release 收口。
