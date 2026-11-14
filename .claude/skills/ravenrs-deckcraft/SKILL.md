---
name: ravenrs-deckcraft
description: Como o ravenrs-deck (motor de apresentações - modelo de slides, PPTX, formato nativo, desenho em pixels, exportação PDF, mídia e os comandos com desfazer) veio do DeckCraft (storytold/deckcraft), junto com os craft-libs que ele usa (metafile, pdf, media, audio, video) e o krilla remendado, e como trazer uma revisão nova sem perder o que é nosso - conversão com scripts/storytold, mescla em três vias, licenças, mudanças locais e testes. Use sempre que for atualizar, sincronizar ou corrigir o ravenrs-deck, comparar com o DeckCraft, mexer em src/craft ou src/third_party dele, construir um editor de slides em cima dele, ou quando o pedido citar pptx, PowerPoint, apresentação ou slides, mesmo que só diga "puxar as novidades do Deck".
---

# ravenrs-deck e o DeckCraft

O `crates/ravenrs-deck` é uma cópia convertida do motor sem tela do **DeckCraft**
(https://github.com/storytold/deckcraft, MIT OR Apache-2.0) e dos crates do **craft-libs**
(https://github.com/storytold/craft-libs) que ele usa por git. Os commits de origem estão no
`NOTICE`. Os nomes e logos ArtCraft e DeckCraft são marcas: aparecem só no `NOTICE` e como
crédito, nunca em API, docs de produto, ícones ou telas.

## O que veio e o que ficou

| origem | módulo nosso | o que é |
|---|---|---|
| `crates/geom`, `crates/color` | `geom`, `color` | medidas, transformações, formas predefinidas, cores e temas |
| `crates/model` | `model` | a apresentação: slides, layouts, mestres, formas, texto, tabelas, gráficos |
| `crates/fonts`, `crates/text` | `fonts`, `text` | fontes (skrifa, harfrust), quebra de linha, bidi, campos de data |
| `crates/anim` | `anim` | animações e transições num instante |
| `crates/render` | `render` | slides em pixels (vello_cpu) |
| `crates/pdf` | `pdf` | slides, anotações e folhetos em PDF |
| `crates/pptx`, `crates/format` | `pptx`, `format` | PPTX e o formato nativo (zip com JSON) |
| `crates/media` | `media` | sondagem de áudio e vídeo, quadro de capa |
| `crates/engine` | `engine` | `Session`: todo gesto é um comando com desfazer |
| `crates/mcp` | `mcp` | os comandos como ferramentas de agente |
| craft-libs `crates/metafile` | `craft::metafile` | WMF/EMF/EMF+, leitura e escrita |
| craft-libs `crates/pdf` | `craft::pdf` | só o escritor (`write`); leitura e raster desligados |
| craft-libs `crates/media` | `craft::media` | contêineres MP4/MOV, Matroska/WebM, Ogg |
| craft-libs `crates/audio` | `craft::audio` | só o decodificador Opus |
| craft-libs `crates/video` | `craft::video` | só os decodificadores H.264, HEVC, VP9 e AV1, com threads |
| craft-libs `vendor/krilla` | `third_party::krilla` | krilla 0.8.2 com os remendos do craft-libs |

Ficou de fora o `ui-egui` (a tela deles, egui): o editor de slides será nosso, com design antes
no Claude Design. Os apps (`apps/`), o `xtask` e os testes do craft-libs também não vêm.

Os craft-libs entram sem testes (`"tests": false, "unit_tests": false`): o DeckCraft não os roda,
e muitos dependem de features que ficaram desligadas. As features desligadas viram `cfg(any())`,
então o código de codecs que o DeckCraft não usa fica no repositório, compilado fora.

## Como a conversão funciona

```bash
scripts/storytold/convert.py deckcraft <clone deckcraft> <commit> <saída> \
    --with craft-libs=<clone craft-libs>@<commit que o DeckCraft fixa>
```

O commit do craft-libs é o do `rev =` da linha `craft-media` no `Cargo.toml` raiz do DeckCraft
naquele commit (todos os `craft-*` estão no mesmo). Regras gerais do conversor estão no topo de
`scripts/storytold/convert.py`; as próprias deste perfil, em `projects.json`:

- `features`: o mapa de cada fonte diz o que fica ligado; o `rayon` do krilla vira
  `not(target_arch = "wasm32")`, porque o DeckCraft só liga fora da web.
- `build_rs` no `fonts`: o `build.rs` dele vira o do crate (gera `CRAFT_FONTS`, vazio sem
  `CRAFT_FONTS_DIR`); `tests/fonts_build_input.rs` o inclui por `#[path]` e roda os 13 testes dele.
- `replace`:
  - o krilla usa `skrifa` 0.42 e o resto, 0.47: no krilla `skrifa::` vira `skrifa_042::` (o
    `Cargo.toml` tem `skrifa_042 = { package = "skrifa", version = "0.42.0" }`), e ganha
    `#![allow(deprecated)]` pelas constantes `std::f32::INFINITY`;
  - no `pdf`, o krilla é chamado pelo alias `backend as krilla` do `craft::pdf::write`: a regra
    desfaz a troca do nome por `crate::third_party::krilla`;
  - `fonts` e `media` leem fixtures por `CARGO_MANIFEST_DIR` e `include_str!("tests/…")`: passam a
    `tests/fonts_fixtures` e `tests/media_fixtures`;
  - no `engine`, o teste que varre o código-fonte procura em `src/engine` e `mod.rs`.

## O que é nosso

Rodar o conversor nos commits do `NOTICE` reproduz `src/`, `tests/`, `assets/` e `build.rs`, exceto:

- `src/lib.rs`: a raiz do crate, com a documentação, o exemplo (PPTX de ida e volta e PNG do
  primeiro slide) e o `allow` de lints do `third_party`, que nunca passou pelo clippy do DeckCraft.
- **zip 9**, como no `ravenrs-word` e no `ravenrs-sheet`: o DeckCraft está no zip 3, que tem
  avisos de segurança (RUSTSEC-2026-0367, 0368, 0369 e 0429). No 9 o nome de uma entrada é um
  `Result` e `data_start()` um `Option`: no código, `src/pptx/opc.rs` pula a entrada de nome
  ilegível; nos testes, `unwrap` (os arquivos com a nota `RavenRS: zip 9` na primeira linha). Numa
  mescla, conflito nessas linhas é isso: aplique o mesmo ajuste à linha nova deles.
- `Cargo.toml`, `NOTICE`, `README.md`, `LICENSES/`, `clippy.toml` (o deles sem a regra do egui) e
  `rustfmt.toml`.
- Fora do crate: `[profile.dev.package.ravenrs-deck] opt-level = 1` no `Cargo.toml` raiz (a pilha
  estoura em `groups_nest_only_as_deep_as_a_saved_file_reopens` sem otimização), e no `deny.toml`
  a exceção `Ubuntu-font-1.0` do `epaint_default_fonts` (fonte de último recurso) e o
  `RUSTSEC-2026-0206` (rustybuzz sem mantenedor, pelo texto simples do krilla).

Toda mudança nossa dentro de `src/` e `tests/` leva um comentário `RavenRS:`. Ao acrescentar uma,
liste-a aqui.

O `Cargo.toml` segue as versões deles (as do krilla vêm do `Cargo.toml` de `vendor/krilla`), salvo
o `zip`. Trocar uma versão vira mudança local em vários arquivos e conflito a cada sincronização:
só com motivo como esse.

## Trazer uma revisão nova

```bash
S=<scratchpad>
git clone --filter=blob:none https://github.com/storytold/deckcraft.git $S/deckcraft
git clone --filter=blob:none https://github.com/storytold/craft-libs.git $S/craft-libs
OLD=$(grep -o '[0-9a-f]\{40\}' crates/ravenrs-deck/NOTICE | sed -n 1p)
OLD_LIBS=$(grep -o '[0-9a-f]\{40\}' crates/ravenrs-deck/NOTICE | sed -n 2p)
NEW=$(git -C $S/deckcraft rev-parse origin/main)
NEW_LIBS=$(git -C $S/deckcraft show $NEW:Cargo.toml | grep '^craft-media' | grep -o '[0-9a-f]\{40\}')
scripts/storytold/convert.py deckcraft $S/deckcraft $OLD $S/old --with craft-libs=$S/craft-libs@$OLD_LIBS
scripts/storytold/convert.py deckcraft $S/deckcraft $NEW $S/new --with craft-libs=$S/craft-libs@$NEW_LIBS
diff -r $S/old/src crates/ravenrs-deck/src   # só lib.rs e as mudanças da seção acima
scripts/storytold/merge.py deckcraft $S/old $S/new
```

1. O `diff` antes da mescla confere a conversão: se aparecer algo fora da lista "O que é nosso", o
   conversor ou a lista estão desatualizados. Conserte isso antes de mesclar.
2. O `merge.py` pega o deles onde não mexemos, mantém o nosso onde eles não mexeram, faz
   `git merge-file` onde os dois mexeram e lista os conflitos (sai com 1 se houver).
3. Confira dependências novas: `git -C $S/deckcraft diff $OLD $NEW -- 'crates/*/Cargo.toml'
   Cargo.toml` e, no craft-libs, `crates/{metafile,pdf,media,audio,video}/Cargo.toml` e
   `vendor/krilla/Cargo.toml`. Um `craft-*` novo vira fonte nova no `projects.json` (copiado como
   módulo, nunca dependência git). Feature nova usada no código precisa entrar no mapa `features`
   da fonte, ou vira `cfg` de uma feature que o nosso crate não tem.
4. Crate novo deles usado pelo motor: acrescente em `projects.json` e em `lib.rs`.
5. Atualize os dois commits no `NOTICE`, e a lista de licenças se veio material de terceiros novo
   (o `ATTRIBUTION.md` e o `NOTICE` deles dizem).

## Testar

```bash
export CARGO_TARGET_DIR=<scratchpad>/target   # absoluto, um por worktree
cargo nextest run -p ravenrs-deck
cargo test -p ravenrs-deck --doc
cargo clippy -p ravenrs-deck --all-targets -- -D warnings
cargo check -p ravenrs-deck --target wasm32-unknown-unknown
cargo tree -p ravenrs-deck | grep -c 'git+'   # tem de dar 0
cargo deny check
scripts/check.sh
```

Os testes de origem (1638 no `b0c77eb`, mais o exemplo de `lib.rs`) têm de passar todos:
unitários de cada módulo, `tests/pptx_it`, `tests/engine_it`, `tests/mcp_it`, `tests/media_it`,
`tests/fonts_build_input.rs` e os soltos de `tests/`. Um é `#[ignore]` de origem
(`engine::soak_tests`, que roda com `DECKCRAFT_SOAK_STEPS`). O mais lento leva ~20 s
(`double_clicking_a_capped_freeform_keeps_its_vertices`).
