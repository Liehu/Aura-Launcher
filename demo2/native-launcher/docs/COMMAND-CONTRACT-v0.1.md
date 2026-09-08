# COMMAND-CONTRACT-v0.1（Revision 2 · Proposed）

**Status:** **FROZEN v0.1**（review 16 有条件通过；语义修补完成后经 ADR-0011 冻结，2026-09-04）
**Date:** 2026-09-04
**来源:** review 14-mvp3-0.1 + review 15-mvp3-0.1；Raycast(Command/Action)、uTools(Feature)、Lertaro(Context)、Asyar(Capability)
**前提:** Plugin Contract v0.1 FROZEN；本契约是 Domain 层延伸，不改 wire 协议。UI Schema 不在本契约内。

---

## 1. Goal

契约化"插件如何描述一个可发现/可选择的对象及其操作"：

```text
Plugin → Command → ActionDescriptor → Action Resolution → Action Engine → Effect
```

## 2. Command Identity

**三层身份语义（冻结，见 INV-027/028）：**

```text
Command.id   = stable logical identity（资源身份或逻辑动作身份）
Action.id    = stable within Command（Command 内局部唯一，可跨 Command 重复）
Action.type  = 全局执行语义（见 ACTION-CONTRACT §2）
provider_id  = Host authority（Host 生成，插件不可声明）
```

**Command.id 生命周期**：表示资源/逻辑动作身份，而非查询实例身份：

```text
✅ github.issue:123   file:C:\a\b.txt   app:{known-id}   calc.result
❌ query-104-result-7
```

违反后果：Action Panel / history / cache / 键盘选择的 id 定位全部失效（INV-016 依赖此语义）。动态搜索结果无法给出稳定 id 时，允许 id 含去稳定化的哈希后缀，但 MUST NOT 含 query 序号。

## 3. Command Fields

```json
{
  "id": "calc.result",
  "title": "= 80",
  "subtitle": "12 + 34 * 2",
  "icon": null,
  "keywords": ["calc"],
  "category": "command",
  "score": 0.0,
  "requires_context": [],
  "actions": []
}
```

- 字段与 `launcher_domain::Command` 对齐；未知 optional 字段 MUST 忽略。
- **`score` ∈ [0, 1]**（插件排名 hint），最终排名仍归 Core：`final = lexical + hint*weight + type_prior + frequency + recency + context`。超界值按 0 处理 + WARN。
- **`provider_id` Host-authoritative**（INV-029）：插件输出中的 provider_id 一律忽略；Host 统一生成 `plugin:<manifest.id>`。插件侧字段标记为 optional/ignored。
- **`kind`（resource_kind）** 为 v0.2 预留；现有 `category` 保持"搜索结果类型"语义，二者将来并存不合并。

## 4. Context Eligibility（requires_context）

**`requires_context` 表达 eligibility（展示资格），不是 permission（数据/效果权限）**——与 capability 是两个正交维度（INV-032）：

```text
requires_context = ["folder"]      → 有 folder 才展示（ eligibility）
requires = ["context.location.read"] → 插件有权读 location（permission）
```

- v0.1 取值：`folder`。预留：`selection` / `clipboard.file` / `foreground.process`。
- Host 按 Popup Session 冻结的 ContextSnapshot 判定（INV-011 不变），插件不得自行探测系统状态。
- 未知取值 → 该 Command 丢弃 + WARN（不 kill）。

## 5. Action Association

- `actions[]` 有序；**"第一个通过 Action Resolution 的合法 action 为 primary"**（规则式，不是"数组第 0 项"——未知/非法 action 不占据 primary 位）；其余为 secondary。
- `role` 显式字段为 v0.2 预留（AI 生成/动态/禁用 action 出现时再引入）。
- **`actions = []` → informational / non-executable**：Command 可展示（纯信息条目），Enter 无效果；`actions != []` 且至少一个合法 primary → executable（INV-031 的 Command 侧）。

## 6. Ranking Hint

见 §3 score 规则。插件不得通过任何字段直接决定全局排序。

## 7. Provider Identity

`provider_id` 由 Host 从 manifest identity 生成；host/plugin 信任边界同 Plugin Contract §25。插件声明 provider_id 不产生任何效果。

## 8. Validation / Error Semantics

| 情形 | 处置 |
|---|---|
| 未知 `requires_context` 值 | 丢弃该 Command + WARN |
| `score` 超界 | 按 0 处理 + WARN |
| 未知/非法 secondary action | 丢弃该 action，Command 保留（ACTION-CONTRACT §6） |
| 未知/非法 primary action | Command 保留但 non-executable，或 fallback 到下一合法 action |
| Command 本身非法（无 id/title） | 丢弃该 Command |

## 8.1 Revision 3 语义修补（review 16，冻结前最后一批）

1. **全局命令身份 = (provider_id, command.id)**：不同插件可有相同 `command.id`（`plugin:a + calc.result` 与 `plugin:b + calc.result` 不冲突），插件作者无需生成 UUID；plugin 内 id 保持局部稳定。
2. **provider_id Host-authoritative 且永远不来自不可信来源**（INV-029 强化）：Plugin / AI / MCP / Remote provider 输出中的 provider_id 一律忽略——provider identity 属于 source provenance，不是可修改的显示属性。
3. **primary = 第一个 Ready action，且 Resolver 保持插件声明的 action 顺序**（永不重排；Copy→Disabled、Insert→Ready 时 Insert 即 primary）。
4. **score 三态区分**：缺省 = no hint；`0` = 显式最低 hint；越界 = 归一化为 0 + WARN。前两者语义不同，不得混淆。
5. **requires_context ≠ capability**（已有，重申为冻结项，INV-032）。
6. **Action-level fault containment 总原则**：畸形 Action 的爆炸半径必须最小（Action-level failure），只有 Command 本身非法才丢弃 Command（INV-031）。

## 9. 验收（进入实现前必须回答）

1. calculator-plus（**只实现 system.\* action**，`plugin.*` 留给第二个参考插件）：`= 80` 携带 primary(Copy) + secondary(Insert/History)，不接触 Host internals。
2. `requires_context: ["folder"]` 的 command 在 Explorer 前台出现、否则消失。
3. Enter 恒等于执行 primary action；Action Panel 呈现 secondary。
4. Rust 与 Python SDK 对同一 Command JSON 产出一致（SDK Conformance）。
