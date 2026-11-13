---
name: ravenrs-wordcraft
description: Como o ravenrs-word (motor de documentos de texto, DOCX/DOC/RTF/ODT/HTML/Markdown) veio do WordCraft (storytold/wordcraft) e como trazer uma revisão nova sem perder o que é nosso - conversão com scripts/storytold, mescla em três vias, mudanças locais e testes. Use sempre que for atualizar, sincronizar ou corrigir o ravenrs-word, comparar com o WordCraft, trazer mais crates dele (layout, render, pdf) ou mexer em scripts/storytold, mesmo que o pedido só diga "puxar as novidades do Word" ou "docx".
---

# ravenrs-word e o WordCraft

O `crates/ravenrs-word` é uma cópia convertida do motor sem tela do **WordCraft**
(https://github.com/storytold/wordcraft, MIT OR Apache-2.0). O commit de origem está no `NOTICE`.
Os nomes e logos ArtCraft e WordCraft são marcas: aparecem só no `NOTICE` e como crédito, nunca em
API, docs de produto, ícones ou telas.

## O que veio e o que ficou

| crate deles | módulo nosso | o que é |
|---|---|---|
| `crates/geom` | `geom` | medidas, pontos, cores; a feature `image` deles fica sempre ligada |
| `crates/doc` | `doc` | o modelo do documento |
| `crates/metafile` | `metafile` | WMF/EMF |
| `crates/docx` | `docx` | DOCX, com senha |
| `crates/docbin` | `docbin` | `.doc` 97-2003, só leitura |
| `crates/formats` | `formats` | TXT, Markdown, HTML/MHT, RTF, ODT, LaTeX, células de planilha |

Ficaram de fora porque dependem de `storytold/craft-libs` por git, ou não são motor: `layout` e
`engine` (`craft-proof`, `craft-font-store`, `craft-font-catalog`), `pdf` (`craft-pdf` e um `krilla`
remendado por `[patch]`), `render`, `fonts`, `mcp`, `zotero` e `ui-egui`. Para trazer algum deles,
os `craft-*` que ele usa entram copiados como módulos, com crédito no `NOTICE`; nunca como
dependência git, que impediria publicar no crates.io.

## Como a conversão funciona

`scripts/storytold/convert.py wordcraft <clone> <commit> <saída>` lê os crates listados em
`scripts/storytold/projects.json` direto do git (um clone parcial basta) e:

- `crates/X/src/**` → `src/X/**`, `lib.rs` → `mod.rs`;
- `crates/X/tests/it/**` → `tests/X_it/**`; `crates/X/tests/Y.rs` → `tests/X_Y.rs`;
- `crate::` → `crate::X::`; `wordcraft_X` → `crate::X` em `src/` e `ravenrs_word::X` em `tests/`;
- `feature = "..."` conforme `features` do projeto: ligada some do `cfg`, desligada vira `any()`;
- `include_bytes!`/`include_str!` relativos apontam para onde o arquivo foi parar (em `assets/` se
  vier de fora dos crates);
- formata com o `rustfmt.toml` do crate, que é o deles (`max_width = 150`).

O mesmo script serve ao GridCraft e ao pdfcraft; cada um é um perfil no `projects.json`.

## O que é nosso

Rodar o conversor no commit do `NOTICE` reproduz `src/` e `tests/`, exceto:

- `src/lib.rs`: a raiz do crate, com a documentação e o exemplo de ida e volta.
- `src/doc/mod.rs`: `test_log` é `pub(crate)`, e `src/docx/test_log.rs` e
  `src/formats/test_log.rs` só reexportam o do `doc`. Cada crate deles tinha o seu logger de
  teste; num binário só, o `log` aceita um logger e os outros nunca recebiam as linhas.
- **zip 9**, como no `ravenrs-sheet`: o WordCraft está no zip 3, que tem avisos de segurança
  (RUSTSEC-2026-0367, 0368, 0369 e 0429), e o `cargo deny` do `check.sh --full` recusa. No 9, o
  nome de uma entrada é um `Result`: no código, a entrada de nome ilegível é pulada
  (`docx/package.rs`, `formats/odt/crypt.rs`, `formats/odt/import.rs`); nos testes, `unwrap`
  (os arquivos com a nota `RavenRS: zip 9` no topo). O teste
  `pictures_over_the_budget_are_not_inflated` mudou: o zip 9 para uma entrada no tamanho que ela
  declara, então a "mentirosa" não gasta o orçamento. Numa mescla, conflito nessas linhas é isso:
  aplique o mesmo ajuste à linha nova deles.
- `Cargo.toml`, `NOTICE`, `README.md`, `clippy.toml` (o deles sem as regras do egui), `rustfmt.toml`.

Toda mudança nossa dentro de `src/` leva um comentário `RavenRS:`. Ao acrescentar uma, liste-a aqui.

O `Cargo.toml` segue as versões deles, salvo o `zip` (acima). Trocar uma versão vira mudança local
em vários arquivos e conflito a cada sincronização: só com motivo como esse.

## Trazer uma revisão nova

```bash
S=<scratchpad>
git clone --filter=blob:none https://github.com/storytold/wordcraft.git $S/wordcraft
OLD=$(grep -o '[0-9a-f]\{40\}' crates/ravenrs-word/NOTICE)
NEW=$(git -C $S/wordcraft rev-parse origin/main)
scripts/storytold/convert.py wordcraft $S/wordcraft $OLD $S/old
scripts/storytold/convert.py wordcraft $S/wordcraft $NEW $S/new
diff -r $S/old/src crates/ravenrs-word/src   # só as mudanças da seção acima
scripts/storytold/merge.py wordcraft $S/old $S/new
```

1. O `diff` antes da mescla confere a conversão: se aparecer algo fora da lista "O que é nosso", o
   conversor ou a lista estão desatualizados. Conserte isso antes de mesclar.
2. O `merge.py` pega o deles onde não mexemos, mantém o nosso onde eles não mexeram, faz
   `git merge-file` onde os dois mexeram e lista os conflitos (sai com 1 se houver).
3. Confira se o `Cargo.toml` de cada crate deles ganhou dependência: `git -C $S/wordcraft diff
   $OLD $NEW -- crates/{geom,doc,metafile,docx,docbin,formats}/Cargo.toml Cargo.toml`. Dependência
   nova de `craft-*` ou com `git =` não pode entrar como está: copie o crate como módulo.
4. Crate novo usado pelos seis, ou módulo novo deles: acrescente em `projects.json` e em `lib.rs`.
5. Atualize o commit no `NOTICE`.

## Testar

```bash
CARGO_TARGET_DIR=<scratchpad>/target cargo test -p ravenrs-word
cargo clippy -p ravenrs-word --all-targets -- -D warnings
cargo check -p ravenrs-word --target wasm32-unknown-unknown
cargo tree -p ravenrs-word | grep -c 'git+'   # tem de dar 0
scripts/check.sh
```

Os testes de origem (~1.880 no `fab8793`) têm de passar todos: unitários de cada módulo,
`tests/docx_it`, `tests/docx_matrix_save_size.rs`, `tests/doc_empty_paragraph_merge.rs` e o exemplo
em `lib.rs`. Os testes de integração usam a feature `test-support` (contadores de passos para os
testes de tempo linear), ligada pela dev-dependency do crate nele mesmo.
