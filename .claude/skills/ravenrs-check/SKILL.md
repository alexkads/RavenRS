---
name: ravenrs-check
description: Como rodar, ler e manter as checagens do RavenRS (não há CI hospedado), feitas pelo cargo xtask - lint para iterar rápido sem testes, dev enquanto se trabalha, pr como gate do PR na máquina de quem desenvolve, release antes de publicar, cargo-nextest obrigatório, um CARGO_TARGET_DIR por worktree, o que o seletor de pacotes testa e o que deixa para o release. Use sempre que for validar um PR ou branch, rodar ou interpretar o cargo xtask lint, dev, pr ou release, investigar teste que falha só no check, um check lento ou "can't find crate", instalar o ambiente de quem contribui, ou mexer no xtask/, no changed-packages.py ou nos perfis do Cargo.toml, mesmo que o pedido não cite esta skill.
---

# As checagens do RavenRS (`cargo xtask`)

O projeto não tem CI hospedado. As checagens são subcomandos do `cargo xtask` (crate `xtask/`, só biblioteca padrão), cada um com nome e finalidade fixos e sem argumentos (#1034). Decisão do Alex em 27/11/2026: nada de um script só com parâmetros, e nada de `.sh`, para rodar igual no Mac, no Linux e no Windows. O `scripts/check.sh` foi apagado.

| Subcomando | Quando | O que roda |
|---|---|---|
| `cargo xtask lint` | para iterar rápido, **sem nenhum teste** | rustfmt, conferências baratas e clippy **só dos pacotes alterados**. Nunca vale como gate: antes do commit vem o `dev`, antes do merge o `pr` |
| `cargo xtask dev` | enquanto se trabalha, antes do commit | rustfmt, conferências baratas, clippy e testes **só dos pacotes alterados** |
| `cargo xtask pr` | o gate do PR, na máquina de quem desenvolve | clippy em tudo o que a mudança alcança, testes dos alterados e dos dependentes diretos, design system e API pública quando alcançados |
| `cargo xtask release` | antes de publicar | o workspace inteiro, wasm32, features opcionais, MSRV, `cargo deny`, docs e builds web |
| `cargo xtask web <nome>` | build para o navegador | o que o `scripts/build-web.sh` fazia |

Todo PR registra em "How it was tested" que o `cargo xtask pr` passou, o tempo total, as três etapas mais lentas da tabela final e se o diretório de build começou vazio.

## Antes da primeira vez

**O `cargo-nextest` é obrigatório.** Decisão do Alex em 12/11/2026: velocidade, e todo mundo usa. Sem ele, o script para antes de compilar e mostra o comando:

```bash
cargo install cargo-nextest --locked
```

Vale para todas as máquinas: a do Alex, a do Matheus, as de Linux e Windows, e as worktrees dos agentes. Para conferir, rode `cargo nextest --version`. O nextest roda todos os binários de teste ao mesmo tempo, enquanto o `cargo test` roda um depois do outro, e põe cada teste num processo próprio. Os doctests ele não roda, e por isso o script os passa ao `cargo test --doc`.

## Rodar

```bash
cargo xtask lint
```

Para iterar rápido entre uma edição e outra, **sem nenhum teste**: rustfmt, as conferências baratas e o clippy dos pacotes alterados. Não exige o `cargo-nextest`. Nunca vale como prova: antes do commit vem o `dev` e antes do merge o `pr`, e a mensagem final lembra isso.

```bash
cargo xtask dev
```

Enquanto se trabalha. Só os pacotes cujos arquivos mudaram: clippy `--all-targets`, testes e doctests. Os dependentes, o design system e a API pública ficam para o `pr`. Quando a mudança alcança o workspace inteiro (`ALL`: perfis, lints, toolchain, `.cargo/`), ele roda só o clippy do workspace, para provar que compila.

```bash
cargo xtask pr
```

O gate do PR, com a mesma cobertura do antigo `check.sh` rápido. Compara com `origin/master`; para mudar a base, use `CHECK_BASE=<ref>`.

```bash
cargo xtask release
```

Tudo: o workspace inteiro, wasm32, as features opcionais, o MSRV, o `cargo deny`, a documentação e os builds web, com a mesma cobertura do antigo `--full`. Vai antes de publicar uma versão ou quando o Alex pedir. Os benchmarks e o gate de desempenho só entram quando ele pedir, com `RAVENRS_GATE=1` (skill `ravenrs-desempenho`).

Nenhum deles aceita argumentos: `cargo xtask pr --full` é recusado e diz qual usar.

## Onde o tempo vai

Ao terminar, passando ou falhando, o script imprime uma tabela com o tempo de cada etapa, da mais demorada para a mais rápida, com a porcentagem do total. Quem monta a tabela é o `scripts/check-timings.py`, e ela fica salva em `<target>/check-timings/last.txt`. Para comparar duas rodadas, copie o arquivo antes da segunda e use `diff`.

- **Cabeçalho:** modo (`quick`, `quick, ALL` ou `full`), se o diretório de build começou `empty` (vazio) ou `reused` (com algo de antes), se passou ou falhou, o branch e a hora. `reused` só diz que havia alguma coisa: um diretório que só compilou o `ravenrs-lint` também é `reused`, e a mudança seguinte ainda compila quase tudo. Para comparar duas rodadas, rode a mesma mudança duas vezes seguidas e use a segunda. Com o sccache ligado, um `empty` ainda reaproveita o cache dele, então dois `empty` só se comparam na mesma máquina.
- **Etapa de testes:** vem dividida em `compile the tests` (o `cargo nextest run --no-run`), `run the tests` e `doctests`. Se a compilação pesa mais que a execução, o ganho está no build, não nos testes.
- **Etapa que falhou:** aparece marcada `FAILED`, com o tempo até a falha. O código de saída continua o do erro.
- **Testes mais lentos:** os 10 mais demorados, lidos do JUnit que o nextest grava em `target/nextest/default/junit.xml`, na raiz do repositório mesmo com outro `CARGO_TARGET_DIR` (configurado em `.config/nextest.toml`). Os que passam de 30 s também saem como `SLOW` no resumo do nextest.
- **Compilação por crate:** com `RAVENRS_CHECK_TIMINGS=1`, o clippy e a compilação dos testes recebem `--timings`, e o script lista os `cargo-timing-*.html` desta rodada. Eles mostram quais crates demoram e quanto o build ficou em série, esperando um crate só.

```bash
RAVENRS_CHECK_TIMINGS=1 cargo xtask pr
```

Para investigar um check lento, leia a tabela antes de mexer em qualquer coisa: a etapa no topo é onde está o ganho.

## O que o `pr` escolhe (e o `dev`)

Quem escolhe é o `scripts/changed-packages.py`. Os passos, em ordem:

1. **rustfmt** no workspace inteiro.
2. **O check.sh não volta** (#1068), em Rust, no `xtask/src/guard.rs`: reprova o `scripts/check.sh` ou o `scripts/build-web.sh` de volta, qualquer `*.sh` novo em `scripts/` (os cinco de hoje estão em `KNOWN_SCRIPTS`) e qualquer citação de um dos dois num arquivo do `git ls-files`, versionado ou novo. A mensagem traz `arquivo:linha` e o comando do xtask que substitui. Passam o histórico (`CHANGELOG.md`, `novidades*.json`, `historico-de-novidades.json`) e as linhas da lista `ALLOWED`, curta e comentada, que só contam o que aconteceu com os scripts. Toda citação conta, não só a que manda rodar: prosa nova fala do `cargo xtask`.
3. **Conferências baratas do workspace inteiro**, sempre: ciclos de módulos (`ravenrs-lint --only module_cycle`), camadas dos crates de widgets (`check-widget-layers.py`), as camadas do repositório em `camadas.toml` (`camadas.py check`, #1080: todo pacote numa camada só e nenhuma dependência, nem de teste, para uma camada de cima), arquivos de teste declarados em `tests/suite/main.rs` (`check-test-suites.py`) e as listas do playground em dia (`gen-live.py --check`, #828).
4. **O teste do seletor** (`scripts/changed_packages_test.py`). O que o seletor deixa de fora não é conferido, por isso ele tem testes próprios.
5. **ravenrs-lint** nos exemplos, só quando muda o crate do lint, o `family-finance`, o `component-gallery` ou a configuração no `Cargo.toml`.
6. **API pública do `ravenrs-widgets`** (`public-api.py --check`, rustdoc no nightly), só quando a mudança alcança a fachada `ravenrs-widgets`.
7. **Design system versionado** (`gen-design-system.py --check`, que roda o `--verify` do RCB, #1000), só quando a mudança alcança o `raven-component-book` ou mexe em `docs/design-system/export/`, no script ou num arquivo que o RCB embute de fora de qualquer pacote: o guia `docs/design-system/README.md` e o catálogo `site/components/index.html`. Leva uns 3 minutos, porque o RCB mede todas as histórias de novo. Para corrigir: `scripts/gen-design-system.py`.
8. **clippy `--all-targets`** em tudo o que a mudança alcança, em qualquer profundidade. Isso também prova que esses pacotes compilam.
9. **testes** (nextest e doctests) só dos pacotes alterados e dos dependentes diretos. Os dependentes mais distantes ficam para o `release`. Também fica para o `release` o dependente direto que não foi alterado e puxa um crate pesado (`HEAVY` no `changed-packages.py`: o `ravenrs-ai`, com o `candle` e o `tokenizers`, e o `ravenrs-movie-video`, com o `openh264`) que a mudança não alcança. Na prática, uma mudança só no `ravenrs-app` não testa o `family-finance`, mas o clippy do passo 8 continua provando que ele compila. A saída mostra esses pacotes na linha `tested in --full:` (#348). No `pr` e no `release`, os testes e os doctests rodam com `--no-fail-fast`: uma falha não interrompe os outros, e a rodada vermelha lista tudo o que quebrou de uma vez. O `dev` para na primeira falha, para responder rápido.

| Mudança | Alcança |
|---|---|
| Arquivo dentro de um crate | o crate, e os dependentes dele |
| Crate novo | só ele (membro novo no `Cargo.toml` e no `Cargo.lock` não vira `ALL`) |
| Versão do workspace ou de crate interno | os crates que usam `CARGO_PKG_VERSION` |
| Entrada de `[workspace.dependencies]` | os crates que usam essa dependência |
| Crate externo atualizado no `Cargo.lock` | os membros que chegam nele pelo grafo do lock |
| `[workspace.metadata]` | só o lint |
| `[profile.*]`, `[workspace.lints]`, edition, resolver, `[patch]`, `.cargo/`, toolchain, `clippy.toml` | `ALL`: workspace inteiro |
| `rustfmt.toml`, `deny.toml` | nada a mais (o rustfmt já cobre tudo, e o deny é do `release`) |

Para ver a escolha sem compilar nada:

```bash
./scripts/changed-packages.py --shell
```

## Workspaces separados (#1080, F1 #1084)

As nove camadas da biblioteca (`camadas/<nome>/Cargo.toml`, membros em `crates/`, F3 #1086), os apps (`examples/*` e `tools/*`), a vitrine (feature-center, galeria e ComponentBook, F2 #1085) e o Raven CI são workspaces Cargo próprios, com `Cargo.lock` próprio. A raiz tem só a infra (o `xtask`, o lint, o mcp, o `cargo-ravenrs`). Mexer num widget roda clippy, testes e doctests da camada `camadas/widgets` inteira, e nada acima dela. Quando a mudança alcança tudo (`ALL`: perfis, lints, `.cargo/`, toolchain), o `pr` percorre as nove camadas e a raiz. O `module_cycle` roda em cada camada; a API pública roda quando a mudança toca `camadas/nucleo` ou `camadas/widgets`; no `release`, os passos com `-p` (wasm32, features opcionais, MSRV, docs, `cargo deny`) vão ao manifesto da camada de cada pacote. Quando a mudança toca a vitrine, o `pr` roda também a cobertura do feature-center, a do RCB e o design system (o PR de lote, skill `ravenrs-feature-center`). Quem diz qual é qual é o `camadas.toml` (`separada = true`), e tudo passa por `scripts/camadas.py`:

- `camadas.py workspaces` lista as pastas; `--mudados` dá só as que a mudança toca. O `lint`, o `dev` e o `pr` rodam rustfmt, clippy e (menos no `lint`) os testes só nos workspaces tocados. Uma mudança na biblioteca não compila app nenhum; o `release` percorre todos.
- `camadas.py deps` confere que cada workspace separado tem a cópia em dia do `Cargo.toml` da raiz: versão, `[workspace.dependencies]` com os `path` reescritos, lints e perfis, entre as marcas `>>> camadas.py` e `<<< camadas.py`. Confere também que dois `Cargo.lock` não resolvem versões diferentes da mesma série de um crate externo. Depois de mudar a raiz (versão nova, dependência nova), rode `python3 scripts/camadas.py deps --write`. Ele escreve também o `members` e o `exclude` da raiz e o `workspace = "../../camadas/<nome>"` de cada crate da biblioteca: um crate novo entra numa seção de `camadas.toml` e o `--write` faz o resto. Ele também cria o `Cargo.lock` que faltar, copiando o da raiz para as versões serem as mesmas.
- Todos compilam no `target/` da raiz (`[build] target-dir` em `.cargo/config.toml`), o do slot.
- Um app se roda pela raiz com `cargo run --manifest-path examples/<app>/Cargo.toml`, ou de dentro da pasta dele. `cargo run -p <app>` na raiz não acha mais o app.

## Slots fixos e aquecidos (slot-a e slot-b)

Desde 28/11/2026, por regra permanente do Alex (*"não tem como seguir nessa lentidão"*), tudo o que compila no RavenRS roda num de dois slots fixos (#1075):

- `/Users/alexkads/Projects/RavenRS/.claude/worktrees/slot-a`
- `/Users/alexkads/Projects/RavenRS/.claude/worktrees/slot-b`

Cada slot usa o `target` padrão dentro dele, sem `CARGO_TARGET_DIR`, e esse **target não se apaga** ao fim da tarefa (a exceção está na regra global de limpeza). Antes, cada tarefa abria uma worktree com um diretório de build novo e o apagava ao terminar. Cada PR chegava a passar por 3 a 4 builds de 9 a 21 minutos, de 15 a 19 GB cada.

Para trabalhar:

```bash
git -C <slot> fetch -q
git -C <slot> switch -c claude/<tarefa> origin/master   # ou: git -C <slot> switch <branch-do-PR>
# ... editar, cargo nextest run -p <pacote>, cargo xtask dev ...
git -C <slot> switch --detach origin/master             # ao terminar, slot limpo
```

- Como o caminho não muda, o Cargo recompila só o que a troca de branch mexeu.
- **Um agente por slot.** Se os dois estiverem ocupados, a tarefa espera. Não se cria uma worktree avulsa para compilar.
- O branch preso numa worktree de outra sessão vem para o slot com `git switch --detach origin/<branch>` e volta com `git push origin HEAD:<branch>`.
- O revisor não compila: lê o diff e o retorno de quem implementou. O `cargo xtask pr` roda uma vez por lote, num slot.
- O trabalho que não compila (issues, docs, conferências) não ocupa slot.

**Nunca dois checkouts no mesmo target.** O Cargo não põe o caminho de um crate local no hash dos artefatos, então duas pastas que dividem um `CARGO_TARGET_DIR` trocam artefatos. Isso já deu dois sintomas:

- "can't find crate for `ravenrs`" num doctest;
- um derive compilado com um `ravenrs-app-spec` antigo, de outro branch.

O script grava a pasta da checkout em `<target>/.ravenrs-checkout` e avisa em amarelo quando o diretório foi usado por outra. Diante desse aviso, ou de um dos sintomas, confira se alguém apontou um `CARGO_TARGET_DIR` para o target de um slot. O target de um slot com o workspace inteiro compilado passa de 20 GB: confira o disco antes de um `pr` ou `release` (`df -h /System/Volumes/Data`).

Fora dos slots, só numa checkout avulsa de outra máquina ou de quem contribui, vale o jeito antigo: um `CARGO_TARGET_DIR` próprio, por caminho absoluto, apagado ao terminar.

## Cache do compilador entre worktrees (xcache por padrão, Kache como alternativa)

Com os slots, o caminho principal é o target que fica. O cache vale para uma worktree avulsa ou para o primeiro build de um slot. No macOS, os proc macros não são reaproveitados entre pastas diferentes (#1072). Cada worktree tem o seu diretório de build, então uma worktree nova compilava tudo de novo. Desde a #358, o `cargo xtask pr` põe o `ravenrs-xcache` na frente do `rustc` (seção abaixo). Com `RAVENRS_XCACHE=0` e o [Kache](https://github.com/kunobi-ninja/kache) instalado, o script usa o Kache como `RUSTC_WRAPPER`, e o que outra worktree já compilou volta do cache. A instalação é uma vez por máquina:

```bash
cargo install kache --locked --version 2.1.1
```

- O script só liga o Kache com `RAVENRS_XCACHE=0`, o `kache` no `PATH` e nenhum `RUSTC_WRAPPER` definido. Sem ele, o check roda igual, só mais devagar. `RAVENRS_KACHE=0` o deixa de fora.
- **Não rode `kache init`** sem o Alex pedir: ele altera o ambiente do usuário. O script não precisa disso.
- O diretório de build continua um por worktree. O Kache guarda os artefatos num cache próprio (cerca de 31 GB depois de um `--full`) e os restaura por clone copy-on-write.
- Ao final, o check imprime a taxa de acertos (`kache stats --last-build`). Para saber por que um crate não veio do cache: `kache explain <crate>`.
- O Kache passa `--remap-path-prefix` ao `rustc`, então os caminhos gravados nos artefatos são marcadores (`/kache/workspace`, `/kache/target`). No depurador, mapeie de volta: `settings set target.source-map /kache/workspace <checkout>` no lldb.

**Medido em 14/11/2026** (Mac de 10 núcleos e 16 GB, `cargo xtask release`, uma rodada de cada):

| Cenário | Tempo |
|---|---|
| Worktree nova, sem cache (como era) | 31m04s |
| Worktree nova, Kache com cache vazio | 29m33s |
| Mesma worktree de novo, sem alterações | 5m58s |
| Worktree nova do mesmo commit, Kache com cache cheio | **11m30s** |

Os 5m58s são o piso: rodar os testes, os doctests, o `wasm-bindgen` e o `wasm-opt`, que não são compilação. Na worktree nova, 1.264 de 2.024 compilações vieram do cache, 91,5% do tempo de compilação; o que ainda compila são sobretudo os projetos que a CLI gera em pastas temporárias (2m26s) e os builds web (1m59s).

**O cache próprio (`ravenrs-xcache`), ligado por padrão (#358).** `cargo xtask pr` usa o cache de compilação deste repositório (`crates/ravenrs-xcache`, issue #352) sem precisar de variável. `RAVENRS_XCACHE=0` desliga, e aí o Kache entra se estiver instalado. Ao fim de cada rodada, o script reduz o cache a 20 GB (`RAVENRS_XCACHE_MAX` muda o limite), apagando primeiro o que foi usado há mais tempo, porque todas as worktrees enchem a mesma pasta, `~/.cache/ravenrs/xcache`. É a primeira versão e guarda só bibliotecas. O que passa direto, e quanto custou, aparece nas estatísticas ao fim do check. Medido na mesma data: 14m15s numa worktree em caminho novo, com o cache cheio. O modo também desliga a compilação incremental, o que sozinho levou a rodada de cache vazio de 31m04s para 23m01s. Para saber por que um crate não veio do cache: `ravenrs-xcache explain <crate>`; para comparar do que duas chaves foram feitas: `RAVENRS_XCACHE_TRACE=<pasta>`. O README do crate tem o desenho, os limites e o plano.

**No Linux também (#1023).** Medido em 27/11/2026 na macpro-i7 (Fedora, 8 threads, `CARGO_BUILD_JOBS=6`), num check rápido depois de uma mudança no `ravenrs-widgets-base`: 32m38s com o cache vazio e **17m04s** com o diretório de build vazio e o cache cheio (1.018 de 1.018 bibliotecas do cache). Com `RAVENRS_XCACHE=0`, a mesma situação levava de 42 a 60 minutos. Não desligue o xcache nos roteiros. Os membros do workspace que o `cargo clippy` não analisa (não estão na linha de comando) vão ao `rustc` e vêm do cache; os analisados e os proc macros ainda compilam toda vez.

**Onde rodar.** O `cargo xtask pr` roda na máquina de quem desenvolve, a mesma em que o PR foi feito. Não há CI hospedado nem servidor de checagens.

**Por que não o sccache.** Medido na mesma data: ele põe toda variável `CARGO_*` na chave, e cada worktree usa um `CARGO_TARGET_DIR` diferente. O mesmo código num segundo diretório teve 0 acertos em 34, e no uso real a taxa em Rust era de 0,26%. Passando o diretório por `--target-dir`, o `cargo check --workspace` teve 382 acertos, 150 perdas e 218 não cacheáveis, e caiu só de 81 s para 67 s. Sem `--remap-path-prefix`, o `rustc` grava o caminho no artefato e recusa misturar artefatos de diretórios diferentes (`E0460`). Ele foi desligado em `~/.cargo/config.toml`.

**Nunca aponte duas worktrees para o mesmo diretório de build.** Reproduzido na mesma data: o Cargo respondeu `Finished` em 0,6 s com o artefato da outra worktree, sem a função que só existia na segunda.

## Quando falha

- **Teste que passa no `cargo test` e falha no nextest:** quase sempre é estado dividido entre binários, como arquivo fixo em disco, porta ou variável de ambiente, que agora rodam ao mesmo tempo. Corrija o teste (pasta temporária própria, porta 0) em vez de serializar. Para isolar um teste:

  ```bash
  cargo nextest run -p <crate> -E 'test(<nome>)'
  ```

  Para confirmar que é concorrência, rode de novo com `--test-threads=1`.
- **Teste lento (`SLOW` na saída do nextest):** um `#[test]` que percorre uma lista inteira num laço não se paraleliza. Divida em um teste por item, como foi feito com os filmes em `examples/family-finance/tests/suite/training.rs`, e mantenha um teste que garante que nenhum item ficou sem o seu.
- **Doctest "can't find crate", ou derive com código de outro branch:** é diretório de build compartilhado (seção acima).
- **`rts-coverage` (contrato do RTS, #736):** um tipo novo do `recursos.json` precisa de entrada em `tools/raven-test-studio/contrato.json` (completa, `sem_tela` ou `pendente` com a issue da família). A guarda também recusa `test_id` feito com `format!` fora de `parte:chave`, ou numerado pela posição, e ação listada que o código não nomeia (ou declarada com `on_access` e não listada). O checklist está em `CONTRIBUTING.md`, "Testability (RTS)". `scripts/rts-coverage.py --issues` também confere, pelo `gh`, se alguma issue de pendente já fechou; o `cargo xtask pr` não usa essa opção, porque roda sem rede.

## Mexer nas checagens

- O `release` não perde cobertura. O que sai do `pr` tem que continuar no `release`, e o que sai do `dev` continua no `pr`.
- Mudou o `changed-packages.py`? Acrescente o caso em `scripts/changed_packages_test.py` e rode `python3 scripts/changed_packages_test.py`. O Python do macOS é o 3.9, sem `tomllib`, então o leitor de TOML é próprio e mínimo.
- Meça antes e depois no mesmo PR de exemplo, com o mesmo diretório de build aquecido, e registre as duas tabelas de tempo (`last.txt`) no PR.
- Atualize junto o `xtask/src/main.rs` (a ajuda), o `CONTRIBUTING.md`, o `docs/QUALITY.md`, o `.github/PULL_REQUEST_TEMPLATE.md`, a seção 3 da skill `ravenrs-github` e esta skill.
- As dependências de terceiros compilam com `opt-level = 1` no perfil dev (`[profile.dev.package."*"]`). Os crates do workspace ficam sem otimização. Não tire isso sem medir os testes de UI e de banco.
