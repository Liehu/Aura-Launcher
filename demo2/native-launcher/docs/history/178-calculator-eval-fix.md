# 178 — 安全修复：calculator.py eval → AST 安全求值器

> 日期：2026-09-11。范围：安全审计 finding #1 修复（high: eval 代码注入）。
> 输入基线：history/177。

## 根因与修复

calculator.py 的 `evaluate()` 使用 `eval(expr, {"__builtins__": {}}, {})`
——虽然限制了 builtins 和字符白名单，但 Python eval 的逃逸技术
（`().__class__.__bases__` 等无需字母即可构造）意味着这不是安全边界。

修复：用 **AST 安全求值器** 替代 eval——`ast.parse(mode="eval")` →
白名单遍历（仅 Constant/BinOp/UnaryOp + 六种算术运算符）→ 逐节点递归
求值（深度上限 32）。任何非白名单 AST 节点立即 raise ValueError。

安全改进：
- **结构性验证**：验证表达式 AST 结构而非字符集
- **无名称解析**：Call/Name/Attribute/Import 节点直接拒绝
- **深度上限**：防止栈溢出
- **除零显式处理**：Div/Mod 右值为 0 → ValueError
- **`^` → `**` 预处理**：保持原语义（幂运算）

## 测试

- 基础算术：12+34*2=80, (1+2)/3=1, 2^10=1024(经预处理), -5+3=-2, 7%3=1
- 安全注入：__import__/abc/1..2/1+(2 全部拒绝
- 有限性：NaN/Inf → None

## Gate 结果

- `cargo test --workspace`：925 passed / 0 failed（Python 测试不在
  cargo test 内，由 python_sdk_e2e 覆盖）
- 手工验证：12+34*2=80 ✓, 2^10=1024 ✓, 1/0→None ✓,
  __import__→None ✓
