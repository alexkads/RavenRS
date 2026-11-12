# Primeiro app com ravenrs-app no VS Code — RavenDevLab

Tutorial em português brasileiro: um cadastro de tarefas com navegação, lista,
formulário e validação gerados de uma struct Rust. Versão usada: **27.1.14**.
Os dados deste exemplo ficam em memória e são perdidos quando o app é fechado.

## Arquivos para publicar

Execute `build_video.py` para gerar em `output/`:

- `ravendevlab-primeiro-app.mp4`: vídeo 1920 × 1080, H.264, áudio AAC, 30 fps.
- `legendas.srt`: legendas em português, segmentadas por frase.
- `thumbnail.jpg`: capa 1280 × 720.
- `youtube.md`: título, descrição e capítulos com os tempos do vídeo final.
- `roteiro.md`: fala completa e a cena correspondente.
- `minhas-tarefas.zip`: projeto completo para acompanhar a aula.
- `tarefas.ravenmovie`: demonstração nativa gravada pelo Recorder do RavenRS.

As telas do app são renderizadas pelo RavenRS a partir de uma execução real.
As cenas do editor são uma representação didática do VS Code, identificada no
vídeo; não são uma captura do programa aberto. A voz é sintética, fornecida pelo
macOS. Não há música de terceiros. A geração não publica nada no YouTube.

## Acompanhar a aula

1. Instale Rust por [rustup](https://rustup.rs/) e os pré-requisitos nativos da
   sua plataforma. Use Rust 1.88 ou superior. No macOS, as ferramentas de linha de
   comando do Xcode são necessárias; no Windows, use a toolchain MSVC e o workload
   de C++ do Visual Studio Build Tools. Linux também precisa das dependências de
   janela, áudio e diálogos usadas pelo RavenRS; confira o guia do framework.
2. Instale o [VS Code](https://code.visualstudio.com/) e a extensão
   [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
3. Confira `rustc --version` e `cargo --version` em um terminal novo.
4. Em uma pasta fora de outro workspace Rust, crie o projeto:

   ```sh
   cargo new minhas-tarefas
   cd minhas-tarefas
   code .
   ```

   Se `code` não estiver no PATH, use **Arquivo → Abrir Pasta** no VS Code.
5. No terminal integrado (**Terminal → Novo Terminal**), adicione as bibliotecas:

   ```sh
   cargo add ravenrs@=27.1.14 ravenrs-app@=27.1.14 ravenrs-app-ui@=27.1.14
   ```

6. Substitua `src/main.rs` pelo conteúdo de [projeto/src/main.rs](projeto/src/main.rs).
7. Execute:

   ```sh
   cargo run
   ```

   A primeira compilação baixa e compila as dependências. Aguarde o terminal abrir
   a janela. É necessário um ambiente gráfico com GPU/backend compatível com wgpu.
8. Clique em **Novo** e depois em **Salvar** sem preencher o título. O app mostra
   **Título é obrigatório**. Digite **Publicar meu primeiro app**, clique em
   **Salvar e fechar** e confira a tarefa na lista. O campo **Concluída** é um
   booleano e pode ser marcado no formulário.

Também é possível abrir diretamente a pasta `projeto` no VS Code e executar
`cargo run`. As dependências estão fixadas para acompanhar esta aula.

## O que cada parte faz

| Código | Efeito |
| --- | --- |
| `#[derive(BusinessObject)]` | Descreve a entidade para o framework |
| `#[object(...)]` | Define o nome e o grupo da navegação |
| `#[field(key)]` | Marca a chave do registro |
| `caption = "Título"` | Define o rótulo visível |
| `default_property` | Define o campo usado para representar o registro |
| `required` | Valida o preenchimento ao salvar |
| `done: bool` | Gera um editor de valor booleano |
| `.entity::<Task>()` | Registra a entidade na aplicação |
| `.language(Language::BrazilianPortuguese)` | Traduz os comandos do framework |
| `.into_view()` | Constrói a interface gerada |
| `ravenrs::native::run(...)` | Abre a janela nativa |

`ravenrs-app` descreve o modelo; `ravenrs-app-ui` gera as telas; `ravenrs` fornece
a interface e a execução nativa. SQLite pode ser acrescentado em uma próxima aula.

## Reproduzir a produção do vídeo

Na raiz do repositório:

```sh
cargo run -p tutorial-tasks --example record -- docs/tutorials/ravenrs-app-vscode/output
python3 -m venv docs/tutorials/ravenrs-app-vscode/.venv
docs/tutorials/ravenrs-app-vscode/.venv/bin/pip install -r docs/tutorials/ravenrs-app-vscode/requirements.txt
docs/tutorials/ravenrs-app-vscode/.venv/bin/python docs/tutorials/ravenrs-app-vscode/build_video.py
```

A narração usa `say`, com uma voz pt-BR disponível no macOS. Por padrão, Luciana.
Para trocar: `RAVEN_VIDEO_VOICE='Eddy (Português (Brasil))' .../python build_video.py`.
A voz escolhida deve estar instalada. O FFmpeg é fornecido por `imageio-ffmpeg`.
O script guarda narração, imagens e segmentos intermediários em `output/`.

Para verificar o exemplo sem abrir uma janela:

```sh
cargo clippy -p tutorial-tasks --all-targets -- -D warnings
cargo run -p tutorial-tasks --example record -- docs/tutorials/ravenrs-app-vscode/output
```

O gravador verifica a mensagem de validação e o registro salvo antes de exportar
as telas usadas no vídeo. A pasta `output/` é ignorada pelo Git.

Fontes: [documentação do ravenrs-app](https://docs.rs/ravenrs-app/27.1.14/ravenrs_app/),
[guia do framework](https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md),
[Rust no VS Code](https://code.visualstudio.com/docs/languages/rust).
