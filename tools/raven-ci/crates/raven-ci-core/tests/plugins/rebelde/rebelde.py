"""Misbehaves when asked: `sleep` seconds before answering, `crash` to die, `secret` to ask the
host for a secret without declaring the permission."""
import json
import sys
import time


def send(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


for line in sys.stdin:
    message = json.loads(line)
    method, params = message.get("method"), message.get("params") or {}
    if method == "shutdown":
        break
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": message["id"], "result": {"name": "rebelde", "version": "1.0.0"}})
        continue
    parameters = params.get("parameters", {})
    if parameters.get("crash"):
        sys.exit(3)
    if parameters.get("sleep"):
        time.sleep(float(parameters["sleep"]))
    if parameters.get("secret"):
        send({"jsonrpc": "2.0", "id": "s", "method": "host/secret", "params": {"name": "token"}})
        answer = json.loads(sys.stdin.readline())
        send({"jsonrpc": "2.0", "id": message["id"], "error": {"code": -1, "message": answer["error"]["message"]}})
        continue
    send({"jsonrpc": "2.0", "id": message["id"], "result": {"outputs": {"main": [{"ok": True}]}}})
