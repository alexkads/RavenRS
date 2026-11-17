# Raven Data Studio

Documentação de produto e arquitetura de um módulo do RavenRS para **importar, transformar e modelar dados com IA**, integrado ao `ravenrs-app`.

> **Situação:** só documentação. Nada aqui foi implementado. Componentes, integrações, telas e motor ficam para tarefas futuras, que devem começar pela decisão D-03. O desenho das telas já foi aprovado (D-09).

## Revisão analisada

| | |
|---|---|
| Branch / commit | `master` @ `0a990af3ffa5f3ab9c8296f924ffc2375ccbe473` |
| Data | 15/11/2026 |
| Modo | Somente leitura do código; nenhum build, teste ou dependência instalada |
| Atualização | Antes do PR, o `master` chegou a `513577b5d` com o PR #389 (desfazer, JSON e portas pelo teclado no `FlowCanvas`); os documentos foram corrigidos nesses pontos ([02-analise-do-codigo.md](02-analise-do-codigo.md#depois-da-revisão-analisada)) |

## Como ler

1. Comece pelo [cenário de referência](cenario-de-referencia.md): é a fonte de todos os números.
2. Leia a [visão e os requisitos](01-visao-e-requisitos.md) para entender o que o produto faz.
3. Para decidir a implementação, leia a [análise do código](02-analise-do-codigo.md), a [matriz de componentes](03-matriz-de-componentes.md) e as [lacunas](04-lacunas-e-novos-componentes.md).
4. Para desenhar ou construir telas, leia os [fluxos](05-fluxos.md), as [telas](06-telas.md) e os [wireframes](07-wireframes.md).
5. Para construir o núcleo, leia a [arquitetura](08-arquitetura.md), o [modelo de domínio](09-modelo-de-dominio.md), [MCP e conectores](10-mcp-e-conectores.md), [schema, mapeamento e transformações](11-schema-mapeamento-transformacoes.md) e [execução e recuperação](12-execucao-e-recuperacao.md).
6. Antes de entregar qualquer parte, confira [segurança, privacidade, acessibilidade e idiomas](13-seguranca-privacidade-a11y-i18n.md) e os [critérios de aceitação](15-criterios-de-aceitacao.md).
7. A ordem de entrega está no [roadmap](14-referencias-e-roadmap.md). O que falta decidir está nas [decisões em aberto](16-decisoes-em-aberto.md).

## Documentos

| Arquivo | Conteúdo |
|---|---|
| [cenario-de-referencia.md](cenario-de-referencia.md) | Origem, destino, os 12 casos, contagens e registros de amostra |
| [01-visao-e-requisitos.md](01-visao-e-requisitos.md) | Visão, perfis, princípios, RF, RNF, fora do escopo, hipóteses |
| [02-analise-do-codigo.md](02-analise-do-codigo.md) | Revisão examinada, achados, limitações, persistência, expressões, IA |
| [03-matriz-de-componentes.md](03-matriz-de-componentes.md) | Requisito × componente × caminho × evidência × adequação × limitação × evolução |
| [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md) | Nomes propostos avaliados, N-1 a N-5, E-1 a E-11, M-1 a M-8, fronteira grafo × importação |
| [05-fluxos.md](05-fluxos.md) | Fluxo guiado, conversa, invalidação, execução e retomada |
| [06-telas.md](06-telas.md) | As 16 telas com componentes, ações, estados, acessibilidade e critérios |
| [07-wireframes.md](07-wireframes.md) | Wireframes SVG e interações passo a passo ([wireframes/](wireframes/)) |
| [08-arquitetura.md](08-arquitetura.md) | Camadas, crates, fluxo de dados, integração com o ravenrs-app |
| [09-modelo-de-dominio.md](09-modelo-de-dominio.md) | Entidades, máquinas de estado, invalidação, privacidade |
| [10-mcp-e-conectores.md](10-mcp-e-conectores.md) | As seis peças (MCP de desenvolvimento a motor) e os contratos |
| [11-schema-mapeamento-transformacoes.md](11-schema-mapeamento-transformacoes.md) | Descoberta, tipos, DDL, mapeamento e regras do cenário |
| [12-execucao-e-recuperacao.md](12-execucao-e-recuperacao.md) | Fases, lotes, idempotência, cancelar e retomar, rejeitados |
| [13-seguranca-privacidade-a11y-i18n.md](13-seguranca-privacidade-a11y-i18n.md) | Segredos, IA, permissões, auditoria, acessibilidade, idiomas, desempenho |
| [14-referencias-e-roadmap.md](14-referencias-e-roadmap.md) | Referências oficiais e decisões ligadas; MVP, Evolução, Avançado |
| [15-criterios-de-aceitacao.md](15-criterios-de-aceitacao.md) | Critérios verificáveis por requisito |
| [16-decisoes-em-aberto.md](16-decisoes-em-aberto.md) | D-01 a D-15 com recomendação |

## Classificação usada para os componentes

| Código | Significado |
|---|---|
| **R** | Existente e diretamente reutilizável |
| **C** | Existente, mas exige composição ou adaptação |
| **E** | Existente, mas precisa evoluir |
| **N** | Novo componente reutilizável (entra antes na biblioteca) |
| **M** | Nova funcionalidade específica do módulo |

## Em uma página

- **O plano é o produto.** Assistente, grade, canvas e conversa editam o mesmo `MappingPlan` versionado, e só por `PlanPatch` validado pelo domínio.
- **A IA propõe, o domínio valida, o usuário revisa, o motor executa.** A IA não tem ferramenta de escrita.
- **Grade antes do canvas.** O `DataGrid` remoto já atende o MVP. O canvas vem sobre o `FlowCanvas` existente, depois da continuação dele (#238).
- **Faltam três componentes genéricos:** `SchemaDiagram`, `TextDiff` e `IssueList`, além de `CodeLanguage::Sql`.
- **A persistência do ravenrs-app não é motor de carga.** Ela guarda os projetos. A carga vai pelos adaptadores, em lotes, com checkpoint e idempotência.
- **Sem rollback global.** Cada lote é atômico, e a execução é retomável e idempotente.

## Hipóteses principais

PostgreSQL primeiro (H-01). IA local primeiro, com o produto funcionando sem ela (H-02). Amostra de 200 linhas (H-03). Até cerca de 1 milhão de linhas por estrutura no MVP (H-04). O módulo é um `Module` do ravenrs-app e também roda como exemplo independente (H-05).
