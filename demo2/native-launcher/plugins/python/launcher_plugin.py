"""Native Launcher Plugin SDK v0.1 (Python).

Cross-language validation of Plugin Contract v0.1: a plugin author writes
only a ``query()`` method and never sees JSON-RPC, ``query_id`` echo,
initialize/shutdown handshake, or stdin/stdout framing.

Contract guarantees handled here (PLUGIN-CONTRACT-v0.1):
  - initialize handshake + protocol_version negotiation
  - query_id echo ({"query_id", "commands"} response profile)
  - shutdown -> clean exit
  - stdout is a protocol channel: ``print()`` is redirected to stderr

Usage:
    from launcher_plugin import Plugin, Command

    class Calculator(Plugin):
        def query(self, text):
            return [Command(title="= 4", subtitle=text)]

    Calculator().run()
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field

PROTOCOL_VERSION = "0.1"

# JSON-RPC error codes (contract section 11)
PARSE_ERROR = -32700
INVALID_REQUEST = -32600
METHOD_NOT_FOUND = -32601
INVALID_PARAMS = -32602
VERSION_MISMATCH = -32007


@dataclass
class Command:
    """One discoverable/executable result item (Native UI Schema list item)."""

    title: str
    subtitle: str | None = None
    icon: str | None = None
    actions: list[str] = field(default_factory=list)

    def to_dict(self) -> dict:
        d: dict = {"title": self.title}
        if self.subtitle is not None:
            d["subtitle"] = self.subtitle
        if self.icon is not None:
            d["icon"] = self.icon
        d["actions"] = list(self.actions)
        return d


class Plugin:
    """Base class: override ``query(text) -> list[Command]``."""

    def query(self, text: str) -> list[Command]:
        return [Command(title=f"query: {text}", actions=[])]

    # -- internals (plugin authors stop reading here) ---------------------

    def run(self) -> None:
        # stdout is the protocol channel (INV-019): anything the plugin
        # author prints must go to stderr instead.
        protocol_out = sys.stdout
        sys.stdout = sys.stderr

        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                req = json.loads(line)
            except json.JSONDecodeError:
                self._send(protocol_out, {"jsonrpc": "2.0", "id": 0,
                                          "error": {"code": PARSE_ERROR, "message": "parse error"}})
                continue
            rid = req.get("id", 0)
            method = req.get("method", "")
            params = req.get("params") or {}

            if method == "initialize":
                if params.get("protocol_version") != PROTOCOL_VERSION:
                    self._send(protocol_out, {"jsonrpc": "2.0", "id": rid, "error": {
                        "code": VERSION_MISMATCH,
                        "message": f"unsupported protocol_version: {params.get('protocol_version')}"}})
                    continue
                self._send(protocol_out, {"jsonrpc": "2.0", "id": rid,
                                          "result": {"protocol_version": PROTOCOL_VERSION}})
            elif method == "query":
                qid = params.get("query_id")
                text = params.get("text", "")
                if qid is None:
                    self._send(protocol_out, {"jsonrpc": "2.0", "id": rid, "error": {
                        "code": INVALID_PARAMS, "message": "missing query_id"}})
                    continue
                try:
                    items = self.query(text)
                    commands = [i.to_dict() if isinstance(i, Command) else dict(i) for i in items]
                except Exception as e:  # plugin error must not corrupt the stream
                    self._send(protocol_out, {"jsonrpc": "2.0", "id": rid, "error": {
                        "code": INVALID_PARAMS, "message": f"query failed: {e}"}})
                    continue
                self._send(protocol_out, {"jsonrpc": "2.0", "id": rid,
                                          "result": {"query_id": qid, "commands": commands}})
            elif method == "shutdown":
                self._send(protocol_out, {"jsonrpc": "2.0", "id": rid, "result": {"bye": True}})
                break
            else:
                self._send(protocol_out, {"jsonrpc": "2.0", "id": rid, "error": {
                    "code": METHOD_NOT_FOUND, "message": f"unknown method: {method}"}})

    @staticmethod
    def _send(out, obj: dict) -> None:
        out.write(json.dumps(obj, ensure_ascii=False) + "\n")
        out.flush()
