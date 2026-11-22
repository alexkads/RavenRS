#!/usr/bin/env python3
"""Checks that every public type keeps the testability contract of the Raven Test Studio (#734, #736).

    scripts/rts-coverage.py               # checks; exit 1 with what is missing
    scripts/rts-coverage.py --self-test   # checks the checks, on made-up manifests and sources
    scripts/rts-coverage.py --issues      # also refuses a pending entry whose issue is closed (asks gh)

tools/raven-test-studio/contrato.json has one entry per type of the feature center's
tools/raven-feature-center/recursos.json, `{"crate", "tipo", "rts"}`, where `rts` is one of:

- complete: `{"ids": [...], "acoes": [...], "busy": bool, "driver": "...", "cenario": "..."}`.
  `ids` are the test ids the component gives its parts, each `part:<key>`; `acoes` the accessible
  actions it takes (`AccessAction`); `driver` and `cenario` are optional and, when given, must be
  a `pub struct` of the `testing` module of its crate and a `.raventest` under a `tests/e2e`
  folder, which the RTS finds;
- `{"sem_tela": true}` for a type with nothing on the screen (a database, an option set);
- `{"pendente": true, "issue": N}` while its family's issue has not given it the contract.

The list lives with the RTS, apart from recursos.json, so it and the feature center change in
parallel without stepping on each other (the RCB keeps its pendentes.json the same way).

It also refuses, anywhere in the sources, a test id made with `format!` that does not start with
`part:`, or that numbers the items of a collection by position (`{i}`, `{index}`, `{idx}`): a test
id stays with the data when the list sorts, filters or scrolls, so it is made from the key.

A complete entry agrees with the code both ways: every action beyond a click it lists is named in
its crate (`AccessAction::Expand`), and every action the file of the type declares with
`BuildCx::on_access` is listed.

Nothing here needs a network, a GPU or a compiler, except `--issues`, which asks `gh`.
"""
import json
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = root / "tools/raven-feature-center/recursos.json"
CONTRACT = root / "tools/raven-test-studio/contrato.json"
ACCESS = root / "crates/ravenrs-core/src/access.rs"
BASIC = {"Click", "Focus", "Increment", "Decrement"}
# The format! that is the test id itself: `.test_id(format!(…))` or `cx.test_id(node, &format!(…))`.
TEST_ID = re.compile(r'test_id\(\s*(?:[\w.]+\s*,\s*)?&?\s*format!\(\s*"([^"]*)"\s*(?:,\s*([^)]*))?\)', re.S)
INDEXED = re.compile(r"\{(?:i|index|idx)(?::[^}]*)?\}")
POSITIONAL_INDEX = re.compile(r"^\s*(?:i|index|idx|self\.index)\s*$")
PART = re.compile(r"^[a-z][a-z-]*:")


def actions_of(access_text):
    """The variants of `AccessAction`, as the core declares them."""
    body = access_text.split("pub enum AccessAction", 1)[1].split("}", 1)[0]
    return set(re.findall(r"^\s*(\w+),", body, re.M))


def test_id_problems(path, text):
    """Each test id built with `format!` that is not `part:<key>`, with its file and line."""
    out = []
    for match in TEST_ID.finditer(text):
        literal, args = match.group(1), match.group(2) or ""
        line = text.count("\n", 0, match.start()) + 1
        where = f"{path}:{line}"
        if not PART.match(literal):
            out.append(f"{where}: test id {literal!r} is not part:<key> (as row:<key>, cell:<key>:<column>)")
        elif INDEXED.search(literal) or any(POSITIONAL_INDEX.match(a) for a in args.split(",") if a.strip()):
            out.append(f"{where}: test id {literal!r} numbers the items by position; make it from the key")
    return out


def entry_problems(entry, known_actions, crate_source, type_file, drivers, exists):
    """What is wrong with one entry of the contract."""
    name = f"{entry.get('tipo')} ({entry.get('crate')})"
    rts = entry.get("rts")
    if not isinstance(rts, dict) or not rts:
        return [f"{name}: no rts block; give it ids and actions, sem_tela, or pendente with its issue"]
    kinds = [k for k in ("sem_tela", "pendente") if rts.get(k)] + (["ids"] if "ids" in rts else [])
    if len(kinds) != 1:
        return [f"{name}: the rts block must be one of complete (ids), sem_tela or pendente, not {kinds or 'none'}"]
    if rts.get("pendente"):
        return [] if isinstance(rts.get("issue"), int) else [f"{name}: pendente needs the issue that gives it the contract"]
    if rts.get("sem_tela"):
        return []
    out = []
    # A control with no parts of its own (a checkbox) has no ids: its name finds it.
    for test_id in rts["ids"]:
        if not PART.match(test_id) and not re.match(r"^[a-z][a-z-]*$", test_id):
            out.append(f"{name}: test id {test_id!r} is not part:<key>, nor the name of a single part")
    listed = set(rts.get("acoes", []))
    for action in sorted(listed - known_actions):
        out.append(f"{name}: {action} is not an AccessAction")
    for action in sorted((listed & known_actions) - BASIC):
        if f"AccessAction::{action}" not in crate_source:
            out.append(f"{name}: the contract lists {action}, which the code of its crate never names")
    for call in re.findall(r"on_access\((.*?)\)\s*;", type_file, re.S):
        for action in re.findall(r"AccessAction::(\w+)", call):
            if action not in listed:
                out.append(f"{name}: the code declares {action} with on_access, which the contract does not list")
    if "driver" in rts and rts["driver"] not in drivers:
        out.append(f"{name}: driver {rts['driver']} is not a pub struct of the testing module of its crate")
    if "cenario" in rts:
        scenario = rts["cenario"]
        if not scenario.endswith(".raventest") or "/tests/e2e/" not in f"/{scenario}":
            out.append(f"{name}: scenario {scenario} is not a .raventest under a tests/e2e folder, where the RTS finds it")
        elif not exists(scenario):
            out.append(f"{name}: scenario {scenario} does not exist")
    if "busy" in rts and not isinstance(rts["busy"], bool):
        out.append(f"{name}: busy is true or false")
    return out


def coverage_problems(manifest, contract):
    """Types of the feature center with no entry, entries of no type and entries twice."""
    wanted = {(c["crate"], c["tipo"]) for c in manifest}
    seen, out = set(), []
    for entry in contract:
        key = (entry.get("crate"), entry.get("tipo"))
        if key in seen:
            out.append(f"{key[1]} ({key[0]}): two entries in contrato.json")
        seen.add(key)
        if key not in wanted:
            out.append(f"{key[1]} ({key[0]}): in contrato.json but not a type of recursos.json")
    for crate, tipo in sorted(wanted - seen):
        out.append(f"{tipo} ({crate}): no rts block in tools/raven-test-studio/contrato.json")
    return out


def view_impl(tipo, texts):
    """The `impl View for` block of the type, where it declares what it takes; empty if none."""
    start = re.compile(rf"^impl(?:<[^>]*>)?\s+(?:[\w:]+::)?(?:View|Component)\s+for\s+{re.escape(tipo)}\b", re.M)
    for text in texts:
        if found := start.search(text):
            end = re.search(r"^(?:impl|pub |fn |struct |enum |#\[)", text[found.end():], re.M)
            return text[found.start(): found.end() + (end.start() if end else len(text))]
    return ""


def sources(folder):
    return {p: p.read_text() for p in folder.rglob("*.rs") if "/tests/" not in str(p)} if folder.exists() else {}


def check(issues=False):
    manifest = json.loads(MANIFEST.read_text())["componentes"]
    contract = json.loads(CONTRACT.read_text())["tipos"]
    known = actions_of(ACCESS.read_text())
    problems = coverage_problems(manifest, contract)
    crates = {}
    for entry in contract:
        crate = entry.get("crate")
        if crate not in crates:
            crates[crate] = sources(root / "crates" / str(crate) / "src")
        files = crates[crate]
        defining = re.compile(rf"pub (?:struct|enum) {re.escape(str(entry.get('tipo')))}\b")
        type_file = view_impl(str(entry.get("tipo")), files.values())
        drivers = {d for p, t in files.items() if p.name == "testing.rs" for d in re.findall(r"pub struct (\w+)", t)}
        problems += entry_problems(
            entry, known, "\n".join(files.values()), type_file, drivers, lambda s: (root / s).is_file()
        )
    for folder in ("crates", "tools", "examples"):
        for path, text in sources(root / folder).items():
            if "test_id(" in text:
                problems += test_id_problems(path.relative_to(root), text)
    if issues:
        pending = sorted({e["rts"]["issue"] for e in contract if e.get("rts", {}).get("pendente")})
        for number in pending:
            state = subprocess.run(
                ["gh", "issue", "view", str(number), "--json", "state", "--jq", ".state"],
                capture_output=True, text=True, check=False,
            ).stdout.strip()
            if state == "CLOSED":
                problems.append(f"#{number} is closed, but contrato.json still has types pending on it")
    return problems, contract


def self_test():
    known = {"Click", "Focus", "Increment", "Decrement", "Expand", "Collapse", "SetValue"}
    manifest = [{"crate": "kit", "tipo": "Tree"}, {"crate": "kit", "tipo": "Db"}]
    lacks = coverage_problems(manifest, [{"crate": "kit", "tipo": "Db", "rts": {"sem_tela": True}}])
    assert lacks == ["Tree (kit): no rts block in tools/raven-test-studio/contrato.json"], lacks
    assert test_id_problems("a.rs", 'cx.test_id(node, &format!("row:{index}"));') == [
        "a.rs:1: test id 'row:{index}' numbers the items by position; make it from the key"
    ]
    assert test_id_problems("a.rs", 'cx.test_id(node, &format!("row:{}", i));')
    assert test_id_problems("a.rs", 'cx.test_id(node, &format!("{}", key));')
    assert test_id_problems("a.rs", 'cx.test_id(node, &format!("row:{}", key(self.index)));') == []
    tree = {"crate": "kit", "tipo": "Tree", "rts": {"ids": ["item:<key>"], "acoes": ["Click", "Expand"]}}
    said = entry_problems(tree, known, "on_click", "", set(), lambda s: True)
    assert said == ["Tree (kit): the contract lists Expand, which the code of its crate never names"], said
    code = "cx.on_access(node, &[AccessAction::Expand, AccessAction::Collapse], |a, _| true);"
    said = entry_problems(tree, known, code, code, set(), lambda s: True)
    assert said == ["Tree (kit): the code declares Collapse with on_access, which the contract does not list"], said
    tree["rts"]["driver"] = "TreeDriver"
    tree["rts"]["cenario"] = "tests/e2e/tree.raventest"
    said = entry_problems(tree, known, code, "", set(), lambda s: False)
    assert any("driver TreeDriver" in s for s in said) and any("does not exist" in s for s in said), said
    assert entry_problems({"crate": "kit", "tipo": "X", "rts": {"pendente": True}}, known, "", "", set(), bool)
    assert entry_problems({"crate": "kit", "tipo": "X"}, known, "", "", set(), bool)
    print("rts-coverage: self-test ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return
    problems, contract = check(issues="--issues" in sys.argv)
    if problems:
        print("The testability contract of the RTS is not kept (#734):")
        for p in problems:
            print(f"  {p}")
        sys.exit(1)
    count = lambda key: sum(1 for e in contract if key(e["rts"]))
    print(
        f"{len(contract)} types: {count(lambda r: 'ids' in r)} keep the RTS contract, "
        f"{count(lambda r: r.get('sem_tela'))} have no screen, {count(lambda r: r.get('pendente'))} pending"
    )


if __name__ == "__main__":
    main()
