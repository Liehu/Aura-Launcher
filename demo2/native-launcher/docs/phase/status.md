# Phase Status（P210-011 Documentation Reconciliation）

> 状态词汇（P2.10 §28，唯一权威）：`DESIGNED → IMPLEMENTED → INTEGRATED
> → ACCEPTED`，补充 `DEFERRED`（主动后置）与 `BLOCKED-EXTERNAL`（等外部
> 条件）。本文件是阶段状态的唯一真相；README/HANDBOOK 只做索引；
> `docs/history/` 是不可回写的历史证据。

## 1.0 GA
- Core pipeline / UI / 插件执行链：**ACCEPTED**（G01–G17 gate，97/96 号审计）

## P2.4 Foundation
- Catalog 2.0 / Index 2.0 / CLI：**ACCEPTED**（91–94 号）

## P2.5 Search Intelligence
- Contract v2 / Coordinator / FTS5 / Ranking / Pinyin 全表：**ACCEPTED**（95–99/145/156 号）

## P2.6 Workflow 2.0
- Graph / Durable / Approval / Trigger / Editor Surface：**ACCEPTED**（100–110/126–132/150 号）
- 自由画布编辑器（free-form canvas）：**DEFERRED**（列表式 surface 满足声明式红线；P3 UI 批次）

## P2.7 AI / Agent Productization
- A–G 全线（Contract/理解/规划/运行时/审批/记忆隐私/UX/QA）：**ACCEPTED**（101–121/133–136/138–142/147–148/151–155 号）
- 交互式 clarify 循环（§18）：**IMPLEMENTED 的 fail-closed 版本**（澄清即
  Failed + 提示）；完整多轮交互：**DEFERRED**
- H 线 CI/质量语料/文档：**ACCEPTED**（155 号）；Security/Release Gate 的
  签名部分：**BLOCKED-EXTERNAL**（等 MSIX 证书）

## P2.8 Plugin Lifecycle & Local Ecosystem（原 Ecosystem & Distribution）
- Identity/Integrity/Resolver/事务化 Install/Trust/Repository：**ACCEPTED**（111–116 号）
- Marketplace 服务端 / Version Channel / Management UI：**DEFERRED**（P3.x Distribution）
- Signature 校验链路：**BLOCKED-EXTERNAL**（接口已冻结，等证书）
- 阶段更名建议（103 号评审）：采用「Plugin Lifecycle & Local Ecosystem」，
  Distribution 独立为 P3.x

## P2.9 System Integration & Automation
- Capability/Policy/Resolver（契约与策略层）：**ACCEPTED**（104/105/122–125 号）
- Windows Adapter（真 Win32）：**ACCEPTED**（149 号）
- 同进程 HWND 生命周期身份（WindowGeneration）：**DEFERRED**（跨进程 reuse
  已覆盖，158 号 Batch 2）

## P2.10 Cross-System Consistency & Security Hardening
- H1 Effect Authority（token 门/铸造唯一化/绑定/move-only）：**ACCEPTED**（157/158 号）
- H2 Dynamic Target（PID FILETIME 身份/HWND liveness+属主）：**ACCEPTED**（157/158 号）
- H3 Replan Safety（Succeeded 不可变/Unknown 不盲重）：**ACCEPTED**（157/159 号）
- H4 Timeout/Unknown/Retry/Recovery Protocol + Probe：**ACCEPTED**（158/159 号）
- H5 State/Generation：**ACCEPTED**（STATE-GENERATION.md，159 号）
- H6 Documentation：**IMPLEMENTED → INTEGRATED**（contracts/ 落地、phase/ 本
  文、history 不可回写已成立；README/HANDBOOK 全面索引化待最后扫尾）
- Cross-Phase E2E（§33 三条黄金路径）：**ACCEPTED**（159 号）
- 跨阶段状态矛盾 / 契约-状态矛盾：无已知实例（E2E + 契约映射佐证）

## P3.0 Launcher Experience 1.0（进行中）
- B0 搜索手感（模糊匹配/防抖/即时答案/设置面）：**ACCEPTED**（161 号）
- B1 视觉与信息密度（双主题/微动效/元数据/详情面板/自绘控件/VR 双基线）：**ACCEPTED**（162 号）
- B2 差异化（入口前移/settings:ai/富结果草案）：**IMPLEMENTED → INTEGRATED 待验收**（163 号待办）

## P3 前置条件（Hard Gate）
P2.10 出口条件全部满足（见 159 号 Hard Gate 自查表）；P3 可启动。
BLOCKED-EXTERNAL 项不阻塞 P3（均属发布/分发域，非运行时安全域）。
