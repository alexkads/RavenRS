---
name: ravenrs-pdfcraft
description: Como o ravenrs-pdf (motor de PDF - ler, desenhar em pixels, juntar, dividir, anexos) veio do PdfCraft (storytold/pdfcraft), junto com os forks remendados de hayro, vello_cpu e lopdf, e como trazer uma revisão nova sem perder o que é nosso - conversão com scripts/storytold, mescla em três vias, licenças, mudanças locais e testes. Use sempre que for atualizar, sincronizar ou corrigir o ravenrs-pdf, comparar com o PdfCraft, trazer mais crates dele (annot, forms, edit, sign), mexer em src/third_party ou nas licenças de LICENSES/, ou construir um PdfViewer em cima dele, mesmo que o pedido só diga "puxar as novidades do PDF".
---

# ravenrs-pdf e o PdfCraft

O `crates/ravenrs-pdf` é uma cópia convertida do motor sem tela do **PdfCraft**
(https://github.com/storytold/pdfcraft, MIT OR Apache-2.0). O commit de origem está no `NOTICE`.
Os nomes e logos ArtCraft e PdfCraft são marcas: aparecem só no `NOTICE` e como crédito, nunca em
API, docs de produto, ícones ou telas.

## O que veio

| origem | módulo nosso | o que é |
|---|---|---|
| `crates/geom`, `model`, `content` | `geom`, `model`, `content` | geometria, páginas, operadores |
| `crates/crypt` | `crypt` | RC4/AES, senhas, permissões |
| `crates/filters` | `filters` | Flate, LZW, ASCII85, CCITT, JBIG2 |
| `crates/cos` | `cos` | objetos, reparo de xref, gravação, criptografia |
| `crates/fonts` | `fonts` | métricas, codificações, shaping; `build.rs` nosso é o deles |
| `crates/render` | `render` | página → pixels (`PageRenderer`), texto, inspeção |
| `crates/organize` | `organize` | juntar (`combine`), dividir, páginas, anexos, metadados |
| `crates/oracle`, `testkit` | `testing::oracle`, `testing::testkit` | só com a feature `test-support` |
| `vendor/hayro*`, `vello_cpu`, `vello_common`, `lopdf` | `third_party::*` | forks remendados que o render usa |

**Por que `third_party` existe:** o PdfCraft desenha com hayro 0.7, vello 0.0.8 e lopdf 0.45,
remendados em `vendor/` (cerca de mil marcas `PdfCraft patch:`) e ligados por
`[patch.crates-io]`. Um `[patch]` não chega a quem usa um crate publicado, e as versões novas do
crates.io não têm os remendos. Então esses crates também são convertidos, do mesmo commit, e a
mescla os atualiza junto. Quando o PdfCraft largar um deles (o remendo entrou no crates.io), tire-o
de `projects.json`, de `third_party/mod.rs` e de `LICENSES/`, e ponha a dependência no `Cargo.toml`.

Ficaram de fora: `edit`, `annot`, `forms`, `sign` (aws-lc-rs, C), `js` (boa_engine), `ocr` (rten),
`xfa`, `redact`, `optimize`, `a11y`, `audit`, `measure`, `compare`, `preflight`, `print`
(`craft-print`, `craft-cms`), `export`, `prc`, `automation`, `engine` (`craft-docpath`) e
`ui-egui`. Para trazer um deles, acrescente em `projects.json` e confira as dependências: um
`craft-*` (de `storytold/craft-libs`, só por git) entra copiado como módulo, com crédito no
`NOTICE`; nunca como dependência git.

## Como a conversão funciona

`scripts/storytold/convert.py pdfcraft <clone> <commit> <saída>`. Além das regras comuns (ver a
docstring do script e a skill `ravenrs-wordcraft`), este projeto usa:

- `"precise"`: `hayro`, `lopdf` e `pdfcraft_x` só são trocados onde são caminho ou `use`; em
  exemplos de documentação viram `ravenrs_pdf::x`.
- `"features"`: as de cada crate de `vendor/`, como o PdfCraft as resolve (`cargo metadata` no
  workspace deles). Ligadas somem do `cfg`, desligadas viram `any()`, e as condições que sobram são
  dobradas. Se o PdfCraft passar a ligar outra, mude aqui.
- `"unit_tests": false` nos de `vendor/`: os testes próprios deles, que o PdfCraft nunca roda
  (`vendor/` fica fora do workspace), saem do build, e os exemplos de doc viram `ignore`.
- `"replace"`: `skrifa::` → `skrifa_047::` no `hayro_interpret` (ele usa skrifa 0.47; o resto,
  0.44), e o `#![allow(unsafe_code)]` do `hayro_syntax`, que tem o único `transmute`.
- Arquivos fora dos crates (fontes Inter, PFB da Foxit, CMaps, ICC) vão para `assets/<caminho
  deles>`, e `include_bytes!` e `concat!(env!("CARGO_MANIFEST_DIR"), …)` apontam para lá.
- Macros `#[macro_export]` (`dictionary!`, `or_skip!`) ficam na raiz do crate; o `mod.rs` do
  módulo as reexporta.

## O que é nosso

Rodar o conversor no commit do `NOTICE` reproduz `src/`, `tests/` e `assets/`, exceto:

- `src/lib.rs`: a raiz, com o exemplo testado (abrir, desenhar, juntar).
- `src/testing/mod.rs` e `src/third_party/mod.rs`: as raízes dos grupos. Em `third_party`, os
  lints deles (`clippy::all`, `cognitive_complexity`, imports sem uso), que nunca passaram pelas
  regras do PdfCraft.
- `build.rs` (o de `crates/fonts`, com o caminho do módulo), `Cargo.toml`, `NOTICE`, `LICENSES/`,
  `README.md`, `clippy.toml` (o deles sem as regras do egui), `rustfmt.toml`.
- No `Cargo.toml` da raiz: `[profile.dev.package.ravenrs-pdf] opt-level = 1`. Em `opt-level = 0`
  os testes de arquivos hostis estouram a pilha, e o de criptografia leva minutos.

O `Cargo.toml` do crate segue as versões e features da origem. Ele foi gerado do `cargo metadata`
do workspace deles. Ao trazer uma revisão, refaça essa conferência (passo 3 abaixo).

## Trazer uma revisão nova

```bash
S=<scratchpad>
git clone --filter=blob:none https://github.com/storytold/pdfcraft.git $S/pdfcraft
OLD=$(grep -o '[0-9a-f]\{40\}' crates/ravenrs-pdf/NOTICE)
NEW=$(git -C $S/pdfcraft rev-parse origin/main)
scripts/storytold/convert.py pdfcraft $S/pdfcraft $OLD $S/old
scripts/storytold/convert.py pdfcraft $S/pdfcraft $NEW $S/new
for d in src tests assets; do diff -rq $S/old/$d crates/ravenrs-pdf/$d; done   # só o que é nosso
scripts/storytold/merge.py pdfcraft $S/old $S/new
```

1. O `diff` antes da mescla confere a conversão: fora da lista acima, algo mudou no conversor ou
   foi editado à mão. Resolva antes de mesclar.
2. O `merge.py` faz a mescla em três vias por arquivo e lista conflitos (sai com 1 se houver).
3. Dependências: `git -C $S/pdfcraft diff $OLD $NEW -- Cargo.toml crates/{geom,crypt,filters,cos,model,content,fonts,render,organize}/Cargo.toml vendor/*/Cargo.toml`.
   Versão nova ou feature nova vai para o nosso `Cargo.toml`. Crate novo em `vendor/` usado pelo
   render: acrescente em `projects.json`, `third_party/mod.rs`, `LICENSES/` e `NOTICE`. Mudou o
   que o `[patch.crates-io]` deles aponta? Siga o patch, nunca o crates.io.
4. Features dos crates de `vendor/`: num clone de rascunho, deixe no `Cargo.toml` da raiz deles só
   os membros do recorte (sem egui, winit e `craft-*`) e rode `cargo metadata`. Os `features` de
   cada nó resolvido são os valores de `projects.json`.
5. Atualize o commit no `NOTICE`.

## Testar

```bash
CARGO_TARGET_DIR=<scratchpad>/target cargo test -p ravenrs-pdf --no-fail-fast
cargo clippy -p ravenrs-pdf --all-targets -- -D warnings
cargo check -p ravenrs-pdf --target wasm32-unknown-unknown
cargo tree -p ravenrs-pdf | grep -c 'git+'   # tem de dar 0
scripts/check.sh
```

Os testes que o PdfCraft roda nesses crates têm de passar todos: 1.750 no `175992b`, contra 1.781
aqui, porque os doctests e os testes de `testing` também contam. Para comparar os nomes, rode na
origem `cargo test -p pdfcraft-{geom,crypt,filters,cos,model,content,fonts,render,organize}` no
mesmo workspace de rascunho do passo 4. Não há corpus: os testes que leem `corpus/pdfjs` se
dispensam sozinhos quando ele falta.

No wasm32 o crate compila com avisos de código que só o desktop usa (o pool de threads do render),
como na origem. Por isso ele não está na lista de clippy wasm32 do `check.sh --full`.
