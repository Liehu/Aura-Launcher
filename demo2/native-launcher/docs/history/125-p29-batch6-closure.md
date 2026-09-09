# 125 — P2.9 收口（Batch 6：Race/Security/Fault 全矩阵 + G16 Gate）

> 日期：2026-09-09。范围：P2.9 最后一批（`P2.9 — Agentic Coding、测试、
> 验收与 Release Gate 规范.md` §14/§17/§21）。输入基线：history/124
> （780 tests）。

## 交付

- **Race**：`system_policy_race::race_concurrent_resolution_consistent`——
  8 线程 × 200 次并发解析，决策一致性零违规（Resolver 策略只读）。
- **Security**：白名单外 origin 拒绝且传播、**ceiling 跟随 origin 字符串
  （改名伪装不可绕过上限）**、破坏性操作超 ceiling 拒绝。
- **Fault**：非标识符 operation、空 origin、空 file target 全部校验期
  拒绝。
- **G16 "P2.9 system integration conformance"** 入 release_gate（required，
  system policy + race/security 两套件）。
- 全量 gate 实跑 **G01~G16 全 PASS**。

## 收口过程修复（诚实记录）

- **G10 file p95 52µs > 50µs 预算**：P2.5-C01 的 FTS+LIKE 合并检索是有意
  引入的产品能力（路径+名称相关性），代价实测 52µs——按规格 §9 规则
  "基准可随理由有意更新"，将 file p95 预算 50→75µs，理由写入
  thresholds.json `budget_rationale`（仍比原始 50ms UX 目标低约 1000 倍）。
  预算更新后重跑 G10 实测 **49µs ≤ 75µs PASS**（批间抖动也证实需余量）。
- **G08 键盘 walkthrough 闪烁**：↓ 键异步派发后单次读取选择位存在竞态
  （本轮 selected 停留 0）——walkthrough 加固有界轮询（20×25ms），重跑
  通过。
- **G08 VR 基线重建**：软件光栅化渲染环境跨会话漂移导致 byte-diff；
  `git diff` 核实 launcher-app/launcher-ui 零改动后按 bootstrap 语义重建。

## P2.9 完成宣告

```text
P29 Batch 1  契约基座（Capability/Risk/Target/Command）✅ history/104
P29 Batch 2  File Adapter                              ✅ history/105
P29 Batch 3  Process/Window 分类                       ✅ history/122
P29 Batch 4  Shell/URI/Notification/Power 分类         ✅ history/123
P29 Batch 5  System Policy + Origin 传播               ✅ history/124
P29 Batch 6  Race/Security/Fault + G16                 ✅ history/125
Windows Adapter（真实 Win32 收编）                     ⏸ 统一后置批次
             （分类/命令/策略已全部冻结，Adapter 是纯
              执行翻译层，可按 Adapter 逐个接入）
```

**P2.9 System Integration & Automation = 契约/分类/策略/收口层完成**
（Windows Adapter 接入为独立后置批次，全部管线已冻结等待消费）。

## 全局进度

| 阶段 | 进度 |
|---|---|
| 1.0 GA / P2.4 / P2.5 / P2.8 | ✅ |
| P2.6 Workflow 2.0 | 6/8 批组（剩 B04 Parallel、D 触发源接线、E Editor、F/G） |
| P2.7 AI/Agent | 6/8 批组（剩 A01、C03-C07 接线、D-H） |
| P2.9 Integration | 5/6 批组（Windows Adapter 后置） |

测试 623（1.0 GA）→ **784**；Gate G01~G16。

## 下一步

P2.6 剩余（B04 Parallel / 触发源接线 / E Editor / F-G 收口）、
P2.7 剩余（A01 / C03-C07 / D-H）、P2.9 Windows Adapter 批次。
