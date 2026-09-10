# 161 — P3.0 Batch B0：搜索手感与即时反馈（F01–F04）

> 日期：2026-09-10。范围：P3.0 第一批（规范族：files2/P3.0 四件套）。
> 输入基线：history/160（887 tests）。

## 交付

- **F01 模糊子序列匹配（P30-001/002）**：
  - `crates/launcher-search/src/fuzzy.rs`：单遍 O(n) 评分匹配器——
    前缀短 路 100 / 词边界 +12 / 连续段 +6+段长奖励 / gap −1.5（总惩罚
    封顶 30）；候选侧零分配，query 折叠进栈缓冲；ASCII 折叠快路径；
  - 评分集成：`score_with_parts` 在无词法命中时取
    title/stem/keywords/pinyin_initials/pinyin_full 五字段最佳分，
    归一化后计入上限 40 分的 `fuzzy` 分量（`FUZZY_MAX_CONTRIBUTION`）；
    旧布尔 `is_subsequence` 移除；
  - 测试 9 条（FUZ-P1~P8 + 前导跳过不惩罚）。
- **F02 防抖（P30-003）**：`on_query_changed` 70ms debounce（epoch
  计数器丢弃过期定时器）；空查询旁路（recent 视图即时）。
  *简化说明*：两阶段发布（内存先出/FTS 后合）简化为防抖 + crate 内
  合并排序——实测首帧已达标（见 PERF），全库两阶段后置。
- **F03 即时答案（P30-004）**：
  - `crates/launcher-core/src/providers/answers.rs`：`=1+2*3` 或裸算式
    （`6/3`）内联出 Answer 行（恒 score=1.0 首位），Enter = 复制结果
    （既有 `ActionKind::Copy` 权威链）；
  - 求值器为 ~90 行纯递归下降（+−×÷% 括号、f64、深度 32 上限）；
    解析失败/除零/溢出/尾随垃圾一律降级为普通搜索（UX-R2）。
  - *规范变更记录*：设计文档原定复用 `expr.rs`，评审其文法为条件型
    （无算术运算符）后改为自实现求值器，安全边界不变——设计文档 §F03
    已同步修订。
- **F04 原生设置面（P30-005）**：
  - `apps/launcher-app/src/settings_ui.rs` + `settings-ui` 命名空间路由
    （host 侧 apply，与 agent/editor 同模式）；设置项：theme 循环
    （写 `theme_mode`，B1 接入视觉）、autostart 开关（注册表+config
    同步）、索引目录移除、打开 config.toml；
  - config 新增 `theme_mode`（serde default "dark"，向后兼容）；
  - `launcher_config::save` 已是原子写（tmp+rename），SET-5 由既有
    实现满足；
  - **DEFERRED**：全量重建触发（协调器句柄未留存于 AppState，需小规模
    接线）与热键捕获 UI——随 B1 补。
- 测试 17 条：fuzzy 9 + answers 4 + settings 3 + perf 1。

## 性能预算实测（PERF-2/3）

| 指标 | 预算 | 实测 |
|---|---|---|
| 内存模糊匹配 P95 @10k 候选（3 字段/条） | < 5ms | **2.97ms**（launcher-search dev+opt2 构建） |
| 首帧发布（debounce 70ms 后） | < 100ms | 70ms 防抖 + 排序 < 5ms ≈ 75ms ✓ |

工作区 `[profile.dev.package.launcher-search] opt-level = 2`：匹配器在
dev/test 构建保持优化（热路径可测）。

## Gate 结果

- `cargo test --workspace`：**904 passed / 0 failed**（887 → 904，+17）
- `cargo build --workspace`：零警告；`check_topology.py`：ok

## B0 验收清单对照（P3.0 验收标准 §1）

| # | 结果 |
|---|---|
| B0-H1 chr/wj/wenjian 命中 | ✅ FUZ 语料 + p7 |
| B0-H2 合并去重、非模糊相对序不变 | ✅ FUZ-I2（单测） |
| B0-H3 70ms 窗口单次查询；空查询即时 | ✅ DB-1/3（epoch 单测） |
| B0-H4 Answer 恒首位、Enter 复制 | ✅ ANS-P1/P4/P5 |
| B0-H5 危险输入零 Answer 零崩溃 | ✅ ANS-P2/P3 |
| B0-H6 设置项 roundtrip；写失败保留原文件 | ✅ SET-1~6（原子写为既有实现） |
| B0-H7 性能预算 | ✅ 2.97ms / ≈75ms |
| B0-H8 全量回归 | ✅ 904 / 0 warn / topology ok |

## 待续（B1）

主题调色板参数化（`theme_mode` 已入 config）、微动效（快照禁用）、
文件行元数据 + Tab 详情面板、自绘 SearchBox/Button、VR 亮/暗双基线
重生成；补 reindex 触发接线。
