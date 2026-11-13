---
name: ravenrs-check
description: Como rodar, ler e manter o scripts/check.sh, o gate oficial do RavenRS (não há CI hospedado) - modo rápido por PR, --full antes de publicar, cargo-nextest obrigatório, um CARGO_TARGET_DIR por worktree, o que o seletor de pacotes testa e o que deixa para o --full. Use sempre que for validar um PR ou branch, rodar ou interpretar o check.sh, investigar teste que falha só no check, um check lento ou "can't find crate", instalar o ambiente de quem contribui, ou mexer no check.sh, no changed-packages.py ou nos perfis do Cargo.toml, mesmo que o pedido não cite esta skill.
---

# O check.sh do RavenRS

O projeto não tem CI hospedado: `scripts/check.sh` é o gate. Todo PR registra em "How it was tested" que ele passou e o tempo que imprimiu.

## Antes da primeira vez

**O `cargo-nextest` é obrigatório.** Decisão do Alex em 12/11/2026: velocidade, e todo mundo usa. Sem ele, o script para antes de compilar e mostra o comando:

```bash
cargo install cargo-nextest --locked
```

Vale para todas as máquinas: a do Alex, a do Matheus, as de Linux e Windows, e as worktrees dos agentes. Para conferir, rode `cargo nextest --version`. O nextest roda todos os binários de teste ao mesmo tempo, enquanto o `cargo test` roda um depois do outro, e põe cada teste num processo próprio. Os doctests ele não roda, e por isso o script os passa ao `cargo test --doc`.

## Rodar

```bash
./scripts/check.sh
```

Esse é o modo rápido, para todo PR. Ele compara com `origin/master`; para mudar a base, use `CHECK_BASE=<ref>`.

```bash
./scripts/check.sh --full
```

O `--full` roda tudo: o workspace inteiro, wasm32, as features opcionais, o MSRV, o `cargo deny`, a documentação e os builds web. Ele vai antes de publicar uma versão ou quando o Alex pedir. Os benchmarks e o gate de desempenho só entram quando ele pedir, com `RAVENRS_GATE=1` (skill `ravenrs-desempenho`).

## O que o modo rápido escolhe

Quem escolhe é o `scripts/changed-packages.py`. Os passos, em ordem:

1. **rustfmt** no workspace inteiro.
2. **Conferências baratas do workspace inteiro**, sempre: ciclos de módulos (`ravenrs-lint --only module_cycle`), camadas dos crates de widgets (`check-widget-layers.py`) e arquivos de teste declarados em `tests/suite/main.rs` (`check-test-suites.py`).
3. **O teste do seletor** (`scripts/changed_packages_test.py`). O que o seletor deixa de fora não é conferido, por isso ele tem testes próprios.
4. **ravenrs-lint** nos exemplos, só quando muda o crate do lint, o `family-finance`, o `component-gallery` ou a configuração no `Cargo.toml`.
5. **API pública do `ravenrs-widgets`** (`public-api.py --check`, rustdoc no nightly), só quando a mudança alcança a fachada `ravenrs-widgets`.
6. **clippy `--all-targets`** em tudo o que a mudança alcança, em qualquer profundidade. Isso também prova que esses pacotes compilam.
7. **testes** (nextest e doctests) só dos pacotes alterados e dos dependentes diretos. Os dependentes mais distantes ficam para o `--full`.

| Mudança | Alcança |
|---|---|
| Arquivo dentro de um crate | o crate, e os dependentes dele |
| Crate novo | só ele (membro novo no `Cargo.toml` e no `Cargo.lock` não vira `ALL`) |
| Versão do workspace ou de crate interno | os crates que usam `CARGO_PKG_VERSION` |
| Entrada de `[workspace.dependencies]` | os crates que usam essa dependência |
| Crate externo atualizado no `Cargo.lock` | os membros que chegam nele pelo grafo do lock |
| `[workspace.metadata]` | só o lint |
| `[profile.*]`, `[workspace.lints]`, edition, resolver, `[patch]`, `.cargo/`, toolchain, `clippy.toml` | `ALL`: workspace inteiro |
| `rustfmt.toml`, `deny.toml` | nada a mais (o rustfmt já cobre tudo, e o deny é do `--full`) |

Para ver a escolha sem compilar nada:

```bash
./scripts/changed-packages.py --shell
```

## Um CARGO_TARGET_DIR por worktree

O Cargo não põe o caminho de um crate local no hash dos artefatos. Por isso, duas worktrees que dividem o mesmo `CARGO_TARGET_DIR` reaproveitam o build uma da outra. Isso já deu dois sintomas:

- "can't find crate for `ravenrs`" num doctest;
- um derive compilado com um `ravenrs-app-spec` antigo, de outro branch.

Na worktree, use um diretório próprio no scratchpad da sessão, sempre por caminho absoluto, e apague-o ao terminar:

```bash
CARGO_TARGET_DIR=/private/tmp/claude-501/<projeto>/<sessão>/scratchpad/target ./scripts/check.sh
```

O script grava a pasta da checkout em `<target>/.ravenrs-checkout` e avisa em amarelo quando o diretório foi usado por outra. Diante desse aviso, ou de um dos sintomas acima, troque o diretório antes de culpar o código. Um diretório de build passa fácil de 20 GB: confira o disco antes (`df -h /System/Volumes/Data`).

## Quando falha

- **Teste que passa no `cargo test` e falha no nextest:** quase sempre é estado dividido entre binários, como arquivo fixo em disco, porta ou variável de ambiente, que agora rodam ao mesmo tempo. Corrija o teste (pasta temporária própria, porta 0) em vez de serializar. Para isolar um teste:

  ```bash
  cargo nextest run -p <crate> -E 'test(<nome>)'
  ```

  Para confirmar que é concorrência, rode de novo com `--test-threads=1`.
- **Teste lento (`SLOW` na saída do nextest):** um `#[test]` que percorre uma lista inteira num laço não se paraleliza. Divida em um teste por item, como foi feito com os filmes em `examples/family-finance/tests/suite/training.rs`, e mantenha um teste que garante que nenhum item ficou sem o seu.
- **Doctest "can't find crate", ou derive com código de outro branch:** é diretório de build compartilhado (seção acima).

## Mexer no check.sh

- O `--full` não perde cobertura. O que sai do modo rápido tem que continuar no `--full`.
- Mudou o `changed-packages.py`? Acrescente o caso em `scripts/changed_packages_test.py` e rode `python3 scripts/changed_packages_test.py`. O Python do macOS é o 3.9, sem `tomllib`, então o leitor de TOML é próprio e mínimo.
- Meça antes e depois no mesmo PR de exemplo, com o mesmo diretório de build aquecido, e registre os dois tempos no PR.
- Atualize junto o cabeçalho do script, o `CONTRIBUTING.md`, o `docs/QUALITY.md`, o `.github/PULL_REQUEST_TEMPLATE.md`, a seção 3 da skill `ravenrs-github` e esta skill.
- As dependências de terceiros compilam com `opt-level = 1` no perfil dev (`[profile.dev.package."*"]`). Os crates do workspace ficam sem otimização. Não tire isso sem medir os testes de UI e de banco.
