#!/usr/bin/env python3
"""The example plugin of Raven CI: a step and a command, over JSON-RPC 2.0 by stdio.

Raven CI starts this program at the first call, writes one JSON request per line to its input
and reads one JSON message per line from its output. Anything written to the error output goes
to the server's log. See tools/raven-ci/docs/writing-a-plugin.md.
"""
import json
import sys

NAME = "hello"
VERSION = "0.1.0"

_asked = 0


def send(message):
    """Writes one message, on one line, and flushes it: the server waits for the whole line."""
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def log(line):
    """A notification: the server writes the line to its log and does not answer."""
    send({"jsonrpc": "2.0", "method": "host/log", "params": {"line": line}})


def ask_host(method, params):
    """Asks the server for a service (host/secret, host/settings, host/data_dir) and waits for
    the answer. The server checks the permission the plugin.toml declared."""
    global _asked
    _asked += 1
    my_id = "hello-%d" % _asked
    send({"jsonrpc": "2.0", "id": my_id, "method": method, "params": params})
    for line in sys.stdin:
        message = json.loads(line)
        if message.get("id") == my_id:
            if "error" in message:
                raise RuntimeError(message["error"]["message"])
            return message.get("result")
    raise RuntimeError("the server closed the channel")


def step(params):
    """Runs one step `hello`: each item that came in goes on with a greeting."""
    parameters = params.get("parameters", {})
    who = parameters.get("who", "mundo")
    if "secret" in parameters:
        # Only works when the plugin.toml declares the permission "secrets".
        ask_host("host/secret", {"name": parameters["secret"]})
        who = who + " (com o segredo lido)"
    log("dizendo olá para %s" % who)
    items = params.get("inputs", {}).get("main") or [{}]
    out = [dict(item, hello="Olá, %s!" % who) for item in items]
    return {"outputs": {"main": out}}


def command(params):
    """Runs `raven-ci hello <names>`."""
    names = " ".join(params.get("args", [])) or "mundo"
    return {"code": 0, "output": "Olá, %s!\n" % names}


HANDLERS = {
    "initialize": lambda params: {"name": NAME, "version": VERSION},
    "step/execute": step,
    "cli/run": command,
}


def main():
    for line in sys.stdin:
        if not line.strip():
            continue
        message = json.loads(line)
        method = message.get("method")
        if method == "shutdown":
            return
        if "id" not in message:
            continue
        handler = HANDLERS.get(method)
        try:
            if handler is None:
                raise RuntimeError("o hello não conhece o método %s" % method)
            send({"jsonrpc": "2.0", "id": message["id"], "result": handler(message.get("params") or {})})
        except Exception as error:  # the server shows the message; the plugin stays up
            send({"jsonrpc": "2.0", "id": message["id"], "error": {"code": -32000, "message": str(error)}})


if __name__ == "__main__":
    main()
