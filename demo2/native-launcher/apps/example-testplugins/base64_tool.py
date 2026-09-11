import base64, json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def encode(text):
    return base64.b64encode(text.encode()).decode()

def decode(text):
    return base64.b64decode(text).decode()

def ui(text_value="", output_value=""):
    return {
        "schema_version": 1,
        "root": {"type": "section", "id": "main", "children": [
            {"type": "text", "id": "label_in", "title": "Input"},
            {"type": "text_area", "id": "input", "value": text_value},
            {"type": "container", "id": "ops", "children": [
                {"type": "button", "id": "encode", "title": "Encode"},
                {"type": "button", "id": "decode", "title": "Decode"},
            ]},
            {"type": "text", "id": "label_out", "title": "Output"},
            {"type": "text_area", "id": "output", "value": output_value, "readonly": True},
        ]}
    }

def do_op(node_id, text):
    if node_id == "encode":
        return encode(text)
    elif node_id == "decode":
        return decode(text)
    return text

for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    rid = req.get("id")
    params = req.get("params") or {}

    if method == "initialize":
        pv = params.get("protocol_version", "0.1")
        send({"jsonrpc": "2.0", "id": rid, "result": {"protocol_version": pv}})

    elif method == "query":
        qid = params.get("query_id")
        send({"jsonrpc": "2.0", "id": rid, "result": {"query_id": qid, "commands": [
            {"id": "base64", "title": "🔐 Base64 Tool",
             "subtitle": "Encode / Decode text", "score": 0.9, "actions": []}
        ]}})

    elif method == "tool.open":
        sid = params.get("session_id", "")
        send({"jsonrpc": "2.0", "id": rid, "result": {
            "session_id": sid, "ui_generation": 0,
            "ui": ui()}})

    elif method == "tool.event":
        sid = params.get("session_id", "")
        node = params.get("node_id", "")
        event = params.get("event", "")
        val = ""
        if event in ("click", "submit"):
            v = params.get("value", "")
            # find the input text from the value or default
            text = v if isinstance(v, str) and v else ""
            if node == "encode":
                try:
                    val = encode(text) if text else ""
                except Exception:
                    val = ""
            elif node == "decode":
                try:
                    val = decode(text) if text else ""
                except Exception:
                    val = ""
        # respond with updated output
        gen = params.get("_gen", 0)
        send({"jsonrpc": "2.0", "id": rid, "result": {
            "session_id": sid,
            "ui": ui(text_value=val, output_value=val)}})

    elif method == "shutdown":
        send({"jsonrpc": "2.0", "id": rid, "result": {"bye": True}})
        break
