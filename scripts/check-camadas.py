#!/usr/bin/env python3
"""As camadas do RavenRS (#1080): nenhum pacote depende de uma camada acima da sua.

`camadas.toml` lista os workspaces de baixo para cima e os pacotes de cada um. Este passo lê o
grafo com `cargo metadata --no-deps` (sem compilar nada) e recusa:

- um pacote do repositório que não está em camada nenhuma, ou está em duas;
- um nome em `camadas.toml` que não é pacote do repositório;
- uma dependência de caminho (normal, de build ou de teste) para uma camada que vem depois.

Na camada `apps` cada pacote é um workspace próprio; um app pode usar outro por caminho.

    scripts/check-camadas.py              confere o repositório
    scripts/check-camadas.py --self-test  confere o próprio passo, com uma aresta plantada
"""

import json
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent


def ler_camadas(texto):
    """Lê o subconjunto de TOML que `camadas.toml` usa: seções, strings, booleanos e listas de
    strings (o Python 3.9 dos Macs não traz `tomllib`). Devolve [(nome, {chave: valor})] na ordem."""
    camadas = []
    sem_comentarios = "\n".join(re.sub(r"\s+#.*$|^#.*$", "", linha) for linha in texto.splitlines())
    partes = re.split(r"^\[([\w-]+)\]\s*$", sem_comentarios, flags=re.M)
    for nome, corpo in zip(partes[1::2], partes[2::2]):
        campos = {}
        for chave, valor in re.findall(r"^([\w-]+)\s*=\s*(\[.*?\]|\"[^\"]*\"|true|false)", corpo, re.M | re.S):
            if valor.startswith("["):
                campos[chave] = re.findall(r'"([^"]+)"', valor)
            elif valor.startswith('"'):
                campos[chave] = valor[1:-1]
            else:
                campos[chave] = valor == "true"
        camadas.append((nome, campos))
    return camadas


def conferir(camadas, pacotes):
    """`pacotes`: {nome: [(dependência, tipo)]}, só as dependências de caminho. Devolve os problemas."""
    problemas = []
    nivel = {}
    for indice, (camada, campos) in enumerate(camadas):
        for pacote in campos.get("pacotes", []):
            if pacote in nivel:
                problemas.append(f"camadas.toml: {pacote} está em {camadas[nivel[pacote]][0]} e em {camada}")
            nivel[pacote] = indice
    for pacote in sorted(set(nivel) - set(pacotes)):
        problemas.append(f"camadas.toml: {pacote} não é pacote do repositório")
    for pacote in sorted(set(pacotes) - set(nivel)):
        problemas.append(f"{pacote} não está em camada nenhuma: ponha-o numa seção de camadas.toml")
    for pacote, dependencias in sorted(pacotes.items()):
        if pacote not in nivel:
            continue
        for dependencia, tipo in dependencias:
            if dependencia in nivel and nivel[dependencia] > nivel[pacote]:
                de, para = camadas[nivel[pacote]], camadas[nivel[dependencia]]
                problemas.append(
                    f"{pacote} ({de[1].get('rotulo', '')} {de[0]}) usa {dependencia} "
                    f"({para[1].get('rotulo', '')} {para[0]}) como dependência {tipo}: uma camada só usa "
                    "as de baixo; o que as duas precisam desce, ou a dependência sai"
                )
    return problemas


def pacotes_do_repositorio():
    saida = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout
    metadados = json.loads(saida)
    tipos = {None: "normal", "dev": "de teste", "build": "de build"}
    return {
        p["name"]: [(d["name"], tipos.get(d["kind"], d["kind"])) for d in p["dependencies"] if d.get("path")]
        for p in metadados["packages"]
    }


def autoteste():
    camadas = ler_camadas(
        '# comentário\n[baixo]\nrotulo = "W1" # fim de linha\npacotes = [\n  "a", "b",\n]\n'
        '[alto]\nrotulo = "W2"\num_workspace_por_pacote = true\npacotes = ["c"]\n'
    )
    assert [nome for nome, _ in camadas] == ["baixo", "alto"], camadas
    assert camadas[0][1] == {"rotulo": "W1", "pacotes": ["a", "b"]}, camadas
    assert camadas[1][1]["um_workspace_por_pacote"] is True, camadas
    assert conferir(camadas, {"a": [], "b": [("a", "normal")], "c": [("b", "de teste")]}) == []
    # A aresta plantada: um pacote de baixo que testa com um de cima.
    plantada = conferir(camadas, {"a": [("c", "de teste")], "b": [], "c": []})
    assert len(plantada) == 1 and "a (W1 baixo) usa c (W2 alto) como dependência de teste" in plantada[0], plantada
    faltando = conferir(camadas, {"a": [], "b": [], "c": [], "d": []})
    assert faltando == ["d não está em camada nenhuma: ponha-o numa seção de camadas.toml"], faltando
    sobrando = conferir(camadas, {"a": [], "b": []})
    assert sobrando == ["camadas.toml: c não é pacote do repositório"], sobrando
    print("check-camadas: autoteste passou")


def main():
    if "--self-test" in sys.argv[1:]:
        autoteste()
        return
    camadas = ler_camadas((root / "camadas.toml").read_text(encoding="utf-8"))
    pacotes = pacotes_do_repositorio()
    problemas = conferir(camadas, pacotes)
    if problemas:
        sys.exit("\n".join(problemas))
    print(f"{len(pacotes)} pacotes em {len(camadas)} camadas, nenhuma dependência sobe de camada")


main()
