#!/usr/bin/env python3
"""A conforming ACP agent that answers `session/prompt` and then IGNORES stdin EOF forever.

This is the failure the harness's exit grace exists for: an agent that does not shut down when its
stdin closes must not be able to hang the worker. With the grace elapsed the harness must kill and
reap it, so `wait_result` still returns.
"""
import json, sys, time

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {
            "protocolVersion": 1, "agentCapabilities": {}, "authMethods": [],
            "agentInfo": {"name": "deaf-acp-agent", "version": "0.0.0"}}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": "sess-deaf"}})
    elif method == "session/prompt":
        sid = msg["params"]["sessionId"]
        for frag in ["abl", "ative"]:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {
                "sessionUpdate": "agent_message_chunk", "messageId": "m1",
                "content": {"type": "text", "text": frag}}}})
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn"}})
    else:
        if "id" in msg and msg["id"] is not None:
            send({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": "no"}})

# stdin EOF reached and DELIBERATELY ignored: sleep past any plausible grace.
sys.stderr.write("deaf-acp-agent: stdin closed, ignoring it\n")
sys.stderr.flush()
time.sleep(600)
