<!-- ravenrs-lint:start -->
## Qualidade de código: inegociável

Este projeto usa o RavenRS e é verificado pelo `ravenrs-lint`, pelo rustc e pelo Clippy no modo mais rigoroso. Uma tarefa só está pronta quando este comando passa, sem erros e sem avisos:

```sh
cargo ravenrs-lint --gate
```

Ele roda `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, as regras do RavenRS e `cargo test`. Rode-o antes de dizer que terminou. Se o `cargo ravenrs-lint` não estiver instalado, instale com `cargo install ravenrs-lint --locked`.

### Regras

- Quando uma verificação falhar, corrija o código como diz o `help` do diagnóstico. Não silencie.
- Nunca acrescente `#[allow(...)]`, nunca baixe um nível no `Cargo.toml` ou no `clippy.toml` e nunca escreva um comentário `ravenrs-lint: allow(...)` para fazer uma verificação passar. Se uma exceção for mesmo correta, use `#[expect(lint, reason = "...")]` ou `// ravenrs-lint: allow(regra) motivo`, e avise na resposta.
- Nunca use `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!` ou índice que possa estourar na aplicação. Devolva `Result` e mostre o erro para a pessoa na interface.
- Nada de `println!`, `eprintln!` ou `dbg!` no código.
- Cor só por papel `Semantic`, texto só por `TextStyle`. Nunca `Color` literal nem `font_size`.
- Padding, espaçamento e gap são múltiplos de 4: tela 32, card 24, pilha 16.
- Um botão principal por tela; os outros recebem `.style(ButtonStyle::Secondary)` ou `Ghost`. Rótulos começam com verbo no infinitivo.
- Todo `IconButton` tem `.label(...)`.
- `State` fica num campo do componente, nunca é criado em `body`. `body` e `build` nunca leem arquivo, esperam ou chamam a rede.
- Arquivos e funções pequenos: divida o arquivo antes de 600 linhas e a função antes de 80.
- Antes de usar uma API do RavenRS, confirme-a na documentação ou no código-fonte; não adivinhe nomes de métodos. Se faltar algo no framework, diga isso em vez de montar uma imitação.
- Toda mudança de comportamento vem com um teste que a exercita, pela `Ui` no caso da interface.
<!-- ravenrs-lint:end -->
