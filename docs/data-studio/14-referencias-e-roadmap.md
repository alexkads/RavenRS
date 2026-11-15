# Raven Data Studio: referências e roadmap

## Referências estudadas

Fontes oficiais consultadas em 15/11/2026. A tabela liga cada padrão observado à decisão que ele motivou aqui. As decisões são nossas; nenhum produto é copiado.

| Produto | Padrão observado | Decisão no Data Studio |
|---|---|---|
| Altova MapForce | Canvas de origem e destino com conectores; funções soltas no canvas; visão geral para mapas grandes; ações de tabela (inserir, atualizar se, ignorar se) avaliadas em ordem por colunas-chave; escopo de transação configurável. Sugestões da IA aparecem provisórias, aceitas ou recusadas uma a uma, com commit no fim. | Canvas como vista do mesmo plano, com minimapa ([06-telas.md](06-telas.md#8-canvas)). Sugestões da IA como `PlanPatch` provisório, aceito item a item ([09-modelo-de-dominio.md](09-modelo-de-dominio.md#planversion-e-planpatch)). `ConflictPolicy` explícita por entidade. |
| DBeaver | Assistente em passos com mapeamento em **tabela** (estado por tabela: criar, existente, pular, recriar; transformação por coluna); amostra configurável para inferir tipos; prévia antes de rodar; tarefa salva e reutilizável; comparação de schema gerando DDL. | Grade é a vista principal e acessível do mapeamento; `Disposition` por entidade (usar, criar, alterar, intermediária); amostra configurável (H-03); tela Comparação e DDL; projeto reexecutável. |
| Flatfile | Mapeamento por lista de colunas e **mapeamento de valores** em campos enumerados; limite de confiança no mapeamento automático: se uma coluna fica abaixo, tudo vai para revisão humana; restrições declarativas (obrigatório, único, único composto); ação final bloqueada enquanto houver registro inválido. | `MapValues` como regra própria (situações do pedido); confiança por sugestão e pendência quando abaixo do limite; `ValidationRule` declarativa antes de expressão; aprovação bloqueada por pendência bloqueante. |
| Dromo | Envio só de cabeçalhos ao serviço de casamento (privacidade); memória de mapeamentos; validação em fases (coluna, lote, linha, de novo após edição) com três níveis (erro, aviso, informação); importação sem interface que para e devolve um link para revisão quando não casa. | Política do que vai ao modelo (só estrutura por padrão, RF-72); severidade erro/aviso; motor sem interface (RNF-03) que para em `Bloqueado` e aponta a tela de revisão. |
| Airbyte | Protocolo de conectores declarativo: especificação, verificação, descoberta de catálogo, leitura, escrita; modos de sincronização com chave primária obrigatória para deduplicar; estado só vale depois que o destino confirma; entrega pelo menos uma vez; rejeitados guardados à parte e reprocessáveis; política para mudança de schema (propagar, aprovar, pausar). | `ConnectorCapabilities` e o contrato de adaptador `check / discover / profile / read / write / ddl` ([10-mcp-e-conectores.md](10-mcp-e-conectores.md)); checkpoint gravado na mesma transação do lote; idempotência por chave para tolerar reenvio; `RejectedRecord` fiel; fingerprint da origem bloqueia a execução se a estrutura mudou. |

Páginas consultadas:

- MapForce: `altova.com/mapforce`, manual `mf_ai.html`, `mff_db_table_actions.html`, `mff_db_transactions.html`, `mff_db_merge.html`.
- DBeaver: `dbeaver.com/docs/dbeaver/Data-transfer/`, `Data-import/`, `Data-migration/`, `Data-Transfer-Actions/`, `Schema-compare/`, `Structure-and-Data-Compare/`, `7.3/Database-Structure-Diagrams/`.
- Flatfile: `flatfile.com/docs/legacy-docs/concepts/mapping`, `plugins/automap`, `plugins/record-hook`, `core-concepts/fields`, `guides/using-actions`.
- Dromo: `developer.dromo.io/hooks`, `faqs`, `getting-started/headless`, `guides/saved-schema-api`, `changelog`.
- Airbyte: `docs.airbyte.com/platform/understanding-airbyte/airbyte-protocol`, `using-airbyte/core-concepts/sync-modes/incremental-append-deduped`, `move-data/rejected-records`, `using-airbyte/schema-change-management`.

O que as fontes não esclarecem (não usado como premissa): como a confiança do mapeamento automático da Flatfile é calculada; se a importação sem interface da Dromo reaproveita mapeamentos salvos; em quais edições do DBeaver a comparação de schema está disponível.

### Lições adotadas

1. A IA propõe em estado provisório, item a item, e o usuário confirma (MapForce).
2. Confiança abaixo do limite vira pendência, não decisão silenciosa (Flatfile).
3. Mapear valores, não só colunas (Flatfile).
4. Mandar ao modelo o mínimo; estrutura antes de dados (Dromo). Combina com IA local por padrão.
5. Duas vistas do mapeamento: tabela para o caso comum e para acessibilidade, canvas para relações (DBeaver e MapForce).
6. Restrições declarativas primeiro, expressão para o resto (Flatfile).
7. Upsert explícito por chave, com campos atualizáveis (MapForce, DBeaver).
8. Checkpoint só depois do commit do destino; reenvio tolerado por idempotência (Airbyte).
9. Rejeitados não derrubam a carga, ficam fiéis e reprocessáveis (Airbyte).
10. Capacidades declaradas pelo conector decidem o que a interface oferece (Airbyte).
11. Mudança de estrutura da origem bloqueia até nova revisão (Airbyte).

## Roadmap

O desenvolvimento é dividido em três fases. A ordem segue as dependências técnicas (o que precisa existir antes) e o valor para o usuário (o que já resolve uma migração real).

```
 Fase 0 (biblioteca)   ─► MVP ─────────────────────► Evolução ──────────────► Avançado
 CodeLanguage::Sql       CSV + 1 banco                canvas de mapeamento     MCP externo
 TextDiff                grade de mapeamento          relacionamentos          heterogêneos
 issue list persistente  regras essenciais            conflitos por registro   escrita em massa
                         simulação local + DDL        pausar/retomar           reconciliação ampla
                         execução em lotes            modelos reutilizáveis    desfazer execução
                         relatório                    IA remota opcional
```

### Fase 0: o que entra antes na biblioteca

Pela regra da casa, o que falta para as telas entra primeiro no RavenRS, genérico, e só depois é usado pelo Data Studio. Detalhes em [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md).

| Item | Por que antes | Usado em |
|---|---|---|
| Destaque de SQL no `CodeEditor` (`CodeLanguage::Sql`) | O DDL é revisado pelo usuário no MVP; texto simples serve, mas destaque reduz erro de leitura. Pode ficar para o fim do MVP sem bloquear. | Comparação e DDL |
| `TextDiff` (comparação de texto lado a lado e unificada) | Comparação de versões do plano e do DDL. | Comparação, histórico de versões |
| Lista persistente de problemas (`IssueList`, agrupada, filtrável, com navegação ao item) | Problemas não podem ficar só em toast. Verificar antes se `Diagnostics` serve (ver matriz). | Perfil, Transformações, Simulação, Resultado |

### MVP

**Objetivo:** migrar o cenário de referência de ponta a ponta a partir de CSV, ou de um banco relacional, para um banco relacional, com plano salvo, simulação honesta e relatório.

| Entrega | Requisitos | Depende de |
|---|---|---|
| Crate de domínio (`ravenrs-datastudio`, nome provisório): plano, patches, versões, validação, invalidação | RF-62, RF-71, RNF-02 | nada; é o primeiro |
| Adaptadores CSV (leitura) e PostgreSQL (leitura, escrita, DDL) | RF-03, RF-04, RF-05, RF-10 | domínio |
| Perfil e snapshot | RF-11, RF-12, RF-13, RF-15 | adaptadores |
| Schema: usar existente, criar, `legacy_id`, DDL como texto | RF-20 a RF-23 | domínio, adaptador com DDL |
| Grade de mapeamento com sugestões aceitas item a item | RF-30, RF-33, RF-34 | `DataGrid`, domínio |
| Regras essenciais com antes e depois | RF-40 a RF-43 | domínio, `ravenrs-criteria` |
| Conflitos: inserir, upsert, ignorar; mescla de duplicados; referência ausente | RF-50 a RF-53 | domínio |
| Simulação local e geração de comandos | RF-60, RF-61 | tudo acima |
| Aprovação, execução em lotes, cancelar, idempotência, rejeitados fiéis | RF-63, RF-64, RF-65 (cancelar), RF-66, RF-67 | simulação |
| Relatório com contagens | RF-68 | `ravenrs-report` / `ravenrs-grid-export` |
| Conversa com IA local que produz patches | RF-70 a RF-73 (local) | domínio, `ravenrs-ai` |

**Por que nesta ordem.** O domínio vem primeiro porque todas as vistas e o motor dependem dele, e porque pode ser testado sem interface. A grade vem antes do canvas porque resolve o caso comum, já existe pronta (`DataGrid`) e é a alternativa acessível obrigatória. A IA entra no fim do MVP: o produto precisa funcionar sem ela (edição manual e heurísticas), e a IA só acelera.

**Pronto quando:** o cenário de referência chega exatamente às contagens de [cenario-de-referencia.md](cenario-de-referencia.md) (1.202 clientes, 1.568 endereços, 5.261 pedidos, 83 fora), reexecutar não duplica nada, e o relatório mostra a conta fechando.

### Evolução

| Entrega | Por que agora |
|---|---|
| Canvas de mapeamento sobre `FlowCanvas` (nós com campos, ligações campo a campo, grupos, organizar sozinho, desfazer) | Depende do restante da continuação do `FlowCanvas` (organizar sozinho, grupos, desempenho com 1.000 nós; desfazer e portas pelo teclado já vieram no PR #389). Ganha valor quando há várias entidades e relações. |
| Inferência de relacionamentos | O canvas é onde ela aparece melhor; antes disso, o usuário declara o vínculo. |
| Revisão por registro (fila de revisão com decisão individual) | Precisa da lista persistente e dos rejeitados do MVP. |
| Pausar e retomar do checkpoint | O checkpoint já é gravado no MVP; aqui ganha interface e testes de falha. |
| Execução isolada (schema descartável) | Depende de capacidade do adaptador. |
| Modelos reutilizáveis (regras, mapeamentos de valores, planos-modelo) | Só faz sentido depois de alguns projetos reais. |
| Destaque de SQL e comparação de versões, se não entraram na Fase 0 | |
| IA remota por escolha explícita | Depois de medir o modelo local no planejamento real. |

### Avançado

| Entrega | Por que por último |
|---|---|
| Cliente MCP da aplicação e servidores MCP externos como conectores | Exige um cliente MCP que não existe hoje, política de permissões por ferramenta e isolamento do conteúdo externo. Alto risco, valor só com sistemas heterogêneos. |
| Sistemas heterogêneos (APIs, planilhas com várias abas, outros bancos) | Depende do contrato de adaptador estabilizado no MVP e na Evolução. |
| Escrita em massa nativa (COPY, cargas paralelas) e volumes acima de 1 milhão de linhas por estrutura | Exige medir o gargalo real e capacidades específicas por banco. |
| Reconciliação ampliada (somas de controle, amostragem cruzada, relatório para auditoria) | Constrói sobre o relatório do MVP. |
| Desfazer uma execução concluída | Depende do mapa de ids e de capacidades de apagar/restaurar; nunca global. |
| Execução agendada no servidor (junto com o servidor de relatórios) | Depende do motor sem interface estável. |

### Integração futura separada

A geração de formulários e grades do ravenrs-app a partir das entidades criadas pelo Data Studio é outra iniciativa, com plano próprio. Ela depende de o ravenrs-app aceitar metadados descritos em tempo de execução, o que hoje não é suportado (ver [08-arquitetura.md](08-arquitetura.md)).
