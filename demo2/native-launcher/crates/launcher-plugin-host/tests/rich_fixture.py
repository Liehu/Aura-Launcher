import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    params = req.get("params") or {}
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": rid,
              "result": {"protocol_version": params.get("protocol_version")}})
    elif method == "query":
        qid = params.get("query_id")
        send({"jsonrpc": "2.0", "id": rid, "result": {"query_id": qid, "commands": [
            {"id": "rich", "title": "Rich", "actions": [],
             "rich": {"blocks": [{"type": "text", "text": "hello rich"}]}}
        ]}})
    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": rid, "result": {"bye": True}})
        break
