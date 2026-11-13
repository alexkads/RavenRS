---
name: ravenrs-triagem
description: Levanta tudo o que está parado no RavenRS (issues, PRs, branches, worktrees com trabalho não commitado, designs aprovados à espera, follow-ups esquecidos, disco), põe em ordem e separa entre o que a IA resolve agora, o que depende do Alex e o que vale uma sessão própria. Use quando o Alex disser "mata pendência", "matar pendências", "o que está parado?", "o que falta?", "faxina", "triagem", "o que eu faço agora?", ou pedir uma visão geral do que está aberto, mesmo sem citar a palavra triagem.
---

# Triagem do RavenRS

Trabalho pronto e esquecido custa mais que trabalho por fazer: um PR que só espera merge, uma worktree com horas de código não commitado, um design aprovado que ninguém começou. Esta skill acha essas coisas, põe em ordem e diz o próximo passo de cada uma. O Alex decide só o que é dele.

## 1. Levantar (em paralelo)

Dispare subagentes de leitura com modelo mais barato (Sonnet), um por grupo de fontes. Cada um devolve linhas no formato `tipo | referência | idade | estado | próximo passo provável`. Nenhum deles altera nada.

| Grupo | Como olhar |
|---|---|
| Issues | `gh issue list --state open --limit 200 --json number,title,labels,updatedAt,body`: sem atividade há mais de 7 dias; issue rasa, sem critérios de aceite; issue que um PR já juntado resolveu sem fechar |
| PRs | `gh pr list --state open --json number,title,headRefName,baseRefName,mergeable,isDraft,updatedAt`: mergeável e parado; em conflito; empilhado sobre um branch que já entrou |
| Branches | `git branch -r --no-merged origin/master` sem PR aberto; branches locais já juntados |
| Worktrees | `git worktree list`; em cada uma, `git -C <wt> status --short` e `git -C <wt> log origin/master..HEAD --oneline`: trabalho não commitado, commits sem push, PR já juntado |
| Memória | o `MEMORY.md` do projeto: itens com "aguarda", "pausada", "implementar quando ele mandar", "falta", "pendente" |
| Follow-ups | PRs juntados nos últimos 30 dias (`gh pr list --state merged --limit 50 --json number,body`): seções "Limitations"/"não feito" e "próximo passo" que não viraram issue |
| Disco | `df -h /System/Volumes/Data`; `du -sh /private/tmp/claude-501/*/*`; o `target` de cada worktree |

Antes de usar o que um subagente devolveu, confira os itens que pesam: número, estado atual e se outra sessão já resolveu.

## 2. Priorizar

Ordem, de cima para baixo, e dentro de cada nível o mais antigo primeiro:

1. **Risco de perder trabalho:** worktree com mudança não commitada ou commit sem push.
2. **Destrava outros:** PR em que outros estão empilhados, issue citada como dependência.
3. **Quase pronto:** PR mergeável parado, issue cujo PR já entrou, branch com trabalho feito e sem PR.
4. **Prioridade declarada pelo Alex:** o que a memória marca como prioridade (hoje: Reporting e o ravenrs-app low-code).
5. **Decisões paradas:** design aprovado à espera da ordem dele, pergunta sem resposta.
6. **O resto:** issues grandes sem dono, por idade.

## 3. Separar em três colunas

- **Resolvo agora:** o que tem um passo claro e reversível. Por exemplo: completar uma issue rasa com o que o código mostra, fechar uma issue que um PR juntado resolveu, rebasear um PR em conflito, limpar uma worktree e um branch de PR já juntado, apagar um `target` desta sessão.
- **Precisa do Alex:** o que só ele decide ou faz. Por exemplo: merge (regra: merge só com o ok dele), aprovar um design, testar no Windows ou no Linux, escolher entre caminhos, publicar.
- **Vale uma sessão:** trabalho de horas que termina num PR próprio. Proponha cada um com `start_session` ("own_initiative"), com um brief completo.

## 4. Responder

Uma tabela por coluna, com link para cada item (`[#N](https://github.com/alexkads/RavenRS/issues/N)`), a idade e o próximo passo em uma frase. Depois:

- na coluna "Resolvo agora", pergunte uma vez se pode executar tudo (botão), ou execute direto o que for só limpeza segura;
- na coluna "Precisa do Alex", faça uma pergunta por item, no máximo três, as mais importantes primeiro;
- termine com o espaço em disco antes e depois, se houve limpeza.

## Limites (não negociáveis)

- **Nunca apague trabalho não commitado.** Uma worktree assim é só apontada, com o que tem nela.
- **Nunca faça merge sem o ok explícito do Alex** (exceção: PRs do Matheus, conforme a memória).
- **Coisa de outra sessão ativa** (processo `cargo`/`rustc` rodando, arquivos escritos na última hora) é só informada.
- **Item sem informação suficiente vira pergunta, não issue rasa** (regra global de work items).
- **Branch remoto só é apagado com ok.**
