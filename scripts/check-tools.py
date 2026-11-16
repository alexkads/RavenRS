#!/usr/bin/env python3
"""Every folder in tools/ is a product, not an example, and has what tools/README.md lists.

    scripts/check-tools.py    exit 1, saying what each tool lacks

A tool has its own version (not the workspace's) and publish = false, its news and the history
of its versions, an InstalledApp in src/versao.rs, the installer made from the shared template,
its four icons, --versao and --self-test in src/main.rs, tests in tests/suite, a README, and a
tool.json saying what it offers, which the RavenRS MCP server answers with (`ravenrs_tools`).
"""

import json
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
NEEDED = [
    "README.md",
    "tool.json",
    "novidades.json",
    "historico-de-novidades.json",
    "install.cmd",
    "src/versao.rs",
    "src/main.rs",
    "tests/suite/main.rs",
    "assets/icon.svg",
    "assets/icon.png",
    "assets/icon.ico",
    "assets/icon.icns",
]

problems = []
for manifest in sorted(root.glob("tools/*/Cargo.toml")):
    tool = manifest.parent
    name = tool.relative_to(root)
    text = manifest.read_text()
    package = text.split("\n[", 1)[0]
    if not re.search(r'^version\s*=\s*"\d+\.\d+\.\d+"', package, re.M):
        problems.append(f"{name}: needs its own version (version = \"x.y.z\"), not the workspace's")
    if not re.search(r"^publish\s*=\s*false", package, re.M):
        problems.append(f"{name}: needs publish = false")
    if "ravenrs-app-update" not in text:
        problems.append(f"{name}: needs ravenrs-app-update, the installer and updater")
    for needed in NEEDED:
        if not (tool / needed).exists():
            problems.append(f"{name}: {needed} is missing")
    versao = tool / "src/versao.rs"
    if versao.exists():
        body = versao.read_text()
        for test in ("the_three_release_files_agree", "the_installer_is_the_one_the_template_makes"):
            if test not in body:
                problems.append(f"{name}: src/versao.rs needs the test {test}")
    main = tool / "src/main.rs"
    if main.exists() and "--versao" not in main.read_text():
        problems.append(f"{name}: src/main.rs needs --versao, which the installer runs")
    if main.exists() and '"--self-test"' not in main.read_text():
        problems.append(f"{name}: src/main.rs needs --self-test, the tool testing itself with no window")
    described = tool / "tool.json"
    if described.exists():
        try:
            card = json.loads(described.read_text())
        except ValueError as e:
            problems.append(f"{name}: tool.json is not JSON: {e}")
        else:
            version = re.search(r'^version\s*=\s*"([^"]+)"', package, re.M)
            if card.get("package") != tool.name:
                problems.append(f"{name}: tool.json says package {card.get('package')!r}, not {tool.name!r}")
            if version and card.get("version") != version.group(1):
                problems.append(f"{name}: tool.json says version {card.get('version')!r}, Cargo.toml {version.group(1)!r}")
            for field in ("name", "summary", "when_to_use", "commands", "limits"):
                if not card.get(field):
                    problems.append(f"{name}: tool.json needs {field}, which the MCP server answers with")

if problems:
    print("tools/ that are not ready as products (see tools/README.md):")
    for p in problems:
        print(f"  {p}")
    sys.exit(1)
print(f"tools: {len(list(root.glob('tools/*/Cargo.toml')))} ready")
