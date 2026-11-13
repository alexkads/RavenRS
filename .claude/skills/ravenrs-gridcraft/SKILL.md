---
name: ravenrs-gridcraft
description: Como manter o ravenrs-sheet no nível do GridCraft (storytold/gridcraft) e dos crates do craft-libs que ele usa - ver o que mudou lá, converter os crates em módulos, mesclar em três vias sem perder o que é nosso, trazer crates craft-* novos e testar. Use sempre que o pedido citar GridCraft, craft-libs, storytold, "buscar melhorias da planilha", "atualizar o motor da planilha", ravenrs-sheet desatualizado, ou quando for mexer no ravenrs-sheet e quiser saber se a mudança já existe na origem.
---

# GridCraft no RavenRS

O `ravenrs-sheet` é o motor de planilha do RavenRS. Ele não foi escrito aqui: é o GridCraft (`storytold/gridcraft`, Apache-2.0 ou MIT), com 11 crates que viraram módulos de um crate só. Desde a #311, também traz 8 crates do `craft-libs` (`storytold/craft-libs`), dos quais os formatos de arquivo dependem. O código mora aqui porque o GridCraft não está no crates.io, e uma dependência git impediria o RavenRS de publicar (`ARCHITECTURE.md`, decisão 9).

Por isso, melhorar o motor quase sempre começa lá. A origem anda todo dia, e o que ela corrige (limites contra arquivos hostis, resultados como os do Excel, desempenho) chega aqui por sincronização, não por reescrita.

## O que é de onde

| Aqui | Origem |
|---|---|
| `src/base`, `numfmt`, `formula`, `functions`, `math`, `model`, `calc`, `xlsx`, `chart`, `pdf`, `engine` | `gridcraft/crates/<nome>/src` (`base` é o `core`) |
| `tests/<m>_it/` e `tests/<m>_<arquivo>.rs` | `gridcraft/crates/<nome>/tests/it/` e `tests/<arquivo>.rs` |
| `tests/golden/` e outros dados de teste | os mesmos caminhos, porque os testes os leem por caminho relativo |
| `src/craft/{cfb,csv,offcrypto,opc,sheet,xls,xml,zip}` | `craft-libs/crates/<nome>/src`, com o `README.md` ao lado |
| `tests/craft_<nome>_*.rs`, `tests/craft_xls_common/` | `craft-libs/crates/<nome>/tests/` |
| `src/craft/media_testkit/hostile.rs` | `craft-libs/crates/media-testkit/src/hostile.rs` (só o laço de fuzz, só para testes) |
| `src/lib.rs`, `src/craft/mod.rs`, `Cargo.toml`, `NOTICE`, `README.md` | nossos |

As revisões de origem estão no `crates/ravenrs-sheet/NOTICE`. Em 13/11/2026: GridCraft `c0ee120`, craft-libs `bb57d42`, a revisão que esse GridCraft fixa.

## Ver o que mudou lá

```bash
gh api "repos/storytold/gridcraft/compare/<rev do NOTICE>...HEAD" --jq '"ahead "+(.ahead_by|tostring)+", files "+(.files|length|tostring)'
```

Para saber o que vale trazer, liste os commits por crate:

```bash
for c in core numfmt formula functions math model calc xlsx chart pdf engine; do gh api --paginate "repos/storytold/gridcraft/commits?path=crates/$c&since=<data da rev>&per_page=100" --jq '.[] | select(.parents|length==1) | "'$c' "+.sha[0:9]+" "+(.commit.message|split("\n")[0])'; done
```

Agrupe para o Alex: segurança e limites, correção de cálculo, desempenho, recursos novos. Diga também o que exigiria dependência nova. `ui-egui`, `apps`, `mcp` e `sandbox` não interessam: a tela aqui é o `ravenrs-spreadsheet`.

A nova revisão do `craft-libs` é a que o `Cargo.toml` da raiz do GridCraft fixa (`craft-cfb = { git = …, rev = … }`). Nunca use outra: os dois andam juntos.

## Trazer uma revisão nova

1. **Issue e worktree**, como todo trabalho (skill `ravenrs-github`).
2. **Clones parciais no scratchpad**, só para leitura. Nada se compila ali dentro.
   ```bash
   git clone --filter=blob:none https://github.com/storytold/gridcraft.git $SCRATCH/gridcraft
   git clone --filter=blob:none https://github.com/storytold/craft-libs.git $SCRATCH/craft-libs
   ```
3. **Converter a revisão atual e conferir o conversor.** O resultado precisa reproduzir o `ravenrs-sheet`, fora as mudanças nossas (lista abaixo). Se aparecer outra diferença, o conversor está errado: corrija-o antes de seguir.
   ```bash
   scripts/storytold/convert.py gridcraft $SCRATCH/gridcraft <rev GridCraft do NOTICE> $SCRATCH/old --with craft-libs=$SCRATCH/craft-libs@<rev craft-libs do NOTICE>
   diff -rq $SCRATCH/old crates/ravenrs-sheet | grep -v "Cargo.toml\|NOTICE\|clippy.toml\|rustfmt.toml"
   ```
4. **Converter a revisão nova e mesclar.**
   ```bash
   scripts/storytold/convert.py gridcraft $SCRATCH/gridcraft <rev nova> $SCRATCH/new --with craft-libs=$SCRATCH/craft-libs@<rev craft-libs da nova>
   scripts/storytold/merge.py gridcraft $SCRATCH/old $SCRATCH/new
   ```
   O `merge.py` faz uma mescla em três vias por arquivo: se o arquivo não mudou aqui, entra o deles; se não mudou lá, fica o nosso; se mudou dos dois lados, usa `git merge-file`. Ele também acrescenta os arquivos novos, remove os que sumiram lá e não foram mudados aqui, e lista conflitos. Resolva cada conflito lendo os dois lados: se a origem reescreveu o trecho e a nossa mudança deixou de fazer sentido, fique com o deles e registre isso no PR.
5. **Dependências.** Compare o `[dependencies]` dos crates entre as duas revisões, nos dois repositórios, e ajuste o `Cargo.toml`, que é nosso. Nada de `git =`: um crate `craft-*` novo entra como fonte do perfil `gridcraft` em `scripts/storytold/projects.json` (`"repo": "craft-libs"`, `"readme": true`) e vira módulo, e o `src/craft/mod.rs`, que é nosso, ganha o `pub mod`.
6. **`NOTICE`** com as revisões novas, e o `README` se houver recurso novo.
7. **Testar** (abaixo) e abrir o PR com a lista do que veio, por grupo.

## As mudanças nossas (o que o merge preserva)

São poucas, e cada uma tem um motivo. Ao mesclar, confira que continuam:

- `src/xlsx/xls.rs`: `#[allow(unreachable_patterns)]` no braço `w =>`. O `#[non_exhaustive]` do `craft-xls` não vale dentro do mesmo crate.
- `#![allow(clippy::disallowed_methods)]` em `tests/craft_xml_hostile.rs`, `tests/craft_cfb_writer_scale.rs` e `src/craft/media_testkit/hostile.rs`. O `clippy.toml` daqui proíbe `Instant::now` por causa do wasm32, e esses testes rodam nativos.
- `.name().unwrap()` nos testes que leem o ZIP (`src/xlsx/tests/*`, `src/engine/tests/controls.rs` e `python_files.rs`, `tests/engine_it/chart_workbook_names.rs`): no zip 9 usado aqui, `name()` devolve `Result`. Quando a origem acrescentar um teste novo com `.name().to_string()`, ponha o `.unwrap()`.
- `#[cfg_attr(debug_assertions, ignore = "a timing bound: run it with --release")]` nos testes de tempo de `src/xlsx/tests/controls_tests.rs` e `tests/math_gate.rs`: em debug eles estouram o limite sem que haja problema.

Em 13/11/2026, isso dá 19 arquivos diferentes da origem convertida, além dos que só existem aqui: `src/lib.rs`, `src/craft/mod.rs`, `src/craft/media_testkit/mod.rs`, `README.md`, `Cargo.toml` e `NOTICE`. Se a conferência do passo 3 mostrar mais que isso, algo novo divergiu: entenda o quê antes de mesclar.
- `Cargo.toml`: `md-5` 0.11 (a origem usa 0.10), para casar com o `sha1` 0.11 do resto; o `xls` mistura os dois pelo trait `Digest`.
- `Cargo.toml`: `[[test]] craft_zip_archive` com `required-features = ["test-support"]`. O teste usa uma amostra que só existe com essa feature, e o `check.sh` roda sem features. Se outro teste do `craft` passar a usar `zip64_entry_count_sample` ou o `opc` de teste, faça o mesmo com ele.
- Docs dos módulos `craft`: exemplos com `ravenrs_sheet::craft::…`, porque rodam como doctests. O conversor já faz isso.

Quando corrigir algo no motor que não seja só adaptação, avalie mandar a correção para a origem também. Cada mudança nossa é uma diferença a carregar em toda sincronização.

## O que o conversor resolve (e por quê)

O conversor e a mescla são os de todos os motores da storytold (`scripts/storytold/`, com as regras no topo do `convert.py`). O perfil `gridcraft` do `projects.json` liga, para este motor:

- `"repo": "craft-libs"` nas fontes do craft-libs, lidas na revisão dada por `--with`;
- `"test_data": "in_place"`: `tests/golden/` e outros dados que os testes leem por caminho relativo ficam onde estavam;
- `"readme": true`: o README de cada `craft` vai para o lado do módulo, com os exemplos convertidos, porque roda como doctest;
- `"only": ["src/hostile.rs"]` no `media-testkit`;
- `tests/common/` vira `tests/craft_<nome>_common/`, com `#[path]` no `mod common;`.

E, como para todos:

- `gridcraft_X::` vira `crate::X::`; dentro de um crate, `crate::` vira `crate::<módulo>::`. Nos testes de integração, vira `ravenrs_sheet::…`. Nunca substitua `$crate::` à parte: a regra de `crate::` já o cobre, e fazer as duas trocas gera `engine::engine::`.
- `rustfmt` com o `rustfmt.toml` do crate (`max_width = 150`): os caminhos mudam de tamanho, as linhas quebram em outro lugar, e sem isso tudo vira diferença.
- Clone parcial: o `git show` busca os blobs sob demanda e às vezes falha. O conversor tenta de novo.

## Testar

```bash
cargo clippy -p ravenrs-sheet --all-features --tests -- -D warnings
cargo test -p ravenrs-sheet --all-features
cargo clippy -p ravenrs-sheet -p ravenrs-spreadsheet --target wasm32-unknown-unknown -- -D warnings
cargo test -p ravenrs-spreadsheet
./scripts/check.sh
```

Os testes da origem vêm junto: mais de 2.100 unitários no motor, os de integração em `tests/` e os doctests dos módulos `craft`. Se um deles falhar, descubra se é a conversão (caminho, fixture, lint daqui) ou uma mudança nossa que a origem desfez. Não desligue o teste.

## Fora do escopo

A interface do GridCraft (`ui-egui`, egui) não entra: a tela é o `ravenrs-spreadsheet`, feita com os componentes do RavenRS. Os nomes e logos ArtCraft, GridCraft e craft-libs são marcas e não aparecem em nada que o usuário veja.
