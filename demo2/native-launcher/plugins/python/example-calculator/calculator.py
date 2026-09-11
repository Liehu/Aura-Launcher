"""Python reference plugin: Calculator (mirrors the Rust calculator-plugin).

Contract chain under test: manifest v2 (runtime.type=python) ->
initialize -> query_id echo -> shutdown, via the Python SDK.

P3-UX security fix: eval() replaced with safe AST-walking evaluator.
The AST approach validates expression STRUCTURE (not just characters),
blocking creative bypasses while allowing all legitimate arithmetic.
"""

import ast
import operator
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from launcher_plugin import Command, Plugin  # noqa: E402

# allowed binary/unary operators (whitelist, fail-closed)
_BIN_OPS = {
    ast.Add: operator.add,
    ast.Sub: operator.sub,
    ast.Mult: operator.mul,
    ast.Div: operator.truediv,
    ast.Mod: operator.mod,
    ast.Pow: operator.pow,
}
_UNARY_OPS = {
    ast.USub: operator.neg,
    ast.UAdd: operator.pos,
}
_MAX_DEPTH = 32


def _eval_node(node: ast.expr, depth: int = 0) -> float:
    """Recursively evaluate an AST node. Only numeric literals and
    whitelisted arithmetic operators are permitted. Everything else
    raises ValueError (fail-closed)."""
    if depth > _MAX_DEPTH:
        raise ValueError("expression too deep")
    if isinstance(node, ast.Constant):
        if isinstance(node.value, (int, float)) and not isinstance(node.value, bool):
            return float(node.value)
        raise ValueError("only numeric literals allowed")
    if isinstance(node, ast.BinOp):
        op_type = type(node.op)
        if op_type not in _BIN_OPS:
            raise ValueError(f"operator not allowed: {op_type.__name__}")
        left = _eval_node(node.left, depth + 1)
        right = _eval_node(node.right, depth + 1)
        if op_type is ast.Div and right == 0:
            raise ValueError("division by zero")
        if op_type is ast.Mod and right == 0:
            raise ValueError("modulo by zero")
        return _BIN_OPS[op_type](left, right)
    if isinstance(node, ast.UnaryOp):
        val = _eval_node(node.operand, depth + 1)
        op_type = type(node.op)
        if op_type in _UNARY_OPS:
            return _UNARY_OPS[op_type](val)
        raise ValueError(f"unary operator not allowed: {op_type.__name__}")
    raise ValueError(f"node type not allowed: {type(node).__name__}")


def evaluate(expr: str) -> float | None:
    """Safely evaluate an arithmetic expression via AST walking.
    No eval(), no exec(), no name resolution — pure numeric computation."""
    try:
        # `^` in the original evaluator meant power (right-assoc), but
        # Python's `^` is XOR. Replace with `**` to preserve behavior.
        expr = expr.replace("^", "**")
        tree = ast.parse(expr.strip(), mode="eval")
        result = _eval_node(tree.body)
        if result != result or result in (float("inf"), float("-inf")):
            return None
        return result
    except (ValueError, SyntaxError, TypeError, ZeroDivisionError):
        return None


class Calculator(Plugin):
    def query(self, text: str) -> list[Command]:
        expr = text.replace("x", "*").strip()
        value = evaluate(expr) if expr else None
        if value is None:
            return [Command(title="Calculator (Python)",
                            subtitle="type an expression, e.g. 12+34*2", actions=[])]
        title = f"= {int(value)}" if value == int(value) else f"= {value}"
        return [Command(title=title, subtitle=text, actions=["copy"])]


Calculator().run()
