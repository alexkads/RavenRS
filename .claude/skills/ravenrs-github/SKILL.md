---
name: ravenrs-github
description: Fluxo de trabalho do RavenRS no GitHub - toda mudança nasce numa issue, vai num branch com pull request que a fecha, passa pelo check.sh local e termina nas novidades citando as issues. Use sempre que for começar uma tarefa, corrigir um bug, revisar ou avaliar o PR de alguém, abrir, juntar ou descrever um PR, registrar o que ficou de fora, preparar novidades ou publicar uma versão, mesmo que o pedido não cite GitHub, issue ou PR.
---

# Fluxo do GitHub no RavenRS

Adotado em 2026-11-06, depois do primeiro PR de contribuidor (#1). Vale para todo mundo: quem mantém, quem contribui e os agentes.

O ciclo é este: **issue → branch → PR que fecha a issue → revisão → merge → novidades citando a issue**.

## 1. Comece pela issue

Antes de escrever código, abra a issue em https://github.com/alexkads/RavenRS/issues, ou use a que já existe:

```bash
gh issue create --title "<o que muda>" --body-file issue.md
```

O corpo diz **o que** e **por quê**, e lista o que fazer como caixas de marcar (`- [ ]`). Pedido grande vira várias issues pequenas, uma por entrega.

## 2. Branch e PR

- Um branch por issue, a partir do `master` atualizado. Os agentes usam o prefixo `claude/`.
- Commits no padrão da casa: `<tempo> - <tipo>(<escopo>): <descrição>`, em pt-BR.
- A descrição do PR segue `.github/PULL_REQUEST_TEMPLATE.md`: o que muda e por quê, como foi testado e as limitações. Ela cita `Closes #N` para o GitHub fechar a issue no merge.
- Explique o motivo de cada dependência nova ou atualizada (falha de segurança, plataforma, licença). Quem revisa precisa entender sem perguntar.

```bash
gh pr create --base master --head <branch> --title "<título>" --body-file pr.md
```

## 3. Validação sem CI hospedado

Não há GitHub Actions: a conta não pode usá-lo. A validação é local, e o resultado vai escrito em "How it was tested":

```bash
./scripts/check.sh
```

Esse é o rápido, para todo PR: rustfmt; clippy em todo pacote que a mudança atinge, em qualquer profundidade; e testes dos pacotes alterados e dos dependentes diretos (`scripts/changed-packages.py`, contra `origin/master`). `Cargo.toml` e `Cargo.lock` são lidos entrada por entrada, então crate novo ou bump de versão não viram o workspace inteiro. Os testes rodam no `cargo-nextest`, obrigatório (`cargo install cargo-nextest --locked`), e cada worktree usa o seu próprio `CARGO_TARGET_DIR`. Como rodar, ler e manter o gate está na skill `ravenrs-check`. O completo, com wasm32, features opcionais, MSRV, `cargo deny`, documentação e builds web, é `./scripts/check.sh --full`. Ele roda antes de publicar uma versão, ou quando o Alex pedir. Os benchmarks e o gate de desempenho só rodam quando ele pedir, com `RAVENRS_GATE=1` (skill `ravenrs-desempenho`). Linux e Windows ficam nas limitações, a menos que tenham sido rodados.

## 4. Revisão e merge

- Revise o PR de quem contribui: rode a branch, os testes e o fluxo de ponta a ponta antes de aprovar. Aponte cada problema com arquivo e linha.
- O merge só acontece depois da revisão, e por quem mantém o projeto. Quem é autor do PR não consegue aprovar o próprio PR no GitHub.
- Ajustes de revisão em PR já juntado vão num PR novo, que cita o anterior.

## 5. O que ficou de fora vira issue

Cada limitação que sobra (sem teste em outro sistema, serviço real não testado, passo manual) vira uma issue nova, linkada no PR. Nada fica só no texto.

## 6. Novidades citam as issues

No Raven Finanças, cada versão em `examples/family-finance/novidades.json` e no topo de `historico-de-novidades.json` tem:

- `contribuidores`: nomes de quem fez a versão, como aparecem em `git log --format=%an <versão anterior>..`;
- `issues`: os números das issues que a versão fecha. O diálogo de novidades mostra cada uma como link, em "Issues resolvidas".

O teste `the_three_release_files_agree` exige os dois campos. O campo `issues` vale a partir da 0.1.32.

Para listar as issues fechadas desde a versão anterior:

```bash
gh issue list --state closed --search "closed:>=<data da versão anterior>" --json number,title
```

## 7. Versão

A publicação segue `docs/VERSIONING.md`. O commit da versão do app é o último, num PR de versão próprio, e a tag anotada `vYY.R.P` fica nesse commit. O `cargo publish --workspace` é rodado por quem mantém o projeto.
