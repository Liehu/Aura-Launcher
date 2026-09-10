# STATE-GENERATION（P210-004，spec §21-§25）

> 状态：CONTRACT（当前有效语义）。跨阶段状态分类 + Generation 注册表。

## 1. Lifecycle State 分类（§22）

各阶段的状态体系不合并为一个 enum，但每个状态必须归入以下六类之一；
不同类的状态**不可互相替代**：

| 类别 | 含义 | 归属状态（非穷举） |
|---|---|---|
| Intent State | 请求已表达，尚未规划 | AgentSession `Created` |
| Planning State | 正在决定"做什么" | Agent `Planning`/`Replanning`，Editor draft |
| Authorization State | 等待或已授予单次授权 | Approval `Pending`，UI `ConfirmationPending` |
| Execution State | 正在产生效果 | Workflow Run `Running`、Step `Executing`、Agent `Executing`、SystemEffect in-flight |
| Resource State | 长期资源/目录的生命周期 | Plugin `Enabled/Disabled/Quarantined`，Catalog generation |
| Recovery State | 效果未定/等待恢复 | Workflow `Paused`，Agent `WaitingForConfirmation`/`BudgetExhausted`，`EffectState::Unknown` |

**不变量 INV-STATE-101**：状态不承载授权——`Approved`/`Running` 不产生
Capability/Effect 权威；权威只来自 engine 铸造的 token（见
EFFECT-AUTHORITY.md）。

## 2. Generation 注册表（§24）

每个 generation 必须回答四问：version 什么 / 何时递增 / 使什么失效 /
失败是否递增。

| Generation | Version 什么 | 何时 +1 | 失效什么 | 失败 |
|---|---|---|---|---|
| Catalog generation | 已提交的应用目录（CatalogStore reconcile） | reconcile **提交**后 | App provider 读缓存 | 不递增 |
| Application generation (Core) | 应用目录的宿主可见版本 | `set/bump_application_generation`（采纳 committed gen 或手动 bump） | provider 结果缓存 | 不递增 |
| Context generation (Core) | popup 会话的上下文版本（MVP3.2-C） | 每次弹窗打开 | 旧 generation 的结果不得静默执行（INV-043/045） | 不递增 |
| Index generation | 索引内容版本（index_meta） | 全量 rebuild / rescan **提交**后 | FTS 排序缓存 | 不递增 |

**不变量**（§25 Generation ≠ Identity）：

1. generation 只表示"已提交状态变了几个版本"，**不能替代动态目标的
   身份校验**——窗口/进程用 PID/HWND + identity 字段（INV-IDENTITY-101），
   不用 generation 计数。
2. 失败（事务回滚、校验拒绝、执行失败）**永不递增**任何 generation。
3. Context staleness 判定（`results_gen != context_gen` → 拒绝静默执行）
   是 generation 的唯一合法用途：作废旧结果，不是授权新结果。

## 3. 与执行语义的接口

效果不确定性一律引用 EXECUTION-SEMANTICS-v1（`CommandResult` /
`EffectState` / Recovery Route）——Recovery State 的进入条件是
`EffectState::Unknown`，退出条件是 probe 定论（`probe_effect`）或显式
人工恢复；禁止把 Recovery State 当作可自动重试的普通失败。
