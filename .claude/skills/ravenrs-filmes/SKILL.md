---
name: ravenrs-filmes
description: Como gravar, conferir, editar e embutir os filmes de treinamento do RavenRS (ravenrs-movie) - roteiro em teste que roda o app de verdade, caixas que ensinam regra e consequência, narração gravada, câmera e cartões, o comando de regravar e o que conferir nos quadros. Use sempre que o pedido citar filme, treinamento, tutorial em vídeo, manual interativo, gravar a tela, .ravenmovie, Recorder, MoviePlayer, MovieEditor, MovieLibrary, ou quando uma tela mudar e o filme dela ficar velho, mesmo que não cite esta skill.
---

# Filmes de treinamento

Pedido do Alex em 01/11/2026: um componente que gera filme da tela, com editor, para manuais interativos. **A IA grava os filmes e os deixa embutidos no app.** Eles ficam dentro do RavenRS (`crates/ravenrs-movie`, reexportado como `ravenrs::movie`) para todo usuário treinar a equipe com eles.

## O que é um filme

Um filme não tem pixels nem áudio gravado da tela. Ele guarda o que o app desenhou: os mesmos retângulos e o mesmo texto que o renderer recebe. Por isso fica nítido em qualquer zoom, e um minuto custa kilobytes. Nunca proponha gravar vídeo em pixels nem áudio de microfone.

| Tipo | Para que serve |
|---|---|
| `Recorder` | Roda o app de verdade a partir de um roteiro. Clica pelos nomes que o leitor de tela vê, então não precisa de GPU nem de permissão do sistema |
| `Project`, `MovieEditor` | Edição sem renderizar: cortes, velocidade, cartões, chamadas, legendas, zoom, esmaecer, destaque do ponteiro |
| `MoviePlayer` | Toca com capítulos, barra de posição, clique e tecla audíveis, voz e passos que esperam o clique |
| `MovieLibrary` | Acha o momento do filme que responde uma pergunta |
| `Narration` | A fala gravada antes (skill `ravenrs-voz`) |

O caso real: seis filmes do Raven Finanças, cerca de 16 minutos, em `examples/family-finance/assets/treinamentos/*.ravenmovie`.

## Treinamento raso não ensina

Em 01/11 o Alex achou o primeiro resultado excelente, mas superficial e mudo. Ele espera as regras e as consequências de cada tela, não um passeio pelos botões. Em cada passo, diga o que o campo faz no resto do sistema: "Ela alimenta os orçamentos e os relatórios, então vale a pena acertar".

## O roteiro

O roteiro é uma função num teste, como as de `examples/family-finance/tests/suite/training.rs` (`overview`, `ledger`, `bills`, `agenda`, `planning`, `records`):

```rust
fn ledger(take: &mut Recorder) -> Result<(), RecordError> {
    take.intro("Lançar uma despesa", "Lançamentos guarda tudo o que entrou e saiu. ...");
    take.guide("Lançamentos", "Abra os Lançamentos", "Clique em Lançamentos, onde fica ...")?;
    take.click("Lançamentos")?;
    take.wait(0.8);
    take.explain("Conta", "De onde saiu o dinheiro", "Escolha a conta, ... Ela alimenta os orçamentos ...")?;
    take.type_into("Descrição", "Mercado da semana")?;
    Ok(())
}
```

- `intro` e `chapter` abrem uma parte. `guide` mostra a caixa e espera o clique de quem assiste. `explain` e `note` explicam sem parar. `click`, `type_into`, `press`, `scroll` e `move_to` agem.
- O nome passado é o nome acessível do controle. Um nome que não existe mais quebra o roteiro com `RecordError`, e o teste diz qual filme não cabe mais no app.
- Cada filme entra na lista `FILMS` com o título mostrado no app, o arquivo e o roteiro, e ganha uma despedida em `FAREWELLS`.
- O app gravado começa sempre do mesmo jeito: banco em memória, `seed::load_demo` e uma data fixa.
- `edit` põe por cima a câmera que vai até cada passo, o destaque do ponteiro, os cartões de abertura e despedida e o esmaecer.

## Gravar

```bash
FF_RECORD_TRAINING=1 cargo test -p family-finance --test suite training::
```

Sem a variável, o mesmo teste só confere os filmes que estão no app. Depois de gravar:

1. **Grave a narração do texto novo** (skill `ravenrs-voz`, `FF_NARRATION_ADD=1`). Sem isso, o texto novo sai com a voz do sistema.
2. **Rode os testes de novo sem a variável.** `the_voice_and_the_boxes_go_together_from_the_opening_to_the_farewell` confere que nenhuma caixa sai antes da voz dela e que todo filme abre com o título e termina com a despedida. `the_films_in_the_app_open_and_teach_each_part` e `every_film_has_its_script_test` conferem o resto.
3. **Olhe os quadros.** Um nome achado não prova que o clique fez o que devia. Abra o filme no editor (`cargo run -p movie-editor -- <arquivo>.ravenmovie`) ou no próprio app e confira a tela depois de cada passo.

## Tempo da fala

A caixa dura o tempo da gravação dela. Sem gravação, a voz do sistema fala título e texto a cerca de 0,071 s por letra, e o `Recorder` reserva 0,075 s. Texto longo demais segura a tela parada: prefira duas caixas a uma muito longa.

## Embutir no app

O app traz os `.ravenmovie` e o `narracao.voz` com `include_bytes!`, monta uma `MovieLibrary` com `library.film(título, Movie::from_bytes(..))` e mostra o `MoviePlayer`. O modelo está em `examples/family-finance/src/ui/training.rs`.

## O que não fazer

- Não grave pixels, tela do sistema nem microfone.
- Não faça roteiro só de cliques: cada tela leva a regra e a consequência.
- Não use um assistente semântico para achar o momento do filme: foi medido e recusado. A busca é textual (memória `assistente-treinamento-textual`).
- Não embrulhe o player no app para ele ficar bonito. O que faltar vira recurso do `ravenrs-movie` (regra "a casa segue o padrão").
