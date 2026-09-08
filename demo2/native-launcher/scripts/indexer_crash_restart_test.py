#!/usr/bin/env python3
"""GA-4 indexer crash/restart fault injection (03-test-plan §10/§16).

Drives the real launcher-indexer-service process:
  boot 1  -> rebuild + search via stdio JSON-RPC
  FAULT   -> hard kill (no shutdown RPC, abnormal exit)
  boot 2  -> same db must recover: pre-crash entries searchable,
             rebuild repopulates, file count eventually consistent.

Process launch is a fixed argv list (Popen(shell=False)) of a
compile-known binary path; no shell interpolation anywhere.

Usage:
    python scripts/indexer_crash_restart_test.py [--service PATH]

Exit 0 = PASS, 1 = FAIL.
"""

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SERVICE = ROOT / "target" / "debug" / "launcher-indexer-service.exe"


class Service:
    def __init__(self, exe: Path, db: Path, root: Path):
        # fixed-arity argv, shell=False — no command injection surface;
        # RUST_LOG=off keeps tracing logs off the JSON-RPC stdout channel
        import os
        argv = [str(exe), str(db), str(root)]
        env = {**os.environ, "RUST_LOG": "off"}
        self.proc = subprocess.Popen(
            argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, shell=False, env=env,
        )

    def rpc(self, req: dict) -> dict:
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError("service closed stdout unexpectedly")
        return json.loads(line)

    def search_count(self, query: str) -> int:
        resp = self.rpc({"id": 1, "method": "search", "params": {"query": query}})
        result = resp.get("result")
        return len(result) if isinstance(result, list) else 0

    def kill(self) -> int:
        self.proc.kill()
        return self.proc.wait()


def wait_for(service: Service, query: str, timeout: float = 30.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if service.search_count(query) >= 1:
            return
        service.rpc({"id": 2, "method": "status"})
        time.sleep(0.05)
    raise AssertionError(f"query {query!r} never became searchable")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--service", type=Path, default=SERVICE)
    args = ap.parse_args()
    if not args.service.exists():
        print(f"FATAL: service binary not found: {args.service}", file=sys.stderr)
        return 1

    tmp = Path(tempfile.mkdtemp(prefix="nl_idxcrash_"))
    try:
        db, root = tmp / "index.db", tmp / "corpus"
        root.mkdir()
        (root / "alpha-report.txt").write_bytes(b"a")
        (root / "beta-notes.md").write_bytes(b"b")

        # boot 1
        svc = Service(args.service, db, root)
        wait_for(svc, "alpha")
        assert svc.search_count("alpha") >= 1, "corpus searchable after boot 1"
        before = svc.rpc({"id": 3, "method": "status"})["result"]["files"]

        # FAULT: hard kill, no shutdown RPC
        code = svc.kill()
        assert code != 0, "hard kill must be an abnormal exit"
        print(f"[ok] boot 1 indexed {before} file(s), hard-killed (exit {code})")

        # boot 2: same db must recover
        svc = Service(args.service, db, root)
        wait_for(svc, "beta")
        assert svc.search_count("alpha") >= 1, "pre-crash entry searchable after restart"
        assert svc.search_count("beta") >= 1, "rebuild repopulates after restart"
        after = svc.rpc({"id": 4, "method": "status"})["result"]["files"]
        assert after == 2, f"index eventually consistent (got {after})"
        assert after >= before, "index content never shrinks across restart"
        print(f"[ok] boot 2 recovered: {after} file(s), searches intact")
        print("PASS")
        return 0
    except (AssertionError, RuntimeError, OSError) as e:
        print(f"FAIL: {e}", file=sys.stderr)
        return 1
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
