# 115 — P2.8 Batch 5：Repository Index + Marketplace 搜索（§11-§13/§29）

> 日期：2026-09-09。范围：P2.8 第五批（`P2.8 — Ecosystem & Distribution
> 1.0 技术设计规范.md` §11-§13/§29）。输入基线：history/114（745 tests）。

## 交付

- `apps/launcher-plugin-cli/src/repository.rs`（新）：
  - **§11/§12 Repository**：本地 JSON 索引文件（tmp+rename 原子写），
    `publish()` 按 id 替换并保持排序、**发布时计算 payload SHA-256**
    （§10 完整性，客户端下载后复核）。无中心服务器依赖——repository
    source 是用户配置的任意路径/URL，v1 提供本地文件源。
  - **§29 Marketplace 搜索**：确定性相关性（id 精确 > id 前缀 > summary
    包含），每条结果带 **trust badge**（signature + source 即时计算），
    受 limit 约束；**空查询永远返回空**（不倾倒全目录）。搜索永不安装。
- 测试 3 条：publish 替换+排序、搜索相关性+trust badge+空查询+limit、
  save/load roundtrip（校验和保留）。

## Gate 结果

- `cargo test --workspace`：**748 passed / 0 failed**（745 → 748，+3）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## P2.8 剩余

Batch 6（集成：plan→transactional→repository 串联 / Audit / QA /
Gate 收口）。管理 UI（§39-§40）归 P2.9/E 线后置。
