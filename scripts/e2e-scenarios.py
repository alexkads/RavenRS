#!/usr/bin/env python3
"""Every app with a window in examples/ and tools/ keeps scenarios the Raven Test Studio finds and runs (#702).

    scripts/e2e-scenarios.py               # checks; exit 1 saying what each app lacks
    scripts/e2e-scenarios.py --self-test   # checks the checks, on made-up projects

An app is a package under examples/ (independent workspaces included, such as the contracts) or
tools/ that builds a binary (src/main.rs) and depends on a RavenRS crate. Each one:

- declares `[package.metadata.ravenrs.e2e]` with `app` (the name its scenarios open) and
  `binary` (one it builds), and no other app opens by the same name;
- depends on `ravenrs-e2e-wire`, which is how the Studio knows it speaks the test channel, and
  answers it: its sources name `SERVE_FLAG` (`--e2e-serve`);
- keeps `.raventest` files in `tests/e2e/`, each a scenario that opens the declared app and
  checks something (`check` or `mention`), not only clicks;
- plays them on every `cargo test`: a file under `tests/` reads `tests/e2e` (`FOLDER`) and plays
  them (`Scenario::play`, `play_served`, or each block's `command()` as Raven Finanças does).

An app with no window of its own goes in NO_SCREEN, with the issue that says why; it is refused
there once it keeps scenarios, so the list only shrinks. CONTRIBUTING.md describes the pattern.

Nothing here builds, runs an app or needs a network: the Studio and the tests do that.
"""

import pathlib
import re
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

# Apps with no window of their own: they write to the terminal or drive other apps.
NO_SCREEN = {
    "examples/contracts/associacao": (707, "prints each notice to the terminal; no window"),
    "examples/contracts/cursos": (708, "prints each notice to the terminal; no window"),
    "tools/raven-channel-video": (705, "a command line that records other apps; no window of its own"),
}

CHECKS = {"check", "conferir", "mention", "mencionar", "menciona"}


def section(manifest, name):
    """The lines of `[name]` in a Cargo.toml, up to the next section."""
    match = re.search(r"^\[" + re.escape(name) + r"\]\s*$(.*?)(?=^\[|\Z)", manifest, re.M | re.S)
    return match.group(1) if match else None


def key(body, name):
    match = re.search(r'^\s*' + re.escape(name) + r'\s*=\s*"([^"]*)"', body or "", re.M)
    return match.group(1) if match else None


def dependencies(manifest):
    """The crates a manifest depends on, in any of its dependency sections."""
    names = set()
    for header, body in re.findall(r"^\[([^\]]*dependencies)\]\s*$(.*?)(?=^\[|\Z)", manifest, re.M | re.S):
        if "dev-dependencies" in header:
            continue
        names.update(re.findall(r"^([A-Za-z0-9_-]+)(?:\.workspace)?\s*=", body, re.M))
    return names


def apps(root):
    """The folders of the apps made with RavenRS, from the repository's root."""
    found = []
    for manifest in sorted(list(root.glob("examples/**/Cargo.toml")) + list(root.glob("tools/*/Cargo.toml"))):
        folder = manifest.parent
        if "target" in folder.relative_to(root).parts or not (folder / "src/main.rs").exists():
            continue
        if any(d == "ravenrs" or d.startswith("ravenrs-") for d in dependencies(manifest.read_text())):
            found.append(folder)
    return found


def scenario_problems(file, app):
    """What is wrong with one scenario file, for an app that opens as `app`."""
    lines = [l.split("|", 1)[0].strip() for l in file.read_text().splitlines()]
    lines = [l for l in lines if l and not l.startswith("//")]
    problems = []
    if not lines or not lines[0].startswith("scenario:"):
        problems.append("starts with `scenario: <name>`")
    words = [l.split(None, 1) for l in lines[1:]]
    opens = [w[1].strip() for w in words if len(w) == 2 and w[0].lower() in ("open", "abrir")]
    if app not in opens:
        problems.append(f"opens the app it declares (`open {app}`)")
    if not any(w and w[0].lower() in CHECKS for w in words):
        problems.append("checks something (`check` or `mention`), not only clicks")
    return problems


def problems_of(root, folder, names):
    name = str(folder.relative_to(root))
    manifest = (folder / "Cargo.toml").read_text()
    e2e_folder = folder / "tests/e2e"
    scenarios = sorted(e2e_folder.glob("*.raventest")) if e2e_folder.is_dir() else []
    if name in NO_SCREEN:
        issue, _ = NO_SCREEN[name]
        if scenarios:
            return [f"{name}: keeps scenarios now; take it out of NO_SCREEN in scripts/e2e-scenarios.py (#{issue})"]
        return []
    problems = []
    declared = section(manifest, "package.metadata.ravenrs.e2e")
    app, binary = key(declared, "app"), key(declared, "binary")
    if declared is None or not app or not binary:
        problems.append("declares [package.metadata.ravenrs.e2e] with app and binary")
    else:
        package = key(section(manifest, "package"), "name")
        bins = [key(b, "name") for b in re.findall(r"^\[\[bin\]\]\s*$(.*?)(?=^\[|\Z)", manifest, re.M | re.S)]
        if binary not in [package, *bins]:
            problems.append(f"binary = \"{binary}\" is a binary it builds ({', '.join(filter(None, [package, *bins]))})")
        if app in names:
            problems.append(f"opens by a name of its own; \"{app}\" is also {names[app]}'s")
        names[app] = name
    if "ravenrs-e2e-wire" not in dependencies(manifest):
        problems.append("depends on ravenrs-e2e-wire (features = [\"serve\"]), so the Studio knows it speaks the channel")
    sources = "\n".join(p.read_text() for p in (folder / "src").rglob("*.rs"))
    if "SERVE_FLAG" not in sources and '"--e2e-serve"' not in sources:
        problems.append("answers the channel: opened with ravenrs_e2e_wire::SERVE_FLAG, it serves before any window")
    if not scenarios:
        problems.append("keeps .raventest scenarios in tests/e2e/")
    for file in scenarios:
        for what in scenario_problems(file, app or ""):
            problems.append(f"tests/e2e/{file.name} {what}")
    tests = [p.read_text() for p in (folder / "tests").rglob("*.rs")] if (folder / "tests").is_dir() else []
    if scenarios and not any(("FOLDER" in t or "tests/e2e" in t) and any(way in t for way in ("play(", "play_served(", ".command()")) for t in tests):
        problems.append("plays tests/e2e on cargo test (a test that reads ravenrs_e2e_wire::FOLDER and plays each scenario)")
    return [f"{name}: {p}" for p in problems]


def check(root):
    names, problems = {}, []
    found = apps(root)
    for folder in found:
        problems += problems_of(root, folder, names)
    listed = {str(f.relative_to(root)) for f in found}
    for gone in sorted(set(NO_SCREEN) - listed):
        problems.append(f"{gone}: is in NO_SCREEN but is not an app any more; take it out")
    return found, problems


def self_test():
    global NO_SCREEN
    kept = NO_SCREEN
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)

        def project(path, manifest, main="", scenario=None, test=None):
            folder = root / path
            (folder / "src").mkdir(parents=True)
            (folder / "Cargo.toml").write_text(manifest)
            (folder / "src/main.rs").write_text(main)
            if scenario is not None:
                (folder / "tests/e2e").mkdir(parents=True)
                (folder / "tests/e2e/a.raventest").write_text(scenario)
            if test is not None:
                (folder / "tests").mkdir(exist_ok=True)
                (folder / "tests/scenarios.rs").write_text(test)

        good = '[package]\nname = "bom"\n\n[package.metadata.ravenrs.e2e]\napp = "Bom"\nbinary = "bom"\n\n[dependencies]\nravenrs.workspace = true\n\n[target.\'cfg(not(target_arch = "wasm32"))\'.dependencies]\nravenrs-e2e-wire = { workspace = true, features = ["serve"] }\n'
        played = "let f = ravenrs_e2e_wire::FOLDER; scenario.play(&mut e2e);"
        project("examples/bom", good, "SERVE_FLAG", "scenario: Bom\nopen Bom | Abrir\ncheck #x = 1 | Confere\n", played)
        project("examples/clica", good.replace('"bom"', '"clica"').replace('"Bom"', '"Clica"'), "SERVE_FLAG",
                "scenario: Clica\nopen Clica\nclick #x\n", played)
        project("examples/mesmo-nome", good.replace('name = "bom"', 'name = "outro"').replace('binary = "bom"', 'binary = "outro"'),
                "SERVE_FLAG", "scenario: Outro\nopen Bom\ncheck #x = 1\n", played)
        project("examples/nada", '[package]\nname = "nada"\n\n[dependencies]\nravenrs-widgets-kit.workspace = true\n')
        project("examples/lib-de-terminal", '[package]\nname = "t"\n\n[dependencies]\nserde.workspace = true\n')
        project("tools/sem-tela", '[package]\nname = "st"\n\n[dependencies]\nravenrs.workspace = true\n')
        NO_SCREEN = {"tools/sem-tela": (1, "terminal"), "examples/sumiu": (2, "gone")}
        try:
            found, problems = check(root)
        finally:
            NO_SCREEN = kept
        said = "\n".join(problems)
        assert not any(p.startswith("examples/bom:") for p in problems), said
        assert "examples/lib-de-terminal" not in [str(f.relative_to(root)) for f in found], said
        assert "examples/clica: tests/e2e/a.raventest checks something" in said, said
        assert "examples/mesmo-nome: opens by a name of its own" in said, said
        for lack in ("declares [package.metadata.ravenrs.e2e]", "depends on ravenrs-e2e-wire", "answers the channel",
                     "keeps .raventest scenarios"):
            assert f"examples/nada: {lack}" in said, lack + "\n" + said
        assert not any(p.startswith("tools/sem-tela:") for p in problems), said
        assert "examples/sumiu: is in NO_SCREEN but is not an app any more" in said, said
        (root / "tools/sem-tela/tests/e2e").mkdir(parents=True)
        (root / "tools/sem-tela/tests/e2e/a.raventest").write_text("scenario: x\nopen X\ncheck #a = b\n")
        NO_SCREEN = {"tools/sem-tela": (1, "terminal")}
        try:
            _, problems = check(root)
        finally:
            NO_SCREEN = kept
        assert any("tools/sem-tela: keeps scenarios now" in p for p in problems), problems
    print("e2e-scenarios: self-test ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return 0
    found, problems = check(ROOT)
    if problems:
        print("Apps the Raven Test Studio cannot test as CONTRIBUTING.md asks (#702):")
        for problem in problems:
            print(f"  {problem}")
        return 1
    print(f"e2e-scenarios: {len(found) - len(NO_SCREEN)} apps keep scenarios the Studio runs; "
          f"{len(NO_SCREEN)} with no window ({', '.join(f'#{i}' for i, _ in NO_SCREEN.values())})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
