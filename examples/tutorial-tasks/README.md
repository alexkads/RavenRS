# Minhas tarefas — tutorial RavenDevLab

Um cadastro em português com `ravenrs-app`: título obrigatório e conclusão.
Os registros ficam em memória.

```sh
cargo run -p tutorial-tasks
```

O material da aula, a versão independente para abrir no VS Code e o gerador do
vídeo estão em [docs/tutorials/ravenrs-app-vscode](../../docs/tutorials/ravenrs-app-vscode/README.md).

Para gravar a demonstração nativa e exportar as telas reais usadas no vídeo:

```sh
cargo run -p tutorial-tasks --example record -- docs/tutorials/ravenrs-app-vscode/output
```

O gravador confere o erro de título obrigatório e a tarefa salva na lista.
