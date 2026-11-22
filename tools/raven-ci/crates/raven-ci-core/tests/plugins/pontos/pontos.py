"""One of each point that is not a step, for the tests of the core. Only the standard library."""

import base64
import json
import os
import sys
import threading
import time

out = threading.Lock()
stops = {}
asked = [0]


def send(message):
    with out:
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()


def tick(job, times, stop):
    """Asks for a build of job every 100 ms, times times, with no call of the core in progress."""
    for n in range(times):
        time.sleep(0.1)
        if stop.is_set():
            return
        asked[0] += 1
        send({"jsonrpc": "2.0", "id": "t%d" % asked[0], "method": "host/request_build",
              "params": {"trigger": "tique", "watched": job,
                         "request": {"job": job, "cause": "tique %d" % n, "parameters": {}, "priority": 0}}})


def answer(method, params, call):
    if method == "initialize":
        return {"name": "pontos", "version": "0.1.0"}
    if method == "trigger/watch":
        stop = threading.Event()
        stops[params["job"]] = stop
        threading.Thread(target=tick, args=(params["job"], params["settings"].get("vezes", 2), stop),
                         daemon=True).start()
        return None
    if method == "trigger/unwatch":
        stops.pop(params["job"], threading.Event()).set()
        return None
    if method == "publisher/publish":
        folder = os.path.join(params["workspace"], ".raven-ci-anexos")
        os.makedirs(folder, exist_ok=True)
        big = os.path.join(folder, "grande")
        with open(big, "wb") as f:
            f.write(b"x" * 300000)
        small = base64.b64encode(("build %d" % params["build"]["number"]).encode()).decode()
        return [{"kind": "relatorio", "name": "pequeno.txt", "mime": "text/plain", "summary": {"n": 1}, "base64": small},
                {"kind": "relatorio", "name": "grande.bin", "mime": "application/octet-stream", "file": big}]
    if method == "hook/before":
        if (params.get("settings") or {}).get("recusar"):
            raise Refusal({"code": -32000, "message": "a configuração porteiro: sem espaço",
                           "data": {"kind": "settings", "field": "porteiro", "reason": "sem espaço"}})
        return None
    if method == "route/handle":
        body = base64.b64decode(params["request"]["body"]).decode()
        reply = ("eco de %s: %s" % (params["request"].get("actor"), body)).encode()
        return {"status": 201, "content_type": "text/plain", "body": base64.b64encode(reply).decode()}
    if method == "credential/hidden":
        token = params["credential"].get("token", "")
        return ["usuario:" + token]
    if method == "credential/secret":
        return params["credential"].get("usuario", "") + ":" + params["credential"].get("token", "")
    if method == "step/execute":
        if params["parameters"].get("cair"):
            sys.exit(3)
        if params["parameters"].get("anexos"):
            # Reads back what builds left: the list, then the bytes of the first.
            send({"jsonrpc": "2.0", "id": "a1", "method": "host/attachments",
                  "params": {"job": "site", "number": None, "kind": "relatorio"}})
            listed = json.loads(sys.stdin.readline())["result"]
            send({"jsonrpc": "2.0", "id": "a2", "method": "host/attachment", "params": {"id": listed[0]["id"]}})
            one = json.loads(sys.stdin.readline())["result"]
            text = base64.b64decode(one["base64"]).decode()
            return {"outputs": {"main": [{"nomes": [a["name"] for a in listed], "texto": text}]}}
        send({"jsonrpc": "2.0", "method": "host/hide", "params": {"call": call, "value": "chave-123"}})
        send({"jsonrpc": "2.0", "method": "host/build_log",
              "params": {"call": call, "step": "", "line": "do plugin: chave-123"}})
        return {"outputs": {"main": params["inputs"].get("main", [])}}
    raise Refusal({"code": -32601, "message": "não conheço " + method})


class Refusal(Exception):
    pass


for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method is None:
        continue  # the answer to a request_build: nothing waits for it
    if method == "shutdown":
        break
    try:
        send({"jsonrpc": "2.0", "id": message["id"], "result": answer(method, message.get("params") or {}, message["id"])})
    except Refusal as refusal:
        send({"jsonrpc": "2.0", "id": message["id"], "error": refusal.args[0]})
