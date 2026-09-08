# 103 — P2.9 评审 + 开工计划（System Integration & Automation 1.0）

> 日期：2026-09-09。范围：P2.9 两份文档评审（`demo2/files2/P2.9 —*.md`，
> 共 2468 行）。输入基线：history/102（705 tests，P2.8 计划冻结）。

## 评审结论

1. **设计规范**（1725 行，40+ 节）：统一系统集成层——Process/Window/
   Clipboard/File/Shell/URI/Notification/Power/Display/Hotkey/Settings 等
   全部系统能力经 `System Capability Resolver → Policy → launcher-action →
   Effect`。五大"≠"边界（Capability≠Grant、Resolver≠Authority、
   Command≠Effect、Plugin Request≠Authorization、AI Proposal≠Execution）
   与全项目冻结边界完全一致——**这是对既有架构的收敛，不是新执行通道**。
2. **与现有资产高度重合**：foreground/Explorer context（launcher-context）、
   global hotkey（launcher-hotkey）、File effects（launcher-action）、
   clipboard effect、notification 已分散存在——P2.9 的本质是把这些收拢到
   **Adapter 架构 + Capability 分层 + 风险分类 + Mock Provider**（可测性）
   统一模型下，属重构收敛而非新功能。
3. **Agentic 规范**：14 步任务流（含 Threat Model/Race/Security Test）是
   全阶段最严格的——因为系统集成的每一步都触碰真实 OS。§3 "Native API
   禁止扩散"（Win32 只允许存在于 Windows Adapter 内）与既有 LAYER_GUARDS
   一致。
4. **依赖**：不硬依赖 P2.6/P2.7/P2.8（引用它们但不阻塞）；可与三者并行。
   但它是**大范围重构**——必须逐 Adapter 小批推进，每个 Adapter 独立
   保持旧行为（兼容模式）。

## 批次计划（按风险序，从低风险 Adapter 开始）

```text
Batch 1  契约基座：SystemCapability/RiskClass/SystemTarget/
         SystemCommand DTO + Mock Provider + Resolver 骨架   ← 下一批
Batch 2  File/Clipboard Adapter（已有 Effect 收编）
Batch 3  Window/Process/Foreground Adapter（Win32 收编）
Batch 4  Shell/URI/Notification/Hotkey/Power/Settings Adapter
Batch 5  System Policy + Confirmation + Actor/Origin 传播
Batch 6  Race/Security/Performance/Soak/Fault 全矩阵 + Gate 收口
```

## 红线（全批次生效）

- Capability ≠ Grant；System Resolver ≠ Effect Authority；一切 Effect 仍仅
  经 `validate → Policy → Approval → launcher-action → Effect`。
- Native API 只存在于 Windows Adapter 模块（LAYER_GUARDS 延伸）。
- 破坏性操作（§14）必须走既有 Confirmation 子状态；File Transaction
  （§15）先回收口窗口/临时目录语义。

## 状态

本文件为 P2.9 开工记录；Batch 1（契约基座）自下一会话开始，每批完成追加
history/104+。**多阶段排队提醒**：P2.6（4 批待做）、P2.7（7 批待做）、
P2.8（6 批待做）、P2.9（6 批待做）——四个阶段共约 20 个批次，建议按
P2.9 Batch 1（契约基座小）→ P2.6 B 线（解锁 P2.7）→ 交错推进的顺序执行。
