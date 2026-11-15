---
name: ravenrs-feature-center
description: Regra obrigatória do RavenRS - todo componente criado ou alterado entra no examples/feature-center (Raven Centro de Recursos) com todos os seus recursos públicos, cada um com demonstração e passo no tour, no mesmo PR. Vale para widgets das famílias ravenrs-widgets-*, motores com tela e tipos do framework de aplicação ravenrs-app*. Diz onde pôr a página, como montar, o manifesto recursos.json e o script de cobertura que o check.sh roda. Use sempre que for criar um componente, acrescentar ou mudar um pub fn, um evento, uma variante ou um atalho de um componente, mexer no feature-center, ou quando o check.sh acusar feature-center-coverage, mesmo que o pedido não cite o feature-center.
---

# Todo componente no Centro de Recursos

Decisão do Alex em 15/11/2026 (#579, #580): **o `examples/feature-center` é a vitrine única do RavenRS.** Um componente criado ou alterado entra nele com **todos** os recursos públicos, cada um com demonstração e passo em `tests/tour.rs`, **no mesmo PR**. Sem isso, o PR não está pronto, e o `scripts/check.sh` falha.

O centro mostra o que o framework faz, permite revisar as telas e prova que tudo ainda funciona. O `examples/component-gallery` sai até o fim da #579: o centro ganha parâmetros de abertura (`modo`, `pagina`, `incorporado`) para servir também ao site, ao Playground, ao perf e ao vídeo. Até lá, a amostra curta de um componente continua na galeria, e a demonstração completa fica no centro.

## Como ele abre

O centro tem quatro modos (#587), escolhidos por parâmetro na linha de comando (`--modo componentes`) ou na URL (`?modo=componentes`). A tabela completa está no README do centro.

- **`completo`:** o padrão.
- **`vitrine`:** o site, em memória, com a Ana preenchida.
- **`componentes`:** só as páginas de [`widgets::PAGES`](../../../examples/feature-center/src/widgets/mod.rs), sem o framework de aplicação.
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

`examples/feature-center/recursos.json` tem uma entrada por componente, uma por linha:

```json
{"crate": "ravenrs-widgets-grid", "tipo": "DataGrid", "pagina": "Grade de dados", "recursos": ["new", "columns", "group_by", "…"]}
{"crate": "ravenrs-widgets-base", "tipo": "Button", "pendente": true, "issue": 579, "conhecidos": ["new", "icon", "…"]}
```

- **Demonstrado** (`pagina` e `recursos`): `recursos` é exatamente a lista dos `pub fn` do tipo. Cada um é chamado em `examples/feature-center/src` (`.nome(` ou `::nome(`), e a página, com o nome que tem na navegação, aparece entre aspas em `tests/tour.rs`.
- **Pendente** (`pendente`, `issue` e `conhecidos`): o componente ainda espera o lote dele. `conhecidos` são os `pub fn` que ele tinha quando entrou na espera.

O script roda sempre no `check.sh`, entre as conferências baratas, e leva cerca de 3 s:

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
| página sem passo no tour | visitar a página em `tests/tour.rs` e conferir o que ela mostra |
| método ou tipo que saiu do código | tirar do manifesto |

**Nunca** acrescente um método a `conhecidos`, nem um tipo novo como `pendente`, para fazer o script passar. A lista de pendentes só diminui. Ela existe porque a vitrine começou atrasada em 15/11, não como saída para componente novo.

`tests/accessibility.rs` tem outra guarda, `every_demonstration_has_a_step_in_the_tour`: toda tela da navegação precisa aparecer em `tests/tour.rs`. `NOT_IN_THE_TOUR_YET` também só diminui.

O script confere os recursos pelo nome, não pelo tipo: um `.theme(` de outro tipo também conta como chamada. A conferência de verdade é o passo do tour, que precisa exercitar cada recurso, e não só abrir a página.

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
   - `demonstrations!` em `tests/accessibility.rs`, que audita a tela nova;
   - a tabela "What is here" do `examples/feature-center/README.md`;
   - se o componente é novo, `site/components/index.html` e a amostra, como pede o `check-component-list.py`.

## O que não fazer

- **Não embrulhar nem estender o componente no exemplo para a demonstração ficar bonita.** O que faltar vira recurso do próprio componente, no crate dele (memória `recursos-dentro-do-componente`, regra "a casa segue o padrão").
- **Não inventar visual.** Ajuste de UX feito no centro vira padrão da biblioteca (`ravenrs-widgets`, `ravenrs-app-ui`, `docs/design-system/README.md`). Redesenho passa antes pelo Claude Design.
- **Não deixar o recurso para outro PR.** O componente muda e é demonstrado no mesmo PR.

## Como verificar

```bash
./scripts/feature-center-coverage.py
cargo nextest run -p feature-center
cargo run -p feature-center
./scripts/build-web.sh feature-center && python3 -m http.server -d examples/feature-center/web 8099
```

- **Desktop:** entrar com `ana`, `bia` e `caio` (senha `raven-demo`) e capturar a página nova.
- **Web:** percorrer com Playwright em desktop e em 375 px.
- **No PR:** registrar no "How it was tested" a saída do script ("N components: X shown, Y waiting") e o `check.sh`.
