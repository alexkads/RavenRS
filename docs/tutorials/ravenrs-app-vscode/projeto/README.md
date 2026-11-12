# Minhas tarefas — RavenDevLab

Primeiro app com Rust e ravenrs-app no VS Code.

1. Instale Rust (1.88 ou superior) e as ferramentas de compilação da sua plataforma.
2. Abra esta pasta no VS Code e instale a extensão `rust-lang.rust-analyzer`.
3. Em **Terminal → Novo Terminal**, execute:

   ```sh
   cargo run --locked
   ```

4. Clique em **Novo** e tente **Salvar** sem preencher o título para ver a validação.
5. Preencha **Título**, opcionalmente marque **Concluída**, e clique em
   **Salvar e fechar**. Confira o registro na lista.

O exemplo guarda os registros em memória: eles desaparecem ao fechar a aplicação.

O arquivo `src/main.rs` contém todo o app. `Cargo.toml` fixa as três bibliotecas
principais em 27.1.14; `Cargo.lock` registra também as versões das dependências
indiretas verificadas na produção da aula.

Canal: [RavenDevLab](https://www.youtube.com/@RavenDevLab).
Framework: [RavenRS](https://github.com/alexkads/RavenRS).
