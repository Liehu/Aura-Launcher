# P3 — Adaptive Launcher Platform 1.0

**状态：FROZEN DESIGN DRAFT**

---

# 1. 阶段定位

P3 不再是 P2.x 的“能力补充”。

P3 是：

> **从 Reactive Launcher → Adaptive Launcher 的架构跃迁。**

P2.6–P2.9 已建立：

```text
Search
Workflow
AI / Agent
Plugin
System Integration
Effect Authority
Durable Execution
```

P3 在其之上建立：

```text
Context
Event
Intent
Memory
Pattern
Prediction
Suggestion
Feedback
Learning
Adaptive Planning
```

完整闭环。

---

# 2. P3 产品目标

最终用户不需要区分：

```text
Search
AI
Workflow
Plugin
System Command
```

而只需要表达：

> **“我现在想完成什么。”**

系统内部：

```text
User
  ↓
Context
  ↓
Intent
  ↓
Candidate Actions
  ↓
Suggestion / Plan
  ↓
Policy
  ↓
Approval
  ↓
Execution
  ↓
Observation
  ↓
Feedback
  ↓
Learning
```

---

# 3. P3 核心跃迁

## P2

```text
Input
  ↓
Intent
  ↓
Command
  ↓
Execute
```

## P3

```text
Input
+
Context
+
History
+
Environment
        ↓
Intent Model
        ↓
Candidate Generation
        ↓
Prediction
        ↓
Suggestion
        ↓
Plan
        ↓
Policy / Approval
        ↓
Execution
        ↓
Observation
        ↓
Feedback
        ↓
Learning
```

---

# 4. P3 不变量

必须继承 P2.10：

```text
Prediction ≠ Authorization
Context ≠ Capability
Memory ≠ Authority
Planning ≠ Execution
Observation ≠ Truth
Learning ≠ Policy
Proposal ≠ Effect
Trust ≠ Authority
Capability ≠ Authority
Workflow ≠ Effect Authority
```

新增：

```text
Suggestion ≠ Command
Feedback ≠ Policy
Pattern ≠ User Intent
Prediction ≠ User Intent
Memory ≠ Current Context
```

---

# 5. 产品分层

P3 建立六层：

```text
L1 Context
L2 Intent
L3 Intelligence
L4 Planning
L5 Execution
L6 Learning
```

结构：

```text
             Intelligence
                  │
       ┌──────────┼──────────┐
       ▼          ▼          ▼
    Context     Intent     Memory
       │          │          │
       └──────────┼──────────┘
                  ▼
             Prediction
                  ▼
             Suggestion
                  ▼
                Plan
                  ▼
          Policy / Approval
                  ▼
             Execution
                  ▼
             Observation
                  ▼
              Feedback
                  │
                  └────→ Learning
```

---

# 6. Context Model

Context 是：

> 对“用户当前工作环境”的有限投影。

Context 不是：

```text
system state dump
filesystem dump
history dump
credential store
capability store
```

---

# 7. ContextItem

统一结构：

```text
ContextItem {
    id
    type
    value
    source
    timestamp
    generation
    sensitivity
    confidence
    expires_at
}
```

例如：

```text
ForegroundApplication
ForegroundWindow
CurrentFolder
SelectedFiles
Clipboard
RecentAction
PowerState
NetworkState
```

---

# 8. Context 来源

允许：

```text
Search
System Provider
Application Catalog
Workflow
Plugin Provider
Clipboard
User Input
Session State
```

来源必须显式标记：

```text
source
```

禁止匿名 Context。

---

# 9. Context 生命周期

Context 默认：

```text
ephemeral
```

除非显式声明：

```text
persistent
```

Context 必须支持：

```text
TTL
generation
invalidate
redaction
```

---

# 10. Context Confidence

Context 可以有：

```text
confidence
```

但：

```text
confidence ≠ authorization
```

例如：

```text
current_folder confidence = 1.0
```

也不能意味着：

```text
filesystem.write allowed
```

---

# 11. Context Graph

P3 引入：

```text
ContextGraph
```

节点：

```text
App
Window
File
Folder
Project
Workflow
Plugin
Clipboard
Session
```

边：

```text
contains
opened_by
selected_in
associated_with
recently_used
generated_from
belongs_to
```

---

# 12. Context Graph 原则

Context Graph 只描述：

```text
“当前可能相关的实体关系”
```

不能描述：

```text
“可以对实体做什么”
```

因此：

```text
Graph Edge
≠
Capability
```

---

# 13. Intent

P3 Intent 分为：

```text
ExplicitIntent
InferredIntent
PredictedIntent
```

其中：

```text
ExplicitIntent
```

来自用户直接表达。

```text
InferredIntent
```

来自当前上下文分析。

```text
PredictedIntent
```

来自历史模式。

---

# 14. Intent Priority

优先级：

```text
Explicit
    >
Contextual Inference
    >
Prediction
```

预测只能补充：

```text
candidate
```

不能覆盖：

```text
explicit user intent
```

---

# 15. Prediction

Prediction 可以预测：

```text
next action
next search
workflow candidate
plugin candidate
context transition
routine
```

例如：

```text
User frequently:
VS Code
→ Git status
→ Terminal
```

系统可以推测：

```text
Git status
```

但只能：

```text
Suggest
```

不能自动执行。

---

# 16. Suggestion

正式对象：

```text
Suggestion {
    suggestion_id
    type
    intent
    target
    reason
    confidence
    risk
    expires_at
}
```

---

# 17. Suggestion Types

```text
SearchSuggestion
ActionSuggestion
WorkflowSuggestion
PluginSuggestion
ContextSuggestion
RoutineSuggestion
AgentSuggestion
```

---

# 18. Suggestion 生命周期

```text
Generated
  ↓
Presented
  ↓
Accepted
  ↓
Executed
```

其他：

```text
Ignored
Rejected
Expired
Dismissed
Modified
```

---

# 19. Feedback

Feedback 不允许直接修改 Policy。

例如：

```text
User rejected suggestion
```

只能影响：

```text
ranking
prediction
future suggestion score
```

不能影响：

```text
capability policy
security policy
approval requirement
```

---

# 20. Pattern

P3 将重复行为抽象成：

```text
BehaviorPattern
```

例如：

```text
Pattern:
    Open project
    → terminal
    → run test
    → inspect result
```

---

# 21. Pattern ≠ Intent

重复行为不意味着当前用户一定想重复。

因此：

```text
Pattern
    ↓
Candidate
```

不能：

```text
Pattern
    ↓
Execute
```

---

# 22. Routine

Pattern 达到一定置信度后可以形成：

```text
Routine
```

Routine 描述：

```text
trigger context
conditions
suggested workflow
frequency
confidence
```

第一阶段：

```text
Suggestion-only
```

不要默认：

```text
Autonomous execution
```

---

# 23. Memory Model

P3 Memory 分层：

```text
SessionMemory
RunMemory
PreferenceMemory
PatternMemory
RoutineMemory
```

不允许：

```text
EverythingMemory
```

即：

> 不把所有历史事件永久保存成一个巨大上下文。

---

# 24. Preference Memory

例如：

```text
preferred_search_provider
preferred_terminal
preferred_workflow
preferred_AI_provider
preferred_result_count
```

Preference：

```text
user-controlled
editable
deletable
```

---

# 25. Pattern Memory

记录：

```text
pattern
frequency
recency
confidence
accepted_count
rejected_count
last_seen
```

不直接记录完整历史内容。

---

# 26. Memory Privacy

Memory 必须支持：

```text
inspect
edit
delete
reset
export
```

用户应该可以回答：

> “为什么你认为我经常做这个？”

---

# 27. Learning

学习只允许影响：

```text
Ranking
Prediction
Suggestion
Pattern confidence
Workflow recommendation
```

不得影响：

```text
Policy
Capability
Trust
Approval requirements
Security controls
```

---

# 28. Learning Model

P3 不要求 ML 模型。

第一阶段优先：

```text
frequency
recency
context association
accept/reject feedback
manual preference
```

采用：

```text
deterministic scoring
```

而不是一开始引入黑盒模型。

---

# 29. Adaptive Ranking

P2.5 已有：

```text
Ranking
RankingWeights
Explainability
```

P3 增加：

```text
AdaptiveRankingFeatures
```

例如：

```text
habit score
context association
time association
recent task association
suggestion feedback
```

但结果必须仍然：

```text
deterministic given same state
```

---

# 30. Ranking Safety

Adaptive ranking 可以：

```text
改变顺序
```

不能：

```text
生成不存在的 Action
授予 Capability
绕过 Policy
绕过 Confirmation
```

---

# 31. Planning

P3 Planner 统一：

```text
SearchPlan
ActionPlan
WorkflowPlan
AgentPlan
```

Planner 输出：

```text
PlanProposal
```

不是：

```text
Effect
```

---

# 32. Plan

Plan 包含：

```text
goal
steps
dependencies
expected_outputs
risk
estimated_cost
confidence
```

---

# 33. Plan Validation

所有 Plan：

```text
Schema
Semantic
Capability
Target
Policy
Budget
```

全部通过后才能执行。

---

# 34. Observation

Execution 后生成：

```text
Observation
```

Observation 包括：

```text
effect result
state changes
errors
context changes
target changes
```

---

# 35. Observation ≠ Truth

Observation 必须有：

```text
source
timestamp
confidence
```

例如：

```text
Plugin reports success
```

不能天然认为：

```text
system state definitely changed
```

必须区分：

```text
reported
verified
unknown
```

---

# 36. Agent 与 P3

Agent Runtime 继续使用 P2.7：

```text
Observe
→ Plan
→ Validate
→ Approve
→ Execute
→ Observe
→ Replan
```

P3 新增：

```text
Context
Memory
Prediction
Feedback
```

但 Agent authority 不扩大。

---

# 37. Agent Boundary

```text
Agent
    ↓
Proposal
    ↓
Plan
    ↓
Policy
    ↓
ActionEngine
    ↓
Effect
```

Agent 永远不能：

```text
modify policy
grant capability
modify trust
modify security configuration
```

---

# 38. Context → AI

Context 发送给 AI 之前：

```text
Collect
→ Classify
→ Redact
→ Minimize
→ Bound
→ Build Prompt
```

不得：

```text
Context dump
```

---

# 39. Proactive AI

P3 可以主动建议：

```text
“你似乎正在重复这个操作”
“是否创建 Workflow？”
“这个文件可能需要处理”
```

但默认：

```text
suggest
```

而不是：

```text
execute
```

---

# 40. Autonomous Mode

P3 不默认开放 unrestricted autonomy。

如未来支持：

```text
Autonomous Routine
```

必须绑定：

```text
scope
risk
allowlist
budget
time window
capability
rollback
kill switch
```

建议作为 P3.2+。

---

# 41. Event Model

P3 引入统一：

```text
Event
```

事件来源：

```text
User
System
Search
Workflow
Plugin
Agent
Timer
Hotkey
Context
```

---

# 42. Event ≠ Effect

例如：

```text
FileChanged
```

只是：

```text
Event
```

不是：

```text
Command
```

事件需要经过：

```text
Trigger
→ Condition
→ Proposal
→ Policy
→ Execution
```

---

# 43. Event Schema

统一：

```text
Event {
    event_id
    type
    source
    timestamp
    payload
    generation
    correlation_id
}
```

---

# 44. Event Ordering

不能假设：

```text
Event A
一定先于
Event B
```

系统必须通过：

```text
timestamp
sequence
generation
correlation_id
```

做判断。

---

# 45. Correlation

P3 统一支持：

```text
correlation_id
```

关联：

```text
Intent
Plan
WorkflowRun
Execution
Effect
Observation
Feedback
```

---

# 46. User Goal

P3 新增：

```text
Goal
```

区别于：

```text
Intent = 用户当下表达
Goal   = 系统理解后的目标
```

例如：

```text
Intent:
“把这个项目整理一下”

Goal:
prepare_project_for_submission
```

---

# 47. Goal 生命周期

```text
Created
→ Clarifying
→ Planned
→ Approved
→ Executing
→ Observing
→ Completed
```

失败：

```text
Failed
Cancelled
Blocked
```

---

# 48. Goal 不拥有 Authority

Goal 只能驱动：

```text
Planning
```

不能直接：

```text
Execute
```

---

# 49. P3 Unified Architecture

```text
                    USER
                      │
                      ▼
                  Input / Event
                      │
          ┌───────────┴───────────┐
          ▼                       ▼
       Context                 History
          │                       │
          └───────────┬───────────┘
                      ▼
                 Intent Engine
                      │
             ┌────────┴────────┐
             ▼                 ▼
        Goal Model        Prediction
             │                 │
             └────────┬────────┘
                      ▼
                  Suggestion
                      │
                      ▼
                   Planner
                      │
                      ▼
                Plan Proposal
                      │
                      ▼
            Validation / Policy
                      │
                      ▼
                  Approval
                      │
                      ▼
               ActionEngine
                      │
                      ▼
                   Effect
                      │
                      ▼
                 Observation
                      │
             ┌────────┴────────┐
             ▼                 ▼
         Feedback          Verification
             │
             ▼
           Learning
             │
             └──────────────→ Ranking / Prediction
```

---

# 50. P3 Architecture Hard Boundaries

```text
Context → no authority
Memory → no authority
Prediction → no authority
Suggestion → no authority
Goal → no authority
Planner → no authority
Agent → no authority
Feedback → no policy mutation
Learning → no policy mutation
```

唯一 Effect Authority：

```text
ActionEngine
```

---

# 51. P3 第一阶段范围

P3.0：

```text
Context Graph
Event Model
Intent / Goal
Memory Model
Suggestion Model
Feedback Model
Adaptive Ranking
Routine Detection
Unified Planning
```

---

# 52. P3 暂不做

明确后置：

```text
full autonomous agent
cloud memory
multi-device sync
remote collaborative workflow
enterprise policy
full marketplace
black-box online learning
unbounded background agent
automatic destructive routine
```

---

# 53. P3 产品阶段

```text
P3.0 Adaptive Foundation
        ↓
P3.1 Personal Automation
        ↓
P3.2 Proactive Intelligence
```

---

# 54. P3.0

重点：

```text
Context
Event
Goal
Memory
Suggestion
Feedback
```

---

# 55. P3.1

重点：

```text
Routine
Workflow Recommendation
Context Trigger
Personal Automation
```

---

# 56. P3.2

重点：

```text
Prediction
Proactive Suggestion
Adaptive Agent
Cross-context Planning
```

---

# 57. P3 的核心成功标准

P3 不是：

```text
AI 更聪明
```

而是：

```text
用户少输入
系统少解释
结果更相关
建议更准确
重复工作更少
安全边界不下降
```

---

# 58. P3 最终产品形态

用户：

```text
“我要开始做今天的开发任务”
```

系统可以识别：

```text
当前项目
当前 IDE
当前分支
当前目录
最近工作
相关 Workflow
历史习惯
```

生成：

```text
Suggestion:
    打开项目环境
    检查 Git
    打开终端
    运行测试
```

用户确认：

```text
Start
```

系统才：

```text
Plan
→ Validate
→ Approve
→ Execute
```

并在执行后：

```text
Observe
→ Learn
```

---

# 59. P3 核心定义

> **Aura Launcher 不再只是搜索入口，也不是聊天机器人。**

它是：

> **一个 Context-aware、Goal-oriented、Policy-bounded、Adaptive 的个人操作层。**

::

:::writing{variant="document" id="83051" title="P3 — Agentic Coding 开发规范与任务矩阵"}
