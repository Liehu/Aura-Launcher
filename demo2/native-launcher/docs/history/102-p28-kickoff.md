# 102 — P2.8 评审 + 开工裁决（Ecosystem & Distribution 1.0）

> 日期：2026-09-09。范围：P2.8 三份文档评审（`demo2/files2/P2.8 —*.md`，
> 共 3160 行）与阶段执行计划。输入基线：history/101（705 tests）。

## 评审结论

1. **设计规范**（1765 行）：插件生态完整生命周期——Identity → Repository
   发现 → Package → Signature → Trust → Compatibility → Dependency
   Resolution → InstallPlan → 审批 → 原子 Activation → Health →
   Upgrade/Rollback/Uninstall → Audit。非目标清晰：不做下载网站、不依赖
   中心服务器、默认远程 AI 排除。
2. **依赖裁决（必须声明）**：
   - **硬依赖已满足的**：P2.4-D CLI（init/validate/package/install staged
     原子替换——P28-E 事务化 install 的雏形已在）、P2.4-C trust/capability
     持久化（P28 Trust Model 的地基）、plugins.db backup/restore、
     catalog FTS（Marketplace 搜索地基）。
   - **软依赖**：签名验证（§9）复用 GA-6 的证书外部依赖裁决——P2.8 先实现
     **checksum/完整性验证**（§10 Package Integrity，纯代码），签名留接口。
   - **顺序冲突**：规格前置列表提到 AI Agent Runtime（P2.7）/Workflow DAG
     （P2.6）为"已知非基线"，P2.8 不硬依赖它们——**可与 P2.6/P2.7 并行**，
     不存在 P2.7 那样的硬阻塞。
3. **体量**：约 69 任务、11 个 Agent 组（000-004 / A-I / J）——是 P2.4 的
   ~1.5 倍。必须分 ≥6 个批次。

## 批次计划（按依赖序）

```text
Batch 1  P28-000~004 Foundation：PluginIdentity/Package Manifest v2/
         Integrity(checksum)/SQLite 核心实体/状态机注册     ← 下一批
Batch 2  P28-B + D：Package 工具 + Dependency Resolver
Batch 3  P28-E：事务化 Install（InstallPlan/staging/atomic
         activation/rollback）
Batch 4  P28-C + F：Signature 接口 + Trust 评估 + 生命周期
         （upgrade/health/broken/disable/uninstall）
Batch 5  P28-A + G：Repository/Index/Marketplace 搜索 + 管理 UI
Batch 6  P28-H + I + J：集成/Audit/QA/Release Gate 收口
```

## 红线（从规格提取，全批次生效）

- Repository/Package 元数据是 DATA；**任何安装 Effect 必须走既有
  Resolver→Engine 与 staged install**（P2.4-D 已建立的形态）。
- Trust 默认 fail-closed（未知包 ≠ 可安装）；capability escalation 拒绝。
- Dependency 解析确定性、失败可解释、不引入中心服务器依赖。
- IndexGeneration/PluginGeneration 语义不变。

## 状态

- 本文件为 P2.8 开工记录；Batch 1（Foundation）自下一会话开始执行，
  每批完成追加 history/103+ 并更新本文件勾选。
