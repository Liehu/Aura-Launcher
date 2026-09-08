# 63 — P2 评审与 Launcher 1.0 实施记录（评审对象：60-mvp10-0.5 / 61-mvp10-0.6）

日期：2026-09-06。基线：MVP4.3 RC（477 tests / 零警告 / topology 17+8 / Release Gate PASS）。

---

## 一、对 61（P1-E Visual Polish）的评审结论

61 的方向正确：**"只改呈现，不改权限"**（不碰 Resolver/Engine/Workflow semantics），并给出了完整的三层 UI 模型（Presentation / UI State / Core View Model）、UiSnapshot/UiCommand/UiEvent 边界、20 张 VR 基线规划、INV-UI-001~010。

采纳但部分后置的理由：

1. **UI-CONTRACT v0.1 仍是 FROZEN canonical**（手册 §3/§10 红线 1）。61 的 P1-E.1 要求冻结 UI-CONTRACT-v0.2 + ADR-0018，这是一个独立评审+ADR 流程，不应与功能实施混在同一批改动里。
2. **VR 基线 byte-for-byte 是当前硬闸门**。任何 Main/Action 面板视觉变更都会作废 VR-001~010 基线，必须与 v0.2 基线重生成同批进行。
3. 60（P2 路线图）明确：对"可用的 1.0"而言，**搜索/Windows 集成/历史排名的价值高于视觉第二轮打磨**。因此本批按 60 的优先级表实施 S 档必做 + A 档推荐，P1-E 的 UI 改造保留为下一阶段（需要先冻结 UI-CONTRACT-v0.2）。

61 中**已经**随本批受益（无需改 UI）：§6 空/错误状态区分、§11 disabled reason 文案、§47/48 状态栏基础在 MVP4.x 已存在；§29 键盘系统已收敛。

## 二、对 60（P2 路线图）的评审结论与取舍

60 的核心判断全部成立：P1 完成后不应继续堆大功能；P2 = Launcher 1.0 产品化；AI/MCP/Workflow/Agent 全部可后置。按其 §19 必做表逐项对照现状：

| 60 §19 必做项 | 实施前状态 | 本批动作 |
|---|---|---|
| 搜索引擎 / Ranking / Fuzzy | 有基础评分+子序列 fuzzy；**无使用信号** | ✅ rank_with_boost：频率(≤30) + 新近(≤12)，只作用于已有词法匹配，确定性可测 |
| History / Recent | history 表存在但 **record_use 无任何生产调用方**（死接线） | ✅ 执行成功路径（system + plugin/mcp effect）全部落库；带 title |
| App/File Index | Start Menu .lnk + Uninstall 注册表 + SQLite 文件索引已有 | 保持（索引 2.0 另批） |
| Action 扩展 | 仅 Open/Reveal | ✅ 新增 Run as administrator（.exe）、Copy path（app+file），走同一 Resolver→Engine 路径 |
| Windows Integration | 弹窗永远居主显示器；无单实例 | ✅ 光标所在显示器放置（GetCursorPos→MonitorFromPoint→rcWork）；CreateMutexW 单实例 |
| Plugin SDK / Lifecycle | 已有（manifest 目录 + Python SDK） | 无需动作（60 §五：本地目录即够，市场后置） |
| Settings | 缺展示上限等 | ✅ `result_limit`（默认 12，clamp 到 MAX_RESULTS） |
| Performance / Crash | Release Gate G8（soak/资源）已绿 | 保持 |
| Installer / Upgrade | 未做 | ⏳ 后置（单实例/自启动/配置持久化已就位；打包脚本另批） |
| MCP / AI / Workflow / Agent | 已 RC | 冻结不动（60：推荐/可选档） |

**架构红线核查**：所有新 Effect（runas/copy path）都经 ActionEngine（INV-004）；UI 未获得任何新权限；新 ActionKind 补齐了 proposal `action_type_of`（system.run_as_admin）；capability 单调性不受影响。

## 三、基线修复记录（重要）

接手时 workspace 无法编译：P1-C（Workflow v0.2）重构进行到一半——domain 已加 `condition/output/on_success/on_failure/on_condition_false/entry_step/inputs/variables/skip_reason` 字段并新增 `v2.rs`（变量/模板/条件/图校验），但大量调用点与 v2.rs 自身未跟上。本批修复了约 30 处过期初始化器与 v2.rs 的类型/借用/可达性错误（其中 `validate_graph` 的线性 continuation 未进邻接表导致 "unreachable step" 误报，为真实逻辑缺陷）。

## 四、本批 P2 改动清单（全部在既有架构内）

- `launcher-search`：`rank_with_boost(cmds, q, limit, boost)`；`rank` 委托之。Boost 只加在 score>0 的命令上（不制造无词法匹配的噪音结果）。
- `launcher-indexer`：history 表加 `title` 列（幂等 ALTER）；`record_use_titled`；`usage_map()` 聚合（count, last_used）。
- `launcher-core`：`record_use_with_title`；`usage_snapshot`/`usage_boost`（freq 3×min(n,10) + 24h 内 12 / 7 天内 6，最大 ≈42 < 精确匹配 100，只重排不越级）；`search()` 接入 usage boost；`file_command` 增加 Copy path。
- `launcher-providers`（app-registry）：每个应用命令 = Open（primary）+ Run as administrator（仅 .exe，id=runas）+ Copy path（id=copypath）。
- `launcher-domain`：`ActionKind::RunAsAdmin`；`launcher-action`：validate + ShellExecuteW("runas") 执行；workflow `action_type_of` 映射。
- `launcher-config`：`result_limit`（默认 12）。
- `apps/launcher-app`：单实例互斥体；弹窗在光标所在显示器居中（工作区矩形，失败回退主屏）；查询用 `result_limit`；system/plugin/mcp 执行成功 → `record_use_with_title`（"越用越顺"闭环接通）。
- 新增测试：boost 语义 2 项、usage 聚合 1 项；config roundtrip 更新。

## 五、验证

```text
cargo test --workspace   477 passed / 0 failed
cargo build --workspace  zero warnings
check_topology.py        PASS（17 crates + 8 apps）
release_gate.py          九 Gate 全 PASS（含 VR 10/10 byte-identical、S0=S1=S2=0）
```

## 六、剩余（下一批建议顺序）

1. **P1-E 正式立项**：冻结 UI-CONTRACT-v0.2（UiSnapshot/UiCommand/ActivityItem + INV-UI-001~010）→ 重生成 20 张 VR 基线 → ResultRow 图标/Provider badge/动态 hint。
2. 空查询 = 最近使用列表（`usage_map` 已备好数据，需 UI 呈现决策随 P1-E 一起做）。
3. Installer/Upgrade（zip + Start Menu shortcut + 升级保配置/索引）。
4. App Index 2.0（UWP/MSIX via AppUserModelID、.lnk 目标解析）、FS watch。
