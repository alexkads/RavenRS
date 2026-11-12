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

## O teto por pacote (catraca)

`[package.metadata.ravenrs-lint] max-module-cycle = N` segura o tamanho atual de um crate que ainda está sendo desacoplado. O teto **só desce**: se o maior ciclo ficar menor que `N`, o lint falha pedindo o número novo. Nunca suba o teto para fazer uma mudança passar. Desfaça o acoplamento que ela criou.

Tetos em 12/11/2026: `ravenrs-widgets` 41 (#293), `ravenrs-app` 18 e `ravenrs-app-ui` 17 (#294), `ravenrs-app-pos` 3 e `ravenrs-live` 3 (#292).

## Por que importa também para a IA

Uma sessão que vai mexer no `DataGrid` não precisa ler 70 mil linhas se o `grid` depende só da base: o `Cargo.toml` e o `//!` do módulo dizem o contrato. Com ciclos, entender uma parte exige carregar o resto, o que gasta tokens e aumenta o risco de mexer no lugar errado. Crates menores também dão retorno mais rápido (`cargo check -p` de uma folha).

## Onde está

- Regra: `crates/ravenrs-lint/src/coupling.rs`, testes em `crates/ravenrs-lint/tests/rules.rs`.
- No `scripts/check.sh`: o passo `module_cycle` roda em todo o workspace.
- Mapa de crates e fronteiras: skill `ravenrs-arquitetura` e `ARCHITECTURE.md` §3.
