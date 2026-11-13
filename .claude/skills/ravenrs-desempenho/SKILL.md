---
name: ravenrs-desempenho
description: Roteiro completo para testar o desempenho gráfico do RavenRS - gate de contadores exatos, tempos por fase, frames frios e regressões plantadas. Use somente quando o Alex pedir para testar, medir ou conferir desempenho, FPS, lentidão, travamento, o gate ou "rodar os testes de performance". Nada aqui roda por conta própria, nem ao mexer no core, em componente ou antes de publicar.
---

# Desempenho do RavenRS

Tudo aqui se repete com um comando. A regra é medir antes de otimizar, e nunca aceitar uma mudança sem o número de antes e o de depois.

**Só quando o Alex pedir.** Nenhum passo desta skill é regra fixa. Não rode o gate (nem tempos, frames frios, capacidade ou regressões plantadas) por iniciativa própria: nem depois de mexer no core ou num componente, nem antes de abrir PR ou publicar. Quando uma mudança puder afetar desempenho, diga isso em uma linha e ofereça a medição; ela só roda se ele pedir. O `scripts/check.sh --full` também não roda o gate nem compila os benchmarks, a menos que seja chamado com `RAVENRS_GATE=1`.

Compile com `CARGO_TARGET_DIR` no scratchpad quando estiver num worktree, **um por worktree**. O Cargo não põe o caminho de um crate local no hash dos artefatos. Duas worktrees que dividem o mesmo diretório reaproveitam o build uma da outra: já deu "can't find crate for `ravenrs`" num doctest e um derive compilado com um `ravenrs-app-spec` antigo. Uma medição feita assim não vale. O `scripts/check.sh` avisa quando o diretório foi usado por outra checkout.

## Quando rodar o quê (quando ele pedir)

| Pedido | O que rodar |
|---|---|
| "Roda o gate" ou "confere se piorou" | Gate |
| Quero saber onde o tempo vai | Relatório e tempos |
| "Está travando ao abrir" | Frames frios |
| Quero ver numa tela real | Painel de diagnóstico |
| Mexi no renderer, ou a pergunta é sobre a GPU | Capacidade |
| "Mede tudo antes de publicar" | Gate, tempos, frames frios e o teste funcional no Linux |
| Mudei o próprio gate | Regressões plantadas |

## 1. Gate: contadores exatos

```bash
cargo run --release -p ravenrs-perf -- gate
```

Conta o trabalho de cada cena e compara com `perf/baseline.json`: builds, layouts, paints, shapings, alocações, draw calls, instâncias e bytes enviados à GPU. Roda duas vezes e reprova se as rodadas divergirem. Leva cerca de 40 s. Só entra no `scripts/check.sh --full` com `RAVENRS_GATE=1 ./scripts/check.sh --full`.

Se reprovar, descubra a causa antes de qualquer outra coisa. Só atualize a baseline se a mudança for intencional, e revise o diff:

```bash
cargo run --release -p ravenrs-perf -- gate --bless
```

Sem GPU, o gate falha de propósito. Fora do macOS, só os tetos reprovam; diferença de baseline vira aviso, porque a baseline é do Mac de referência.

## 2. Relatório de contadores

```bash
cargo run --release -p ravenrs-perf -- report
```

Aceita um filtro de cena, como `report curve`. Olhe primeiro:
- `instances` bem maior que o que cabe na tela: algo fora da tela está sendo desenhado.
- `builds` acima de 0 em hover ou rolagem: a interação refaz a tela inteira.
- `shapes` acima de 0 sem texto novo: falta cache de texto.
- `frames` acima de 0 em `idle`: o app não está dormindo.

Para comparar versões com uma tela pequena e estável, use a calculadora (`examples/calculator`), que é medida como está: `report calculator` e `time calculator`. Um clique nela muda um número só, então qualquer aumento ali é custo do core.

## 3. Tempos por fase

Na tomada, no Mac de referência (MacBook Pro 14" M2 Pro, `Mac14,9`). Na bateria, o resultado é inválido.

```bash
./scripts/perf-time.sh
```

Roda 3 processos, fica com o melhor p99 e compara com as metas de `docs/QUALITY.md`: CPU ≤ 4 ms em interação, ≤ 16,67 ms em rebuild e GPU ≤ 3 ms. As cenas `curve-large` são só mostradas, nunca reprovam. Tempo não entra no `check.sh`, porque é ruidoso em laptop.

O relógio de GPU do Metal é grosseiro: passes curtos costumam vir como 0. Compare ordens de grandeza, não décimos.

## 4. Frames frios

```bash
cargo run --release -p ravenrs-perf -- cold
```

Abre o app de finanças e visita cada página duas vezes, mostrando o pior frame de cada visita. O caixa ainda não entra nesta visita; veja a seção do caixa. Os tempos acima descartam o aquecimento, então é aqui que aparece o travamento ao abrir uma tela. Acima de 16,67 ms, a pessoa vê.

## 5. Regressões plantadas

Prova que o gate funciona: planta um rebuild, uma alocação e um shaping extras em `ui.rs`, um de cada vez, exige a reprovação e restaura o arquivo.

```bash
./scripts/perf-plant.py
```

Rode depois de `gate --bless`, porque ele compara com a baseline. Ele regrava `ui.rs` e o restaura no fim. Não edite esse arquivo enquanto o script roda.

## 6. Comparar antes e depois

```bash
cargo run --release -p ravenrs-perf -- time > antes.json
```

```bash
cargo run --release -p ravenrs-perf -- diff antes.json depois.json
```

Não meça tempo com a VM ligada ou com compilação rodando: os números dobram.

## 7. Painel de diagnóstico

Ctrl/Cmd+Shift+D em qualquer app, ou `RAVENRS_DIAG=1` no desktop. Mostra a linha do tempo dos frames, a causa de cada rebuild, as views mais caras, as quebras de draw call e os recursos da GPU, e copia uma captura para abrir no Perfetto. O guia de leitura é `docs/PERFORMANCE.md`. Os testes em `crates/ravenrs-widgets/tests/suite/performance_tools.rs` exigem que cada ferramenta aponte um defeito plantado.

## 8. Teste funcional no Linux

Só para saber se a aplicação continua funcionando, com janela de verdade num X11. Nada de gate ou tempo lá: é ARM64 sem GPU, desenhando em software com `lavapipe`.

```bash
./scripts/vm-linux.sh test family-finance
```

Liga a VM `ravenrs-linux`, instala o compilador, os pacotes `-dev` e o Rust, copia este checkout, compila, abre o app sob o `twm`, confirma que ele continua vivo depois de 30 s e salva uma captura da tela. Leva uns 10 minutos, porque compila do zero. Olhe a captura com a ferramenta de leitura de imagem, porque processo vivo não prova que desenhou. Repita com outros exemplos, como `feature-center` e `component-gallery`. O título da janela sai com acentos trocados: é o `twm`, não o app.

**Sempre termine com `stop`, mesmo depois de uma falha:**

```bash
./scripts/vm-linux.sh stop
```

Ele desliga a VM e volta o disco à base `limpa`, descartando tudo o que o teste trouxe: compilação, crates, Rust e pacotes. A base tem só o que roda um app e ocupa menos de 1 GB. Se o `stop` disser outro tamanho, algo ficou para trás.

Para mudar o que fica na base, edite `RUNTIME` ou `BUILD` no script e rode `seal`, que parte da base atual, limpa, compacta o disco e grava a base nova. Para recriar a VM do zero, rode `create` e depois `seal`. Se o último passo do instalador do VirtualBox falhar com uma tela vermelha, envie Enter com `VBoxManage controlvm ravenrs-linux keyboardputscancode 1c 9c`, espere o login e rode `provision`. A senha é só dessa VM e fica em `~/VirtualBox VMs/ravenrs-linux.password`; o acesso diário é pela chave `ravenrs-linux.key`, na mesma pasta.

Windows continua só nas máquinas do Alex.

## 9. Capacidade da GPU

```bash
cargo run --release -p ravenrs-perf -- capacity
```

Aumenta cada tipo de trabalho até o frame não caber, contando até a GPU terminar, e diz quantos cabem a 120 e a 60 Hz. Com `--curve`, mostra cada tamanho medido. Serve para saber onde o renderer vira gargalo, e é o comando a rodar em outras máquinas, porque imprime a GPU usada. Na tomada, sem nada mais rodando. Se aparecer "leaves shapes out", formas estão sendo descartadas: é defeito, não lentidão.

## Componente novo

Todo componente entra numa cena em `crates/ravenrs-perf/src/scenes/`, com `covers` listando o nome dele, no pior caso realista. Rodar `gate --bless` e revisar o diff fica para quando ele pedir a medição. O contrato completo está em `docs/QUALITY.md`.

## Caixa e etiquetas

A cena `counter-keys` (em `crates/ravenrs-perf/src/scenes/components.rs`) cobre `KeyBar` e `ScanField`. O caixa e a tela de etiquetas ainda não têm cena no gate: entram em `scenes/apps.rs`, como as outras telas de aplicação.

O caixa é a tela usada o dia todo, então o que importa é `builds` e `shapes` a cada item bipado: não podem crescer com o tamanho da venda. Para ver a tela sem janela, `RAVEN_PDV_TELAS=pasta cargo test -p mini-pdv --test telas -- --ignored` gera os PNG.

Não foi medido: as telas do caixa e de etiquetas, tempo em máquina de caixa real, a prévia de etiqueta rasterizada com muitas etiquetas, e impressão em impressora física.

## O que relatar ao terminar

- Os números de antes e depois, nunca só "melhorou".
- O que não foi medido.
- Se a baseline mudou, qual contador mudou e por quê.
