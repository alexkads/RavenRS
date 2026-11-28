---
name: ravenrs-feature-center
description: Regra obrigatória do RavenRS - todo componente criado ou alterado entra no tools/raven-feature-center (Raven Feature Center) com todos os seus recursos públicos, cada um com demonstração e passo no tour, no PR de lote da vitrine logo depois do PR da camada (#1085). Vale para widgets das famílias ravenrs-widgets-*, motores com tela e tipos do framework de aplicação ravenrs-app*. Diz onde pôr a página, como montar, o manifesto recursos.json e o script de cobertura que o cargo xtask pr roda. Use sempre que for criar um componente, acrescentar ou mudar um pub fn, um evento, uma variante ou um atalho de um componente, mexer no raven-feature-center, ou quando o cargo xtask pr acusar feature-center-coverage, mesmo que o pedido não cite o raven-feature-center.
---

# Todo componente no Centro de Recursos

Decisão do Alex em 15/11/2026 (#579, #580): **o `tools/raven-feature-center` é a vitrine única do RavenRS.** Um componente criado ou alterado entra nele com **todos** os recursos públicos, cada um com demonstração e passo em `tests/suite/tour.rs`.

**Desde a F2 da #1080 (#1085), em dois PRs.** A vitrine (feature-center, galeria e ComponentBook) é um workspace à parte, cada um na pasta dele, e não está no grafo da biblioteca. O componente entra no **PR da camada dele**, que compila só a camada e as de baixo. A página, a amostra na galeria, as histórias do RCB e o design system vêm no **PR de lote da vitrine**, logo depois; um lote pode levar as páginas de vários componentes. O `cargo xtask pr` roda `feature-center-coverage.py`, `rcb-coverage.py` e `gen-design-system.py --check` quando a mudança toca a vitrine, e o `cargo xtask release` sempre roda. Assim, componente sem página é recusado no lote, e o lote não sai sem fechar a conta. Para rodar o centro: `cargo run --manifest-path tools/raven-feature-center/Cargo.toml`.

**Onde abrir o editor (F3 e F4 da #1080).** O componente vive na camada dele: um widget em `camadas/widgets`, um motor de documento em `camadas/documentos`, um tipo do framework de app em `camadas/app`, uma tela de relatório ou do app em `camadas/telas` (tabela completa no `ARCHITECTURE.md`). A página dele mora em `tools/raven-feature-center`, um workspace à parte, e só se abre no PR de lote. O guia "Onde entra um componente novo", no desenho das camadas no Claude Design, diz a camada e o checklist do PR.

O centro mostra o que o framework faz, permite revisar as telas e prova que tudo ainda funciona. O `examples/component-gallery` sai até o fim da #579: o centro ganha parâmetros de abertura (`modo`, `pagina`, `incorporado`) para servir também ao site, ao Playground, ao perf e ao vídeo. Até lá, a amostra curta de um componente continua na galeria, e a demonstração completa fica no centro.

## Como ele abre

O centro tem quatro modos (#587), escolhidos por parâmetro na linha de comando (`--modo componentes`) ou na URL (`?modo=componentes`). A tabela completa está no README do centro.

- **`completo`:** o padrão.
- **`vitrine`:** o site, em memória, com a Ana preenchida.
- **`componentes`:** só as páginas de [`widgets::PAGES`](../../../tools/raven-feature-center/src/widgets/mod.rs), sem o framework de aplicação.
- **`playground`:** os exemplos.

`pagina` abre direto numa página, e `incorporado` mostra só a página.

**Página de widget nova:** entra em `widgets::PAGES`, e só ali. Dali ela vai para a navegação do centro, para o modo `componentes` e para `pagina=<id>`. A página não pode depender de `ravenrs-app*`, porque o modo `componentes` compila sem a feature `app`.

## O que conta como componente

Quem decide é o `scripts/component_inventory.py`, não a memória de quem faz o PR:

- **widget:** um `pub struct` dos crates `ravenrs-widgets*` com `impl View` ou `impl Component`, sem `#[doc(hidden)]` e fora de `NOT_LISTED`, a mesma conta do `scripts/check-component-list.py`;
- **framework de aplicação:** um `pub struct` de `ravenrs-app*` (menos `ravenrs-app-testkit` e `ravenrs-app-derive`) com pelo menos um método que recebe `mut self`, como `App`, `EntityInfo`, `ApiService` e `SecuredApp`;
- **recurso:** cada `pub fn` dos blocos `impl Tipo { … }` do próprio tipo: construtores, configuração, eventos (`on_*`) e consultas.

Os motores com tela (relatório, planilha, PDF, documento e apresentação) entram no lote W10 da #579. Até lá, mudança neles segue a regra do exemplo próprio de cada um.

## A guarda: `recursos.json` e o script de cobertura

`tools/raven-feature-center/recursos.json` tem uma entrada por componente, uma por linha:

```json
{"crate": "ravenrs-widgets-grid", "tipo": "DataGrid", "pagina": "Grade de dados", "recursos": ["new", "columns", "group_by", "…"]}
{"crate": "ravenrs-widgets-base", "tipo": "Button", "pendente": true, "issue": 579, "conhecidos": ["new", "icon", "…"]}
```

- **Demonstrado** (`pagina` e `recursos`): `recursos` é exatamente a lista dos `pub fn` do tipo. Cada um é chamado em `tools/raven-feature-center/src` (`.nome(` ou `::nome(`), e a página, com o nome que tem na navegação, aparece entre aspas em `tests/suite/tour.rs`.
- **Pendente** (`pendente`, `issue` e `conhecidos`): o componente ainda espera o lote dele. `conhecidos` são os `pub fn` que ele tinha quando entrou na espera.

O script roda sempre no `cargo xtask pr`, entre as conferências baratas, e leva cerca de 3 s:

```bash
./scripts/feature-center-coverage.py            # confere
./scripts/feature-center-coverage.py --summary  # e lista os pendentes
```

Ele falha, com `arquivo:linha`, quando:

| Situação | O que fazer |
|---|---|
| componente público sem entrada | criar a página com todos os recursos e a entrada demonstrada |
| `pub fn` novo num pendente | **o componente mudou: sai da espera neste PR**, com a página completa |
| `pub fn` novo num demonstrado | chamar o recurso na página, pôr em `recursos` e cobrir no tour |
| recurso declarado e nunca chamado | chamar na página, não apagar da lista |
| página sem passo no tour | visitar a página em `tests/suite/tour.rs` e conferir o que ela mostra |
| método ou tipo que saiu do código | tirar do manifesto |

**Nunca** acrescente um método a `conhecidos`, nem um tipo novo como `pendente`, para fazer o script passar. A lista de pendentes só diminui. Ela existe porque a vitrine começou atrasada em 15/11, não como saída para componente novo.

`tests/suite/accessibility.rs` tem outra guarda, `every_demonstration_has_a_step_in_the_tour`: toda tela da navegação precisa aparecer em `tests/suite/tour.rs`. `NOT_IN_THE_TOUR_YET` também só diminui.

O script confere os recursos pelo nome, não pelo tipo: um `.theme(` de outro tipo também conta como chamada. A conferência de verdade é o passo do tour, que precisa exercitar cada recurso, e não só abrir a página.

## Testabilidade (RTS)

Além da página, todo tipo do `recursos.json` tem uma entrada em `tools/raven-test-studio/contrato.json`, conferida pelo `scripts/rts-coverage.py` no `cargo xtask pr` (#734, #736). A entrada fica separada do `recursos.json`, como o `pendentes.json` do RCB, para os dois mudarem em paralelo. Ela é de um destes três tipos:

- **completa**: `ids` (cada um `parte:<chave>`), `acoes` (as que o componente declara com `BuildCx::on_access`), e, quando houver, `busy`, `driver` (do módulo `testing` do crate) e `cenario` (um `.raventest` em `tests/e2e`);
- `{"sem_tela": true}` para tipo sem nada na tela;
- `{"pendente": true, "issue": N}` com a issue aberta da família (#737 a #741).

O checklist e um exemplo completo estão em `CONTRIBUTING.md`, na seção "Testability (RTS)". A guarda também recusa `test_id` feito com `format!` fora de `parte:chave`, ou numerado pela posição (`{i}`, `{index}`, `{idx}`).

## Onde pôr a página

Cada família tem um grupo na navegação e uma pasta. Um componente por página, com o nome dele na página:

| Crate | Grupo na navegação | Pasta |
|---|---|---|
| `ravenrs-widgets-base` | Básicos | `src/widgets/basicos/` |
| `ravenrs-widgets-kit` | Kit | `src/widgets/kit/` |
| `ravenrs-widgets-lists` | Listas e árvores | `src/widgets/listas/` |
| `ravenrs-widgets-grid` | Grade | `src/widgets/grade/` |
| `ravenrs-widgets-charts` | Gráficos e fluxos | `src/widgets/graficos/` |
| `ravenrs-widgets-chat` | Conversa e código | `src/widgets/conversa/` |
| `ravenrs-widgets-dock` | Doca e faixa | `src/widgets/doca/` |
| `ravenrs-widgets-scheduler` | Agenda | `src/widgets/agenda/` |
| `ravenrs-widgets-imaging` | Imagem e animação | `src/widgets/imagem/` |
| `ravenrs-app*` | o grupo do recurso (Validação, Actions, Listas, Segurança…) | `src/model.rs`, `src/actions.rs` ou um módulo do grupo |

## Como montar

1. **Faça o inventário pelo código.**

   ```bash
   python3 -c "import sys; sys.path.insert(0,'scripts'); import component_inventory as c; print(sorted(c.public_methods('<crate>', '<Tipo>')))"
   ```

2. **Monte a página.** Uma demonstração de widget é uma view em `src/widgets/<familia>/<componente>.rs` com uma entrada em `widgets::PAGES` (id, nome, grupo, ícone e a função que a cria). O centro já põe cada entrada como `DashboardInfo` no grupo dela e registra o controle para `watched()` e `remote()`. O modelo é a página dos cabos.
3. **Mostre todos os recursos, organizados por tarefa e não por método.** Use seções como "Tamanhos", "Estados", "Teclado", "Eventos". Cada evento escreve o que aconteceu numa linha visível, para que o tour possa conferir. Cada variante aparece lado a lado com as outras.
4. **Escreva o passo no tour:** abrir a página e exercitar cada recurso com o mouse e o teclado pelo `Ui`, conferindo o efeito, não só a presença.
5. **Atualize junto:**
   - a entrada em `recursos.json`, com `pagina` e `recursos` exatos;
   - `demonstrations!` em `tests/suite/accessibility.rs`, que audita a tela nova;
   - a tabela "What is here" do `tools/raven-feature-center/README.md`;
   - se o componente é novo, `site/components/index.html` e a amostra, como pede o `check-component-list.py`.
   - se o componente é novo, a tabela "Components" do `README.md`: é dela que a ferramenta `list_components` do `ravenrs-mcp` tira o grupo, e o que não está lá cai em `Other`. O `ravenrs-mcp` lê o `recursos.json` sozinho (ferramenta `feature_center`).

## O que não fazer

- **Não embrulhar nem estender o componente no exemplo para a demonstração ficar bonita.** O que faltar vira recurso do próprio componente, no crate dele (memória `recursos-dentro-do-componente`, regra "a casa segue o padrão").
- **Não inventar visual.** Ajuste de UX feito no centro vira padrão da biblioteca (`ravenrs-widgets`, `ravenrs-app-ui`, `docs/design-system/README.md`). Redesenho passa antes pelo Claude Design.
- **Não deixar o recurso para depois do lote.** O componente muda no PR da camada e é demonstrado no PR de lote da vitrine que vem logo em seguida; o lote não fecha com recurso sem página.

## Como verificar

```bash
./scripts/feature-center-coverage.py
cargo nextest run --manifest-path tools/raven-feature-center/Cargo.toml
cargo run --manifest-path tools/raven-feature-center/Cargo.toml
cargo xtask web raven-feature-center && python3 -m http.server -d tools/raven-feature-center/web 8099
```

- **Desktop:** entrar com `ana`, `bia` e `caio` (senha `raven-demo`) e capturar a página nova.
- **Web:** percorrer com Playwright em desktop e em 375 px.
- **No PR:** registrar no "How it was tested" a saída do script ("N components: X shown, Y waiting") e o `cargo xtask pr`.
