# P3-UX Agentic Coding 开发规范 — Launcher Experience 2.0

**状态：FROZEN PROCESS**

## 批次矩阵

### UX-A（窗口布局与主题）
| 任务 | 内容 | 锚点 |
|---|---|---|
| P30X-01 | Theme 亮色调色板完整化（所有 token 双套） | `theme.slint` |
| P30X-02 | `theme_mode` config 字段 + 注册表 system 跟随 | `launcher-config` + `main.rs` |
| P30X-03 | 窗口高度内容驱动（去固定 420px） | `app.slint` window-height |
| P30X-04 | 窗口显示淡入动画（120ms） | `app.slint` |

### UX-B（搜索与结果）
| 任务 | 内容 | 锚点 |
|---|---|---|
| P30X-11 | `fuzzy.rs` 返回匹配位置区间 | `launcher-search/src/fuzzy.rs` |
| P30X-12 | ResultItem 增 `match-range` + Slint 高亮渲染 | `result-row.slint` |
| P30X-13 | 结果行重设计（28px 图标+元数据+快捷键提示） | `result-row.slint` |
| P30X-14 | 空查询图标网格（最近使用） | `app.slint` + `main.rs` |
| P30X-15 | 搜索框无边框嵌入 | `search-box.slint` |

### UX-C（打磨）
| 任务 | 内容 | 锚点 |
|---|---|---|
| P30X-21 | 剪贴板粘贴条 | `app.slint` + `main.rs` |
| P30X-22 | 管理窗口 General 页扩展 | `management.slint` |
| P30X-23 | VR 基线重生成（dark × 10） | `scripts/` |

## 执行纪律

沿用 P3.0/P3.1 惯例：契约先行 / 小步提交 / 每批 Gate / 测试维度矩阵。
Gate 必查：UX-R1~R4 + VR 确定性 + 性能预算不退化。

## 允许/禁止

允许：launcher-ui 内新增组件与 token、launcher-search 新增纯函数、
launcher-config 新增字段。禁止：改 FROZEN 契约语义、UI 层 IO、
注入式 hook、绕过权威链。
