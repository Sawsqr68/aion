#!/usr/bin/env python3
"""A minimal, conforming ACP agent that behaves like the real ones: it answers
`session/prompt` and then KEEPS RUNNING as a persistent stdio server. It exits only on stdin EOF,
and it prints a non-JSON banner on stdout the way a node-launched agent does."""
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
            "agentInfo": {"name": "fake-acp-agent", "version": "0.0.0"}}})
    elif method == "session/new":
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": "sess-live"}})
    elif method == "session/prompt":
        sid = msg["params"]["sessionId"]
        send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {
            "sessionUpdate": "agent_thought_chunk", "content": {"type": "text", "text": "thinking"}}}})
        # The exact shape that used to destroy a turn: a non-JSON line on stdout.
        sys.stdout.write("Debugger listening on ws://127.0.0.1:9229\n")
        sys.stdout.flush()
        for frag in ["abl", "ative"]:
            send({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": sid, "update": {
                "sessionUpdate": "agent_message_chunk", "messageId": "m1",
                "content": {"type": "text", "text": frag}}}})
        send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "_meta": {"costUsd": 0.01}}})
        # Deliberately do NOT exit: this is what every real ACP agent does.
    else:
        if "id" in msg and msg["id"] is not None:
            send({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": "no"}})

# stdin EOF: shut down cleanly. The adapter's exit grace must allow this to be observed.
sys.stderr.write("fake-acp-agent: stdin closed, exiting cleanly\n")
sys.stderr.flush()
time.sleep(0.2)
