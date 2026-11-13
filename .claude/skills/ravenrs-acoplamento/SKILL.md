---
name: ravenrs-acoplamento
description: Regras do RavenRS para não criar código acoplado - camadas entre módulos e crates, módulos que se usam em ciclo, quando um módulo vira crate e como desfazer um nó sem mudar a API pública. Use sempre que for criar ou mover um módulo ou crate, acrescentar um `use crate::…` entre módulos de topo, criar um componente que precise de outro, refatorar ou dividir um crate grande, ou quando o `ravenrs-lint` acusar `module_cycle`, mesmo que o pedido não cite "acoplamento".
---

# Acoplamento no RavenRS

Um crate só se lê, se testa, se compila em paralelo e se divide por partes se as partes não se usarem em círculo. Em novembro de 2026 o `ravenrs-widgets` tinha 41 de 75 módulos num ciclo só (#293), o `ravenrs-app` 18 e o `ravenrs-app-ui` 17 (#294). Nenhum deles podia sair do crate sem levar os outros, e qualquer mudança recompilava e exigia ler tudo. Esta skill existe para isso não voltar a acontecer.

A regra `module_cycle` do `ravenrs-lint` mede e barra. Esta skill diz como escrever para não ser barrado.

## As regras

1. **Camadas, de baixo para cima.** Um módulo de base (`context`, `view`, `button`, `overlay`…) nunca nomeia um módulo de cima (`grid`, `scheduler`, `dock`…). Se a base precisa de algo de cima, ela recebe um trait, uma closure ou um valor pronto.
2. **Folha não usa folha.** Dois componentes do mesmo nível (`grid` e `scheduler`, `chat` e `imaging`) não se importam. O que os dois precisam desce para a base, sem cópia: a regra da casa é reaproveitar.
3. **Nada de agregador que importa todo mundo.** Um tipo que junta um pedaço de cada componente (como o `Labels`, que fazia `labels` usar 36 módulos) é montado de baixo para cima: cada componente define a própria parte (`GridLabels` em `grid`) e o agregador só a guarda. A base não guarda o agregador por valor.
4. **Par tolerado, trio não.** Dois módulos que se usam (uma tela e seu auxiliar) passam. Três ou mais em laço são erro.
5. **Crate novo quando a camada estiver limpa.** Um módulo só vira crate quando não está em ciclo (`--modules` mostra). O crate antigo reexporta nos mesmos caminhos (`pub use`), e o usuário não percebe a mudança.

## Antes de escrever

- Vai acrescentar `use crate::outro::…` num módulo de topo? Veja com `cargo ravenrs-lint -p <pacote> --modules` se `outro` já usa o seu, direta ou indiretamente. Se usa, a dependência está ao contrário: inverta com trait ou callback, ou desça o pedaço comum.
- Vai criar um componente que reutiliza outro do mesmo nível? Use só o que é público na base. Se precisar de algo interno do outro, isso é sinal de que o pedaço pertence à base.
- Vai criar um módulo grande? Pense em qual camada ele está e em quem pode usá-lo, e escreva isso no `//!` do módulo.

## Quando o lint acusar `module_cycle`

```bash
cargo run -q -p ravenrs-lint -- -p <pacote> --modules            # tamanho, quem usa quem, ciclos
cargo run -q -p ravenrs-lint -- --workspace --only module_cycle  # o que o check.sh roda
```

O diagnóstico aponta o `use` do módulo que mais nomeia outros dentro do laço (o nó a desatar primeiro) e mostra o caminho de volta, como em `lists → context → labels → dock → lists`. Para desfazer:

1. Ache a aresta que vai de baixo para cima no caminho (no exemplo, `context → labels`: a base usando o agregador).
2. Corte-a: mova o tipo comum para baixo, troque a importação por um trait que o módulo de cima implementa, ou receba o valor por parâmetro.
3. Rode `--modules` de novo. O ciclo encolhe ou se parte em pedaços menores.
4. Baixe o `max-module-cycle` do pacote para o novo tamanho. O lint pede isso sozinho quando o teto fica folgado.

Uma exceção pontual leva motivo, na linha do `use`, e tira só aquela aresta: `// ravenrs-lint: allow(module_cycle) <por quê>`. É para casos raros e justificados, não para calar a regra.

## Três cortes que já funcionaram (#293, de 41 para 20 no `ravenrs-widgets`)

1. **Método que recebe tipo de componente sai do agregador.** `Labels::dock_place(DockSide)` fazia `labels` depender do `dock`. Um bloco `impl Labels { .. }` pode ficar em qualquer módulo do mesmo crate: o método foi para `dock/layout.rs`, ao lado de `DockSide`, e continua sendo `Labels::dock_place`, com a API idêntica. O mesmo valeu para `summary(Summary)` (em `grid/model.rs`) e `recurrence_summary` (em `recurrence.rs`). Depois de mover, rode `scripts/gen-live.py`, que dá a cada bloco o seu `part`, para o playground continuar vendo o método, e escreva `impl Labels {` com o tipo importado, não `impl crate::labels::Labels {`, que o script não reconhece.
2. **Dado que a base guarda desce para a base.** `BuildCx` guardava `scroll_scene::ScrollTrack` (dois `f32`): o tipo foi para o `context`. Funções de geometria de painel que `commands` pegava do `chat` foram para `floating`, a casa delas.
3. **A base não nomeia o tipo de cima: usa trait ou chave de tipo.** O `BuildCx` guardava o `nav::SidebarScope` e o `ui::ViewProfile`. Agora guarda valores com escopo por tipo (`cx.scoped::<T>()` e `cx.replace_scoped::<T>(..)`) e um `dyn BuildTiming`, que o `ViewProfile` implementa. Um componente novo que precise passar dados aos filhos usa `scoped`, sem acrescentar campo ao `BuildCx`.

## De 20 para 2 (#298), e as famílias sem atalho entre si (#299)

4. **Dado e view em módulos separados.** O `context` guardava tipos que moravam junto de views. Cada um ganhou módulo próprio, sem view: `ids` (`NodeId`, `WidgetId`), `motion_spec`, `link_service`, `file_picker`, `scroll_link` e `time_link`. O `Localized` saiu de `labels`, que passou a depender só de `date`.
5. **Helper puro desce para `util`; peça pequena ganha módulo.** O anel dos controles (`button → display → button`, `choice ↔ color_picker`, `overlay ↔ floating`) caiu com `Kbd` em módulo próprio, `spinner_paint`, `to_hex` e `stroke_outline` em `util`, e o `SearchPanel` no `overlay`, onde o `Select` o usa.
6. **O que duas famílias usam desce para a base.** `SelectionMode` (de `grid`), `reorder` (de `lists`), `ZoomView`/`ZoomControl` (de `imaging`), `FloatingCorner` (de `chat`) e `Hotkey` (de `commands`). Um tipo de valor vive onde a primeira view o usou, não onde pertence.

Para manter um caminho público ao mover, troque a reexportação da raiz para o módulo novo. Se o módulo antigo for `pub mod`, como o `grid`, deixe nele um `pub use` do novo, como `grid::SelectionMode`.

## A API pública não muda (`scripts/public-api.py`)

`crates/ravenrs-widgets/public-api.txt` lista cada caminho que um usuário escreve: tipo, campo, variante, método, reexportação e trait implementado. É gerado do JSON do rustdoc, que só sai no nightly. O `check.sh` roda `scripts/public-api.py --check`, que falha mostrando a diferença. Mover um item entre módulos ou crates mantém o arquivo igual desde que ele continue reexportado no mesmo caminho. Regenere o arquivo (`scripts/public-api.py`) só quando a mudança for de propósito, e diga isso no PR.

## O teto por pacote (catraca)

`[package.metadata.ravenrs-lint] max-module-cycle = N` segura o tamanho atual de um crate que ainda está sendo desacoplado. O teto **só desce**: se o maior ciclo ficar menor que `N`, o lint falha pedindo o número novo. Nunca suba o teto para fazer uma mudança passar. Desfaça o acoplamento que ela criou.

Tetos em 13/11/2026: `ravenrs-widgets` sem teto (era 41; #293, maior ciclo 2), `ravenrs-app` 18 e `ravenrs-app-ui` 17 (#294), `ravenrs-app-pos` 3 e `ravenrs-live` 3 (#292).

## Por que importa também para a IA

Uma sessão que vai mexer no `DataGrid` não precisa ler 70 mil linhas se o `grid` depende só da base: o `Cargo.toml` e o `//!` do módulo dizem o contrato. Com ciclos, entender uma parte exige carregar o resto, o que gasta tokens e aumenta o risco de mexer no lugar errado. Crates menores também dão retorno mais rápido (`cargo check -p` de uma folha).

## Onde está

- Regra: `crates/ravenrs-lint/src/coupling.rs`, testes em `crates/ravenrs-lint/tests/rules.rs`.
- No `scripts/check.sh`: o passo `module_cycle` roda em todo o workspace.
- Mapa de crates e fronteiras: skill `ravenrs-arquitetura` e `ARCHITECTURE.md` §3.
