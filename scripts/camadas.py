#!/usr/bin/env python3
"""As camadas do RavenRS e os workspaces que saem delas (#1080), num script só.

`camadas.toml` lista as camadas de baixo para cima. Uma camada com `separada = true` já é um
workspace Cargo fora do da raiz: a que tem `pasta` é um workspace naquela pasta, e a que tem
`um_workspace_por_pacote` faz de cada pacote um workspace na pasta dele.

    scripts/camadas.py check            todo pacote numa camada só; nenhuma dependência (normal, de
                                        build ou de teste) sobe de camada
    scripts/camadas.py deps             os workspaces separados têm a cópia em dia do que vem do
                                        Cargo.toml da raiz (versão, dependências, lints e perfis), e
                                        nenhum Cargo.lock resolve um crate externo noutra versão
    scripts/camadas.py deps --write     escreve essa cópia em cada workspace separado e cria o
                                        Cargo.lock que faltar, a partir do da raiz
    scripts/camadas.py workspaces       as pastas dos workspaces separados, na ordem das camadas,
                                        cada uma seguida dos pacotes dela (uma por linha)
    scripts/camadas.py workspaces --mudados
                                        só as que a mudança contra CHECK_BASE (origin/master) toca
    scripts/camadas.py publicacao       os `cargo publish --workspace` de cada workspace com pacote
                                        publicável, na ordem das camadas (F4, #1087)
    scripts/camadas.py --self-test      confere o próprio script, com uma aresta plantada

O Cargo não herda `[workspace.dependencies]` entre workspaces. O Cargo.toml da raiz continua sendo
a lista canônica; cada workspace separado leva uma cópia gerada entre as marcas `>>> camadas.py` e
`<<< camadas.py`, com os `path` reescritos para a pasta dele. Depois de mudar a raiz (uma versão
nova, uma dependência), rode `scripts/camadas.py deps --write`.
"""

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
INICIO = "# >>> camadas.py: gerado do Cargo.toml da raiz por `python3 scripts/camadas.py deps --write`; não edite à mão"
FIM = "# <<< camadas.py"


# --- camadas.toml --------------------------------------------------------------------------------


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


def camadas_do_repositorio():
    return ler_camadas((root / "camadas.toml").read_text(encoding="utf-8"))


# --- check ---------------------------------------------------------------------------------------


def conferir(camadas, pacotes):
    """`pacotes`: {nome: [(dependência, tipo)]}, só as dependências de caminho. Devolve os problemas."""
    problemas = []
    na_raiz = [nome for nome, campos in camadas if not campos.get("separada")]
    if len(na_raiz) > 1 or any(not campos.get("raiz") for nome, campos in camadas if nome in na_raiz):
        problemas.append(
            f"camadas.toml: só a camada com `raiz = true` fica no workspace da raiz; {', '.join(na_raiz)} "
            "não dizem `separada = true` (F4, #1087)"
        )
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


def metadados(pasta):
    saida = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"],
        cwd=pasta, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(saida)


def pacotes_do_repositorio(camadas):
    """Os pacotes de todos os workspaces: o da raiz e cada um dos separados."""
    tipos = {None: "normal", "dev": "de teste", "build": "de build"}
    pacotes = {}
    for pasta in [root] + [root / p for p in workspaces(camadas)]:
        for p in metadados(pasta)["packages"]:
            pacotes[p["name"]] = [(d["name"], tipos.get(d["kind"], d["kind"])) for d in p["dependencies"] if d.get("path")]
    return pacotes


# --- os workspaces separados ---------------------------------------------------------------------


def pastas_dos_pacotes():
    """{nome: pasta relativa à raiz} de todo Cargo.toml de pacote do repositório, lido do texto: um
    pacote de workspace separado não aparece no `cargo metadata` da raiz."""
    pastas = {}
    for manifesto in sorted(root.glob("**/Cargo.toml")):
        relativo = manifesto.relative_to(root)
        if relativo.parts[0] in ("target", "site", "npm", "docs") or "contracts" in relativo.parts or "target" in relativo.parts:
            continue
        pacote = re.search(r"^\[package\]\s*\n(?:.*\n)*?name\s*=\s*\"([^\"]+)\"", manifesto.read_text(encoding="utf-8"), re.M)
        if pacote:
            pastas[pacote.group(1)] = relativo.parent.as_posix()
    return pastas


def separados(camadas, pastas=None):
    """[(pasta do workspace, [pastas dos membros relativas a ela])] das camadas separadas, na ordem."""
    pastas = pastas if pastas is not None else pastas_dos_pacotes()
    lista = []
    for nome, campos in camadas:
        if not campos.get("separada"):
            continue
        if campos.get("um_workspace_por_pacote"):
            lista.extend((pastas[p], []) for p in campos.get("pacotes", []))
        elif "pasta" in campos:
            base = campos["pasta"]
            membros = sorted(
                os.path.relpath(pastas[p], base) for p in campos.get("pacotes", []) if pastas[p] != base
            )
            lista.append((base, membros))
        else:
            raise SystemExit(f"camadas.toml: a camada {nome} é separada, mas não diz `pasta` nem `um_workspace_por_pacote`")
    return lista


def workspaces(camadas):
    return [pasta for pasta, _ in separados(camadas)]


def pastas_do_workspace(pasta, membros):
    """As pastas, relativas à raiz, que são do workspace em `pasta`: ela e a de cada membro. Os
    membros de uma camada da biblioteca ficam fora dela, em `crates/` (F3, #1086)."""
    return [pasta] + [os.path.normpath(os.path.join(pasta, m)) for m in membros]


def pacotes_de(camadas, pasta, pastas=None):
    """Os pacotes do workspace em `pasta`: o dela, os de dentro dela e os dos membros de fora."""
    pastas = pastas if pastas is not None else pastas_dos_pacotes()
    membros = dict(separados(camadas, pastas)).get(pasta, [])
    fora = set(pastas_do_workspace(pasta, membros)[1:])
    return sorted(
        nome for nome, lugar in pastas.items() if lugar == pasta or lugar.startswith(pasta + "/") or lugar in fora
    )


def membros_da_raiz(camadas, pastas):
    """As pastas dos pacotes que ficam no workspace da raiz: os das camadas não separadas."""
    return sorted(pastas[p] for _, campos in camadas if not campos.get("separada") for p in campos.get("pacotes", []))


def raiz_com_membros(raiz_toml, membros, excluidos):
    """O Cargo.toml da raiz com `members` e `exclude` da seção `[workspace]` trocados. Com `members`
    explícito e `crates` em `exclude`, um crate de camada separada não vira membro da raiz, nem
    quando um membro dela o usa por caminho (o Cargo junta à raiz toda dependência de caminho de
    dentro da pasta dela que não esteja excluída)."""
    lista = lambda nomes: "[\n" + "".join(f'    "{n}",\n' for n in nomes) + "]"
    cabeca, resto = raiz_toml.split("\n[workspace.", 1)
    cabeca = re.sub(r"^members = \[.*?\]", lambda _: "members = " + lista(membros), cabeca, count=1, flags=re.M | re.S)
    cabeca = re.sub(r"^exclude = \[.*?\]", lambda _: "exclude = " + lista(excluidos), cabeca, count=1, flags=re.M | re.S)
    return cabeca + "\n[workspace." + resto


def com_workspace(manifesto, relativo):
    """O Cargo.toml de um membro com `workspace = "<relativo>"` logo depois do `name` do `[package]`."""
    sem = re.sub(r'^workspace = "[^"]*"\n', "", manifesto, count=1, flags=re.M)
    return re.sub(
        r'^(\[package\]\s*\n(?:.*\n)*?name\s*=\s*"[^"]+"\n)', lambda m: m.group(1) + f'workspace = "{relativo}"\n',
        sem, count=1, flags=re.M,
    )


def secoes(texto):
    """[(cabeçalho, texto da seção com o cabeçalho)] de um Cargo.toml."""
    partes = re.split(r"^(\[[^\]\n]+\])[ \t]*$", texto, flags=re.M)
    return [(cabecalho[1:-1], cabecalho + corpo) for cabecalho, corpo in zip(partes[1::2], partes[2::2])]


def bloco(raiz_toml, pasta, membros, extras=()):
    """A cópia gerada para o workspace em `pasta`: `[workspace]` e tudo de `workspace.*` e
    `profile.*` da raiz, com os `path` das dependências relativos à pasta, e no fim `extras`: os
    perfis de um pacote que a camada dele declara e que este workspace alcança (#1087)."""
    partes = [INICIO, "[workspace]", 'resolver = "3"']
    partes.append("members = [" + ", ".join(f'"{m}"' for m in membros) + "]")
    partes.append("")
    for cabecalho, texto in secoes(raiz_toml):
        if cabecalho == "workspace" or not (cabecalho.startswith("workspace.") or cabecalho.startswith("profile.")):
            continue
        if cabecalho == "workspace.dependencies":
            texto = re.sub(
                r'path = "([^"]+)"',
                lambda m: f'path = "{os.path.relpath(m.group(1), pasta)}"',
                texto,
            )
        partes.append(texto.strip("\n"))
        partes.append("")
    for texto in extras:
        partes.append(texto.strip("\n"))
        partes.append("")
    partes.append(FIM)
    return "\n".join(partes) + "\n"


def a_mao(manifesto):
    """A parte de um manifesto de workspace escrita à mão: tudo antes do bloco gerado."""
    return manifesto.split(INICIO.split(":")[0], 1)[0]


def perfis_de_pacote(lista):
    """[(pacote, texto da seção, pasta dona)] de cada `[profile.<perfil>.package.<pacote>]` que um
    workspace separado declara à mão: o perfil de um pacote mora na camada dele (F3, #1086)."""
    achados = []
    for pasta, _ in lista:
        manifesto = root / pasta / "Cargo.toml"
        if not manifesto.exists():
            continue
        for cabecalho, texto in secoes(a_mao(manifesto.read_text(encoding="utf-8"))):
            pacote = re.fullmatch(r'profile\.[\w-]+\.package\."?([\w-]+)"?', cabecalho)
            if pacote:
                achados.append((pacote.group(1), texto, pasta))
    return achados


def grafo_de_texto(pastas):
    """{pacote: {pacotes do repositório que o Cargo.toml dele cita como dependência}}, lido do texto:
    vale antes de o workspace existir, quando o `cargo metadata` ainda não responde."""
    grafo = {}
    for nome, lugar in pastas.items():
        texto = (root / lugar / "Cargo.toml").read_text(encoding="utf-8")
        grafo[nome] = {
            d for d in re.findall(r'^\s*"?([\w-]+)"?\s*(?:\.workspace\s*=|=)', texto, re.M)
            if d in pastas and d != nome
        }
    return grafo


def alcance(grafo, inicio):
    """`inicio` e tudo o que ele alcança no `grafo`, a qualquer profundidade."""
    vistos, fila = set(), list(inicio)
    while fila:
        nome = fila.pop()
        if nome not in vistos:
            vistos.add(nome)
            fila.extend(grafo.get(nome, ()))
    return vistos


def com_bloco(manifesto, novo):
    """O manifesto com o bloco gerado trocado (ou posto no fim, se ainda não tem)."""
    if INICIO.split(":")[0] in manifesto:
        antes = manifesto.split(INICIO.split(":")[0], 1)[0]
        depois = manifesto.split(FIM, 1)[1].lstrip("\n") if FIM in manifesto else ""
        return antes + novo + depois
    return manifesto.rstrip("\n") + "\n\n" + novo


def versoes(lock):
    """{crate externo: {versões}} de um Cargo.lock (só os de registro ou git, que têm `source`)."""
    resultado = {}
    for nome, versao in re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\nsource = ', lock):
        resultado.setdefault(nome, set()).add(versao)
    return resultado


def serie(versao):
    """A série semver de uma versão, onde o Cargo escolhe uma só: `1` para 1.x, `0.10` para 0.10.x."""
    partes = versao.split("+")[0].split("-")[0].split(".")
    if partes[0] != "0":
        return partes[0]
    return ".".join(partes[:2]) if partes[1] != "0" else ".".join(partes[:3])


def divergencias(locks):
    """`locks`: {pasta: {crate: {versões}}}. Dois workspaces que usam a mesma série de um crate
    externo (serde 1.x, sha2 0.10.x) precisam resolver a mesma versão: senão as duas pastas
    compilam código diferente com o mesmo nome. Séries diferentes (sha2 0.10 e 0.11) são legítimas."""
    problemas = []
    pastas = sorted(locks)
    for i, a in enumerate(pastas):
        for b in pastas[i + 1:]:
            for crate in sorted(set(locks[a]) & set(locks[b])):
                por_serie_b = {serie(v): v for v in locks[b][crate]}
                for va in sorted(locks[a][crate]):
                    vb = por_serie_b.get(serie(va))
                    if vb and vb != va:
                        problemas.append(
                            f"{crate}: {a}/Cargo.lock resolve {va} e {b}/Cargo.lock resolve {vb}; alinhe com "
                            f"`cargo update -p {crate}@{vb} --precise {va}` dentro de {b}, ou o contrário"
                        )
    return problemas


def deps(camadas, escrever):
    raiz_toml = (root / "Cargo.toml").read_text(encoding="utf-8")
    problemas = []
    pastas = pastas_dos_pacotes()
    lista = separados(camadas, pastas)
    # A raiz: só o que não saiu, e nenhum crate de camada separada como membro dela.
    membros = membros_da_raiz(camadas, pastas)
    fora_de_crates = any(m.startswith("../") for _, ms in lista for m in ms)
    # `crates/ravenrs-perf` é um app (um workspace dele); `crates` inteiro o cobre.
    excluidos = sorted({"examples/contracts", "crates" if fora_de_crates else "crates/ravenrs-perf"})
    esperada = raiz_com_membros(raiz_toml, membros, excluidos)
    if esperada != raiz_toml:
        if escrever:
            (root / "Cargo.toml").write_text(esperada, encoding="utf-8")
            raiz_toml = esperada
            print("Cargo.toml: members e exclude da raiz escritos")
        else:
            problemas.append("Cargo.toml: members ou exclude da raiz não batem com camadas.toml; rode `python3 scripts/camadas.py deps --write`")
    perfis, grafo = perfis_de_pacote(lista), grafo_de_texto(pastas)
    for pasta, membros in lista:
        manifesto = root / pasta / "Cargo.toml"
        if not manifesto.exists() and escrever:
            manifesto.parent.mkdir(parents=True, exist_ok=True)
            manifesto.write_text(
                f"# A camada {pasta.split('/')[-1]} do RavenRS, um workspace Cargo (camadas.toml, #1086).\n"
                "# Os membros ficam em crates/; abra este workspace para trabalhar nela.\n",
                encoding="utf-8",
            )
        for membro in (m for m in membros if m.startswith("../")):
            arquivo = root / os.path.normpath(os.path.join(pasta, membro)) / "Cargo.toml"
            texto = arquivo.read_text(encoding="utf-8")
            certo = com_workspace(texto, os.path.relpath(pasta, os.path.normpath(os.path.join(pasta, membro))))
            if certo != texto:
                if escrever:
                    arquivo.write_text(certo, encoding="utf-8")
                else:
                    problemas.append(f"{arquivo.relative_to(root)}: falta `workspace = ...` apontando para {pasta}; rode `python3 scripts/camadas.py deps --write`")
        atual = manifesto.read_text(encoding="utf-8") if manifesto.exists() else ""
        alcancados = alcance(grafo, pacotes_de(camadas, pasta, pastas))
        extras = [texto for pacote, texto, dona in perfis if dona != pasta and pacote in alcancados]
        esperado = com_bloco(atual, bloco(raiz_toml, pasta, membros, extras))
        if atual != esperado:
            if escrever:
                manifesto.write_text(esperado, encoding="utf-8")
                print(f"{pasta}/Cargo.toml: cópia da raiz escrita")
            else:
                problemas.append(
                    f"{pasta}/Cargo.toml: a cópia do Cargo.toml da raiz está velha ou falta; rode "
                    "`python3 scripts/camadas.py deps --write`"
                )
        lock = root / pasta / "Cargo.lock"
        if escrever and not lock.exists():
            # Partindo do lock da raiz, o Cargo só tira o que a pasta não usa: as versões são as mesmas.
            shutil.copyfile(root / "Cargo.lock", lock)
            metadados(root / pasta)
            subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=root / pasta, check=True, capture_output=True)
            print(f"{pasta}/Cargo.lock: criado a partir do da raiz")
        elif not lock.exists():
            problemas.append(f"{pasta}/Cargo.lock falta; rode `python3 scripts/camadas.py deps --write`")
    locks = {".": versoes((root / "Cargo.lock").read_text(encoding="utf-8"))}
    for pasta in workspaces(camadas):
        lock = root / pasta / "Cargo.lock"
        if lock.exists():
            locks[pasta] = versoes(lock.read_text(encoding="utf-8"))
    problemas.extend(divergencias(locks))
    return problemas


def mudados(camadas):
    base = os.environ.get("CHECK_BASE", "origin/master")
    git = lambda *a: subprocess.run(["git", *a], cwd=root, check=True, capture_output=True, text=True).stdout
    fork = git("merge-base", "HEAD", base).strip()
    arquivos = set(git("diff", "--name-only", fork).split("\n")) | set(git("ls-files", "--others", "--exclude-standard").split("\n"))
    donas = {pasta: pastas_do_workspace(pasta, membros) for pasta, membros in separados(camadas)}
    return workspaces_de([f for f in arquivos if f], donas)


def workspaces_de(arquivos, donas):
    """As pastas de workspace que algum arquivo toca, na ordem de `donas` ({workspace: [pastas
    dele]}). Um arquivo vai para o workspace da pasta mais funda que o contém."""
    pares = [(lugar, pasta) for pasta, lugares in donas.items() for lugar in lugares]
    tocadas = set()
    for arquivo in arquivos:
        contem = [(lugar, pasta) for lugar, pasta in pares if arquivo.startswith(lugar + "/")]
        if contem:
            tocadas.add(max(contem, key=lambda par: len(par[0]))[1])
    return [p for p in donas if p in tocadas]


def publicaveis(pastas):
    """Os pacotes que o crates.io recebe: os que não dizem `publish = false`."""
    return {
        nome for nome, lugar in pastas.items()
        if not re.search(r"^publish\s*=\s*false", (root / lugar / "Cargo.toml").read_text(encoding="utf-8"), re.M)
    }


def publicacao(camadas, pastas, recebe):
    """Os comandos de publicação, um por workspace com algum pacote em `recebe`, na ordem das camadas.
    Dentro de um workspace, o `cargo publish --workspace` acha a ordem sozinho e pula o que diz
    `publish = false`."""
    comandos = []
    for nome, campos in camadas:
        if campos.get("separada") and campos.get("um_workspace_por_pacote"):
            donos = [(pastas[p], [p]) for p in campos.get("pacotes", [])]
        elif campos.get("separada"):
            donos = [(campos["pasta"], pacotes_de(camadas, campos["pasta"], pastas))]
        else:
            donos = [(".", campos.get("pacotes", []))]
        for pasta, nomes in donos:
            if not any(n in recebe for n in nomes):
                continue
            alvo = "" if pasta == "." else f" --manifest-path {pasta}/Cargo.toml"
            comandos.append(f"cargo publish --workspace{alvo}")
    return comandos


# --- autoteste -----------------------------------------------------------------------------------


def autoteste():
    camadas = ler_camadas(
        '# comentário\n[baixo]\nrotulo = "W1" # fim de linha\nraiz = true\npacotes = [\n  "a", "b",\n]\n'
        '[alto]\nrotulo = "W2"\nseparada = true\num_workspace_por_pacote = true\npacotes = ["c"]\n'
        '[ci]\nseparada = true\npasta = "tools/ci"\npacotes = ["ci", "ci-core"]\n'
    )
    assert [nome for nome, _ in camadas] == ["baixo", "alto", "ci"], camadas
    assert camadas[0][1] == {"rotulo": "W1", "raiz": True, "pacotes": ["a", "b"]}, camadas
    assert camadas[1][1]["um_workspace_por_pacote"] is True, camadas
    assert conferir(camadas[:2], {"a": [], "b": [("a", "normal")], "c": [("b", "de teste")]}) == []
    # A aresta plantada: um pacote de baixo que testa com um de cima.
    plantada = conferir(camadas[:2], {"a": [("c", "de teste")], "b": [], "c": []})
    assert len(plantada) == 1 and "a (W1 baixo) usa c (W2 alto) como dependência de teste" in plantada[0], plantada
    faltando = conferir(camadas[:2], {"a": [], "b": [], "c": [], "d": []})
    assert faltando == ["d não está em camada nenhuma: ponha-o numa seção de camadas.toml"], faltando
    sobrando = conferir(camadas[:2], {"a": [], "b": []})
    assert sobrando == ["camadas.toml: c não é pacote do repositório"], sobrando

    pastas = {"a": "crates/a", "b": "crates/b", "c": "examples/c", "ci": "tools/ci", "ci-core": "tools/ci/crates/ci-core"}
    assert separados(camadas, pastas) == [("examples/c", []), ("tools/ci", ["crates/ci-core"])], separados(camadas, pastas)

    raiz = (
        '[workspace]\nmembers = ["crates/*"]\n\n[workspace.package]\nversion = "1.0.0"\n\n'
        '[workspace.dependencies]\na = { version = "1.0.0", path = "crates/a" }\nserde = "1"\n\n'
        '[workspace.lints.rust]\nunsafe_code = "forbid"\n\n[profile.dev]\ndebug = 0\n'
    )
    gerado = bloco(raiz, "examples/c", [])
    assert 'a = { version = "1.0.0", path = "../../crates/a" }' in gerado, gerado
    assert "[workspace.lints.rust]" in gerado and "[profile.dev]" in gerado and 'members = []' in gerado, gerado
    assert gerado.count("[workspace]") == 1, gerado
    app = '[package]\nname = "c"\n\n[lints]\nworkspace = true\n'
    uma_vez = com_bloco(app, gerado)
    assert uma_vez.startswith(app.rstrip("\n")) and uma_vez.endswith(FIM + "\n"), uma_vez
    # Escrever de novo troca o bloco, não acrescenta outro.
    nova_raiz = raiz.replace('version = "1.0.0"\n\n[workspace.dep', 'version = "1.0.1"\n\n[workspace.dep')
    duas_vezes = com_bloco(uma_vez, bloco(nova_raiz, "examples/c", []))
    assert duas_vezes.count(FIM) == 1 and 'version = "1.0.1"' in duas_vezes, duas_vezes

    lock = '[[package]]\nname = "serde"\nversion = "1.0.1"\nsource = "registry+x"\n\n[[package]]\nname = "a"\nversion = "1.0.0"\n'
    assert versoes(lock) == {"serde": {"1.0.1"}}, versoes(lock)
    # O caso de recusa: o mesmo crate externo em versões sem nada em comum.
    divergente = divergencias({".": {"serde": {"1.0.1"}, "rand": {"0.8.5", "0.9.0"}}, "examples/c": {"serde": {"1.0.2"}, "rand": {"0.9.0"}}})
    assert len(divergente) == 1 and divergente[0].startswith("serde: ./Cargo.lock resolve 1.0.1 e examples/c/Cargo.lock resolve 1.0.2"), divergente
    # Séries diferentes do mesmo crate não divergem: sha2 0.10 num, 0.11 noutro.
    assert divergencias({".": {"sha2": {"0.11.1"}}, "examples/c": {"sha2": {"0.10.9"}}}) == []
    assert [serie(v) for v in ("1.2.3", "0.10.7", "0.0.4", "2.0.0-rc.1")] == ["1", "0.10", "0.0.4", "2"]

    assert pacotes_de(camadas, "tools/ci", pastas) == ["ci", "ci-core"], pacotes_de(camadas, "tools/ci", pastas)
    donas = {"examples/c": ["examples/c"], "tools/ci": ["tools/ci", "tools/ci/crates/ci-core"]}
    assert workspaces_de(["tools/ci/crates/x/src/lib.rs", "crates/a/x.rs"], donas) == ["tools/ci"]
    assert workspaces_de(["examples/c/Cargo.toml", "examples/cd/x"], donas) == ["examples/c"]

    # Uma camada da biblioteca: o workspace em camadas/, os membros em crates/ (F3, #1086).
    biblioteca = ler_camadas('[base]\nseparada = true\npasta = "camadas/base"\npacotes = ["a", "b"]\n[topo]\npacotes = ["x"]\n')
    lugares = {"a": "crates/a", "b": "crates/b", "x": "crates/x"}
    assert separados(biblioteca, lugares) == [("camadas/base", ["../../crates/a", "../../crates/b"])], separados(biblioteca, lugares)
    assert pacotes_de(biblioteca, "camadas/base", lugares) == ["a", "b"]
    assert membros_da_raiz(biblioteca, lugares) == ["crates/x"]
    assert workspaces_de(["crates/b/src/lib.rs"], {"camadas/base": pastas_do_workspace("camadas/base", ["../../crates/a", "../../crates/b"])}) == ["camadas/base"]
    assert workspaces_de(["crates/x/src/lib.rs"], {"camadas/base": ["camadas/base", "crates/a"]}) == []
    membro = '[package]\nname = "a"\nversion.workspace = true\n'
    assert com_workspace(membro, "../../camadas/base") == '[package]\nname = "a"\nworkspace = "../../camadas/base"\nversion.workspace = true\n'
    assert com_workspace(com_workspace(membro, "../../camadas/x"), "../../camadas/base") == com_workspace(membro, "../../camadas/base")
    raiz_nova = raiz_com_membros(raiz, ["crates/x", "xtask"], ["crates"])
    assert 'members = [\n    "crates/x",\n    "xtask",\n]' in raiz_nova and "[workspace.package]" in raiz_nova, raiz_nova
    # A raiz: só a camada `raiz = true` fica fora de `separada` (F4, #1087).
    raiz_ok = ler_camadas('[base]\nseparada = true\npasta = "camadas/base"\npacotes = ["a"]\n[infra]\nraiz = true\npacotes = ["x"]\n')
    assert conferir(raiz_ok, {"a": [], "x": [("a", "normal")]}) == []
    duas = ler_camadas('[base]\npacotes = ["a"]\n[infra]\nraiz = true\npacotes = ["x"]\n')
    assert any("só a camada com `raiz = true`" in p for p in conferir(duas, {"a": [], "x": []}))
    lugares_pub = {"a": "crates/a", "x": "crates/x", "app": "examples/app"}
    com_app = ler_camadas('[base]\nseparada = true\npasta = "camadas/base"\npacotes = ["a"]\n[infra]\nraiz = true\npacotes = ["x"]\n[apps]\nseparada = true\num_workspace_por_pacote = true\npacotes = ["app"]\n')
    assert publicacao(com_app, lugares_pub, {"a", "x"}) == [
        "cargo publish --workspace --manifest-path camadas/base/Cargo.toml", "cargo publish --workspace",
    ], publicacao(com_app, lugares_pub, {"a", "x"})
    # Um workspace sem pacote publicável (um app) não entra.
    assert publicacao(com_app, lugares_pub, {"x"}) == ["cargo publish --workspace"]
    # O perfil de um pacote vai só para quem o alcança (F4, #1087).
    assert alcance({"a": {"b"}, "b": {"c"}, "x": set()}, ["a"]) == {"a", "b", "c"}
    com_extra = bloco(raiz, "camadas/x", [], ["[profile.dev.package.c]\nopt-level = 1\n"])
    assert com_extra.index("[profile.dev.package.c]") > com_extra.index("[profile.dev]"), com_extra
    # Um pacote leve e solto numa camada cara gera aviso; um que usa a camada ou é usado nela
    # (pesado, usa), ou numa camada barata, não (F5, #1096).
    pesadas = ler_camadas('[base]\npacotes = ["a"]\n[cara]\npacotes = ["leve", "pesado", "usa"]\n')
    deps_pesadas = {"a": [], "leve": [("a", "normal")], "pesado": [], "usa": [("pesado", "normal")]}
    avisos = leves_em_camada_cara(pesadas, deps_pesadas, {"cara": ({"leve": 50, "pesado": 300, "usa": 20}, 320), "base": ({"a": 5}, 5)})
    assert len(avisos) == 1 and avisos[0].startswith("aviso: leve compila 50 crates, está solto em cara, que compila 320"), avisos
    assert leves_em_camada_cara(pesadas, deps_pesadas, {"cara": ({"leve": 50}, 150)}) == []
    print("camadas.py: autoteste passou")


# --- pesos ---------------------------------------------------------------------------------------

# Uma camada é cara a partir deste total de crates, e um pacote é leve nela quando usa no máximo
# um terço disso (F5, #1096).
CAMADA_CARA = 250


def pesos_do_workspace(pasta):
    """{membro: crates que ele compila (dependências normais e de build)} e o total do workspace,
    com as de teste, tirados do `cargo metadata` com o Cargo.lock."""
    dados = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--offline", "--all-features"],
        cwd=pasta, check=True, capture_output=True, text=True,
    ).stdout)
    nomes = {p["id"]: p["name"] for p in dados["packages"]}
    sem_teste, com_teste = {}, {}
    for no in dados["resolve"]["nodes"]:
        sem_teste[no["id"]] = {d["pkg"] for d in no["deps"] if any(k["kind"] in (None, "build") for k in d["dep_kinds"])}
        com_teste[no["id"]] = {d["pkg"] for d in no["deps"]}

    def fecho(grafo, inicio):
        vistos, pilha = set(), list(inicio)
        while pilha:
            atual = pilha.pop()
            if atual not in vistos:
                vistos.add(atual)
                pilha.extend(grafo.get(atual, ()))
        return vistos

    membros = dados["workspace_members"]
    por_membro = {nomes[m]: len(fecho(sem_teste, [m])) for m in membros}
    total = len(fecho(sem_teste, membros) | {d for m in membros for d in fecho(sem_teste, com_teste[m])})
    return por_membro, total


def leves_em_camada_cara(camadas, pacotes, pesos):
    """Avisos: numa camada cara, um pacote solto nela (não usa nada da camada, e nada dela o usa) que
    compila no máximo um terço dela. Ele desce para uma camada perto do que usa. `pesos`: {camada: ({pacote: crates}, total)}."""
    nivel = {pacote: camada for camada, campos in camadas for pacote in campos.get("pacotes", [])}
    avisos = []
    for camada, (por_pacote, total) in pesos.items():
        if total < CAMADA_CARA:
            continue
        for pacote, crates in sorted(por_pacote.items()):
            usa = [d for d, _ in pacotes.get(pacote, []) if nivel.get(d) == camada]
            usado = [p for p, deps in pacotes.items() if nivel.get(p) == camada and pacote in {d for d, _ in deps}]
            if not usa and not usado and crates * 3 <= total:
                avisos.append(
                    f"aviso: {pacote} compila {crates} crates, está solto em {camada}, que compila "
                    f"{total}; uma camada perto do que ele usa deixa o PR dele mais barato (#1096)"
                )
    return avisos


# --- comando -------------------------------------------------------------------------------------


def main():
    args = sys.argv[1:]
    if "--self-test" in args:
        autoteste()
        return
    comando = args[0] if args else "check"
    camadas = camadas_do_repositorio()
    if comando == "check":
        pacotes = pacotes_do_repositorio(camadas)
        problemas = conferir(camadas, pacotes)
        if problemas:
            sys.exit("\n".join(problemas))
        print(f"{len(pacotes)} pacotes em {len(camadas)} camadas, nenhuma dependência sobe de camada")
    elif comando == "deps":
        problemas = deps(camadas, "--write" in args)
        if problemas:
            sys.exit("\n".join(problemas))
        print(f"{len(workspaces(camadas))} workspaces separados em dia com a raiz, sem versão divergente")
    elif comando == "workspaces":
        pastas = pastas_dos_pacotes()
        for pasta in mudados(camadas) if "--mudados" in args else workspaces(camadas):
            print(pasta, *pacotes_de(camadas, pasta, pastas))
    elif comando == "publicacao":
        pastas = pastas_dos_pacotes()
        for linha in publicacao(camadas, pastas, publicaveis(pastas)):
            print(linha)
    elif comando == "pesos":
        pacotes = pacotes_do_repositorio(camadas)
        pesos = {}
        for camada, campos in camadas:
            if str(campos.get("pasta", "")).startswith("camadas/"):
                pesos[camada] = pesos_do_workspace(root / campos["pasta"])
                print(f"{campos.get('rotulo', '')} {camada}: {pesos[camada][1]} crates")
        for aviso in leves_em_camada_cara(camadas, pacotes, pesos):
            print(aviso)
    else:
        sys.exit(f"camadas.py: comando desconhecido {comando}; use check, deps, workspaces, publicacao ou pesos")


main()
