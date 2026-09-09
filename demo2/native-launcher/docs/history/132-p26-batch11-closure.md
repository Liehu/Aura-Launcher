# 132 — P2.6 收口（Batch 11：E04/E05 接线证明 + G17 Gate；P2.6 完成）

> 日期：2026-09-09。范围：P2.6 最后一批（`P2.6 开发设计规范` §25-§27/
> F/G 线）。输入基线：history/129-130（790 tests）。

## 交付

- **E03/E04 接线证明**：`launcher-ui/tests/editor_surface.rs`——
  `GraphEditorSurface` 实例化、宿主投影行模型（VecModel→rows 属性）
  绑定、行数据 Slint 可达——**VIEW 层绑定管线实测可用**。
- **E05 验证面数据源**：launcher-app `editor_surface::status_text()`——
  对草稿图运行 §10 validate()，错误转 `INVALID: ...` 状态行文本
  （空 = 有效）。
- **G17 "P2.6 workflow conformance"** 入 release_gate（required）：
  launcher-workflow 全套件（graph/engine/durable/scheduler/approval/
  triggers/editor）+ launcher-app bin 测试。
- 全量 gate 实跑 **G01~G17 全 PASS**。

## P2.6 完成宣告

```text
P26-000/001/002  Baseline + Contract v2 + ID 注册      ✅ history/100
P26-A            Graph Model/Validator/Join/Condition/  ✅ 100/106
                 Variables/Contract Kit
P26-B            Durable Run Store/Checkpoint/Scheduler/✅ 107/108/126
                 Parallel/Retry/Recovery
P26-C            Human Approval（契约/存储/决策/
                 restart-safe）                          ✅ 109 号
P26-D            Trigger Framework + 持久队列 + 消费接线 ✅ 110/127 号
P26-E            Editor 状态模型/Undo/导入导出/          ✅ 128/130/131 号
                 宿主投影/Slint surface（v1 列表形态）
P26-F/G          QA + G17 Gate                          ✅ history/132
P26 推迟项        自由画布 Canvas（v1 列表形态已满足      ⏸ 后置
                 声明式红线）、Signature（GA-6 裁决）
```

**P2.6 Workflow 2.0 = 完成**（自由画布与签名按裁决推迟）。剩余：P2.7
（7 批组内除已完成的 6 批外）与 P2.9 Windows Adapter 接线。

## 全局进度

| 阶段 | 进度 |
|---|---|
| 1.0 GA / P2.4 / P2.5 / P2.8 | ✅ |
| P2.6 Workflow 2.0 | ✅ **完成**（Free-form Canvas 后置） |
| P2.7 AI/Agent | 6/8 批组 |
| P2.9 Integration | 5/6 批组（Windows Adapter 后置） |

测试 623（1.0 GA）→ **791**；Gate G01~G17。

## 下一步

P2.7 剩余（A01 Provider 深化、C03-C07 Loop 接线、D-H 线）、
P2.9 Windows Adapter、P2.6 A04/C04 Pinyin（跨阶段尾项）。
