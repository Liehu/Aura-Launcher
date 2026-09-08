"""Python reference plugin: Calculator (mirrors the Rust calculator-plugin).

Contract chain under test: manifest v2 (runtime.type=python) ->
initialize -> query_id echo -> shutdown, via the Python SDK.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from launcher_plugin import Command, Plugin  # noqa: E402


def evaluate(expr: str) -> float | None:
    allowed = set("0123456789+-*/%(). ")
    if not expr or not set(expr) <= allowed:
        return None
    try:
        # restricted eval: digits/operators only, no names
        return float(eval(expr, {"__builtins__": {}}, {}))  # noqa: S307
    except Exception:
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
