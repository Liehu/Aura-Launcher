# 80 — P2.1-E 第一批实施记录：Icon 管线（E1-E4）+ 性能基线现状

日期：2026-09-07。基线：552 tests / 零警告 / Release Gate 十项 PASS。
按 79 号文档（P2.1-E 冻结契约）实施。范围决策：**先做 E1-E4 后端管线 + 基线数据，E5 UI 接线单独成批**。

---

## 一、交付内容

### 1. E1 Icon 契约（`launcher-domain/src/icon.rs`）

- `IconKey { application, variant, source_revision, extractor_version }`：语义身份（非显示名，INV-ICON-006）+ 来源修订 + 抽取算法版本——版本升级天然失效旧磁盘缓存，无需启动清扫（§18/§49/§50）。
- `IconSource`（PackageResource/Executable/Shortcut/Shell/None）、`IconVariant`。domain 不含 bitmap/HICON/文件系统（§11/§69 边界）。

### 2. E2-E4 Icon 管线（`launcher-providers/src/icons.rs`）

- **L3 抽取**：SHGetFileInfoW → HICON → GetIconInfo → GetDIBits → RGBA（BGRA 转换 + 不透明兜底）；只取 base icon（§30 无 Shell overlay）；GDI/HICON 句柄零泄漏出本文件（INV-ICON-007）；失败返回 Err 不 panic。
- **L1 内存缓存**：LRU + **硬字节预算**（§9/§45——按 bytes 而非条目数），淘汰至预算的 80% 防抖动；超预算单图不缓存。
- **L2 磁盘缓存**：PNG（§19 透明 alpha/可调试）、**原子写**（temp+flush+sync+rename，§20）、**损坏自愈**（读取失败即删除条目并重抽取，INV-ICON-005）、cache key 绑定完整 IconKey（§17）。
- **IconService**：L1→L2→L3 门面；抽取在服务锁内串行化——"同一 key 至多一次活跃抽取"（INV-ICON-003）在 v0.1 尺度下构造性成立。
- 测试 4 项：LRU 预算淘汰（含"最近使用存活"）、超预算拒缓存、磁盘往返 + 损坏自愈、**真实 notepad.exe 抽取 + 二次命中缓存抽取计数不变**（ICON-011/012 活体）。

### 3. 性能基线现状（E6-E8 的已有基础）

核实发现 `launcher-bench` **已具备** 76 号文档要求的绝大部分：cold_start_us、search_app/file_us p50/p95/p99、idle/search 内存、targets 内置判定（record 模式写 benchmarks/baseline.json）。本次记录当前基线：**app 查询 p50≈4.3ms / p95≈8.1ms；file 查询 p50≈2.2ms / p95≈7.0ms（debug 构建，targets 内通过）**；外加 perf_baseline.py 的启动/RSS 上界。Budget 按文档 §42 原则由 baseline×tolerance 推导，暂不拍绝对数。

## 二、有意后置

- **E5 UI 接线**（SearchResult 携带 icon_key、异步到位、UiIconState、Loading/Fallback 占位）：会改动 ResultRow 渲染 → 作废 15 张 VR 基线，必须与"基线重生成 + （可选）UI-CONTRACT v0.2"同批进行。管线就绪后这是纯 UI 批次。
- **真并发 in-flight map**（§22：多消费者共享抽取任务）：v0.1 串行化已满足正确性；并发 fan-out 等 Provider v2 线程模型一起做。
- PackageResource 图标（manifest logo/MRT 解析，§14/§15）、DPI 矩阵（§29）、Bench datasets/回归门禁进 CI（§62-65/§71 E7-E8）：后置。
- 79 §46 的冻结清单（Query Cache、Favorites、ML、Cloud 等）继续冻结。

## 三、验证

```text
cargo test --workspace   552 passed / 0 failed（+5：icon 契约 1、LRU 2、磁盘 1、活体抽取 1）
cargo build --workspace  zero warnings
check_topology.py        PASS（icons 位于 launcher-providers，HICON 不出 Windows 层）
release_gate.py          十 Gate 全 PASS（VR 15/15 byte-identical —— 未触碰 UI）
launcher-bench           app p95≈8ms / file p95≈7ms（debug，内置 targets 通过）
```

## 四、下一步

1. **E5 Icon UI 接线批**（含 VR 基线重生成 + DPI 抽查 §29）——管线已就绪，是让用户"看见"图标的最短路径；
2. 或 Batch 3 主体（Provider Contract v2）。
按 79 §71-§73：两条都完成后 P2.1 基础设施阶段关闭，转向 Favorites/Pinning、插件生态管理与 Installer/Upgrade 等产品能力。
