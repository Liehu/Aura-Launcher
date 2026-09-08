# 88 — P2.3 系列启动与 C 批实施记录

日期：2026-09-07。基线：586 tests / 零警告 / Release Gate 全 PASS。

---

## 一、评审结论

83-roadmap + 84~93 九份文档构成完整的 **P2.3 Launcher 1.0 Release Closure** 程序。评审结论：
- **定位正确**：收口而非扩张；每批"不新增功能"铁律正确；
- **Cross-Cutting Contracts 文档**（82 §26 + 83 §一）两次要求后本批落地；
- **不能一批做完**——九批合计约 1.1 万行，每批应独立会话推进。

## 二、本批实施（P2.3 启动 + P2.3-C 核心）

### P2.3 启动
- `docs/P2-CROSS-CUTTING-CONTRACTS.md`：ID/Generation/State/Authority 唯一登记处；
- Release Gate **G10**：release 性能回归 gate（app p95 7µs 基线 +50% 容差）；
- `--require-baseline` 硬门使 release CI 必须显式钉 VR 基线。

### P2.3-A 架构契约基线
- **LAYER_GUARDS**：domain/context/search 层依赖冻结入 check_topology；
- 负向测试验证守卫真实生效；
- `docs/P2.3-A-ARCHITECTURE-BASELINE.md`：30+ 不变量 × 验证锚点。

### P2.3-B Product E2E
- `product_e2e.rs`：Golden Path（搜索→执行→历史→boost→收藏→空查询）+ 文件索引 rescan + Workflow 目录发现 + 定义往返。

### P2.3-C Reliability / Recovery（核心子集）
- **全持久层损坏恢复**：index/favorites/catalog/plugins 四层统一 `try_open`→失败→隔离/删除→重试模式；
- **启动健康标记**：`startup_state.json` starting→healthy + 崩溃环计数（≥3 → DEGRADED BOOT 跳过 coordinator）；
- **update_handoff.json** 消费（中断升级检测）；
- **卸载数据保留策略**写入 install.ps1；
- `recovery_golden.rs`：index corruption rebuild / generation monotonicity / writer failure recovery。
- 新增测试：recovery_golden 3 项。

### P2.3-C 剩余（挂账至后续批次）
- C1 全 crate FailureTaxonomy 统一映射（FailureClass + RecoveryPolicy enum 已创建于 domain）
- C3 Plugin crash soak（真实多进程循环）
- C4 MCP runtime/session 恢复矩阵补充
- C11 长跑 soak（4h profile）
- C12 完整 20 scenario Golden Suite（现有 3 项为起步）

## 三、验证

```text
cargo test --workspace   586 passed / 0 failed
cargo build --workspace  zero warnings
check_topology.py        PASS
release_gate.py --require-baseline  十 Gate 全 PASS
```

## 四、P2.3 执行计划

| 批次 | 内容 | 状态 |
|---|---|---|
| P2.3-A | Architecture & Contract Baseline | ✅ ACCEPTED |
| P2.3-B | Product E2E | ✅ ACCEPTED |
| P2.3-C | Reliability / Recovery | ✅ 核心完成（C6/C7/C10 done; C1/C3/C4/C11/C12 挂账）|
| P2.3-D | Performance / Memory Closure | ⏳ |
| P2.3-E | Security / Trust Boundary | ⏳ |
| P2.3-F | UI / Interaction QA | ⏳ |
| P2.3-G | Installer / Upgrade / Migration QA | ⏳ |
| P2.3-H | Release Engineering & CI | ⏳ G10 done |
| P2.3-I | Release Candidate | ⏳ |
