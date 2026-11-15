# Raven Data Studio: arquitetura

## Visão em camadas

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  Apresentação        ravenrs-widgets (+ ravenrs-app-ui para telas administrativas)
│  telas do módulo     Wizard · DataGrid · FlowCanvas→MappingCanvas · Chat · CodeEditor
│                      lêem o plano, emitem PlanCommand; nenhuma regra de domínio aqui
├──────────────────────────────────────────────────────────────────────────────┤
│  Aplicação           Module "DataStudio" no ravenrs-app
│                      Actions (Simular, Aprovar, Executar…) · Controllers · Segurança · Auditoria
├──────────────────────────────────────────────────────────────────────────────┤
│  Domínio (puro)      ravenrs-datastudio (crate novo, sem UI, sem I/O)
│                      MappingPlan · PlanPatch · validação · invalidação · DependencyGraph
│                      perfilador sobre amostras · avaliação de regras (ravenrs-criteria)
├───────────────┬──────────────────────────────┬───────────────────────────────┤
│  Agente de    │  Motor determinístico        │  Adaptadores de execução      │
│  planejamento │  ravenrs-datastudio-engine   │  trait Connector              │
│  (IA local    │  lotes · checkpoints · mapa  │  csv · sqlite · postgres      │
│  ravenrs-ai;  │  de ids · rejeitados ·       │  (Avançado: mcp-externo)      │
│  remoto opt.) │  reconciliação · relatório   │                               │
└───────────────┴──────────────────────────────┴───────────────────────────────┘
   persistência do módulo: ravenrs-app (ObjectSpace) sobre ravenrs-app-sqlite / -postgres
   sistemas externos: lidos e escritos SÓ pelos adaptadores
```

### Crates propostos (nomes provisórios)

| Crate | Depende de | Não depende de | Conteúdo |
|---|---|---|---|
| `ravenrs-datastudio` | `ravenrs-criteria`, `serde` | widgets, app, I/O | Modelo de domínio, plano, patches, validação, perfilador sobre dados em memória, invalidação. |
| `ravenrs-datastudio-connectors` | `ravenrs-datastudio`, drivers | widgets, app | `trait Connector` e os adaptadores `csv`, `sqlite`, `postgres`. CSV reaproveita o leitor de `ravenrs-sheet` depois de extraído (ver abaixo). |
| `ravenrs-datastudio-engine` | os dois acima | widgets | Motor de execução, sem interface; usado pela tela e pelo servidor. |
| `ravenrs-datastudio-agent` | `ravenrs-datastudio`, `ravenrs-ai` | widgets, conectores | Agente de planejamento: monta o contexto, chama o modelo, lê a resposta como `PlanPatch`. |
| `ravenrs-datastudio-app` | todos acima, `ravenrs-app`, `ravenrs-app-ui`, `ravenrs-widgets` | | `Module`, entidades administrativas, telas, actions, controllers. |

A separação segue as camadas do repositório (skill `ravenrs-acoplamento`): o domínio não conhece widgets nem conectores; widgets genéricos novos (ver [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md)) vão para as famílias de `ravenrs-widgets-*`, nunca para os crates do Data Studio.

### Leitor de CSV compartilhado

`ravenrs-sheet` já tem detecção de codificação (`decode_text`), de separador (`sniff_delimiter`) e um leitor de registros por callback (`craft::csv::parse_records`, `crates/ravenrs-sheet/src/craft/csv/mod.rs:83`). Pela regra de reaproveitar entre motores, a proposta é **extrair** esse leitor para um crate compartilhado (ex.: `ravenrs-csv`) e fazê-lo ler de um `Read` em blocos, em vez de exigir o texto inteiro em memória. A planilha e o Data Studio passam a usar o mesmo crate. Copiar o código está fora de questão.

## Fluxo de dados

```
 Pedido ─► Agente ─► PlanPatch ─► Domínio.validar ─► PlanVersion n+1 ─► vistas atualizam
                          ▲                │
                          │          recusa com motivo ─► conversa mostra
   Usuário (grade, canvas, assistente) ─► PlanCommand ─► PlanPatch

 PlanVersion ─► Simulador (domínio + leitura dos adaptadores) ─► SimulationResult
 Aprovação ─► Motor ─► Adaptadores ─► destino
                 └─► eventos de progresso ─► tela de Execução
```

- **As vistas não chamam adaptadores.** A grade de prévia pede páginas ao serviço do módulo (via `RemoteRows`), que pede ao adaptador.
- **O agente não chama adaptadores.** Ele recebe do domínio um contexto montado (estruturas, perfis, amostra mascarada se permitida) e devolve um patch. Isso mantém a regra "conteúdo externo é dado" num lugar só.

## Integração com o ravenrs-app

### Quatro tipos de "entidade" que não podem se confundir

| Tipo | Exemplo | Onde vive | Como é descrito |
|---|---|---|---|
| **Entidades administrativas do módulo** | `ImportProject`, `Connection`, `ExecutionJob`, `RejectedRecord` | Banco do aplicativo | Structs Rust com `#[derive(BusinessObject)]`, registradas no `Module`; listas e formulários via `ravenrs-app-ui`. |
| **Metadados descobertos** | estrutura de `CLIENTES`, `PEDIDOS` | `SourceSnapshot`, dentro do plano | Dados do domínio (`structures`, `profile`). Não são `EntityInfo`. |
| **Entidades propostas para o destino** | `customers`, `customer_addresses`, `orders` | `SchemaProposal` no plano | `EntityProposal`. Podem ser **projetadas** como `EntityInfo` só para exibição e validação de expressões. |
| **Objetos registrados no aplicativo** | um `Customer` do app de destino | O app de destino, que é outro programa | O app de destino declara suas entidades em Rust. O Data Studio não cria tipos Rust. |

### EntityInfo

`EntityInfo` (`crates/ravenrs-app/src/entity.rs:611`) é um struct montável em tempo de execução pelo builder `EntityInfo::new(name).field(FieldInfo::new(…))`. Isso permite **projetar** uma `EntityProposal` como `EntityInfo` para:

- validar expressões de transformação e validação contra os campos (`EntityInfo::schema()` → `ravenrs_criteria::validate`);
- reaproveitar `FieldKind` como base do `LogicalType` (com a ressalva de que `Decimal` hoje é `f64` com 15 dígitos significativos, insuficiente para `NUMERIC` de precisão arbitrária: o `LogicalType` do Data Studio guarda precisão e escala próprias);
- mostrar a entidade proposta num formulário de prévia gerado.

O que **não** se presume:

- Que uma entidade derivada em compilação (`#[derive(BusinessObject)]`) possa ser criada dinamicamente: não pode. Uma `EntityInfo` montada em execução não tem struct Rust, só `Object` genérico.
- Que o app de destino passe a ter telas para as tabelas criadas: isso é a integração futura separada.

`ravenrs_app_sqlite::entities_from_schema` (`crates/ravenrs-app-sqlite/src/lib.rs:107`) já lê um banco SQLite e devolve `EntityInfo` + `TableMap`. É uma base útil para a descoberta do adaptador SQLite, mas a heurística de tipos é simples (todo `NUMERIC` vira `Decimal(2)`, tabelas sem chave única ficam de fora), então o adaptador usa a introspecção própria e só aproveita o código onde servir. Para PostgreSQL não há equivalente: o adaptador faz a introspecção por `information_schema` e `pg_catalog`.

### ModelLayers

`ModelLayers` (`crates/ravenrs-app/src/model/layers.rs`) guarda diferenças de **modelo de interface** em camadas (`Module`, `Administrator`, `User`) sobre o modelo gerado. Serve para o que ele faz bem:

- personalizar as telas administrativas do módulo (colunas da lista de projetos, rótulos);
- guardar preferências de vista do usuário (largura de colunas da grade de mapeamento, painéis abertos).

Não serve para versionar o plano: camadas resolvem "quem vence" por nível, não histórico linear com autor, motivo, aprovação e invalidação. O plano tem versionamento próprio (`PlanVersion`).

### ObjectSpace

Usado para **as entidades administrativas do módulo** (projetos, jobs, rejeitados), com `commit()` tudo-ou-nada. **Não** é usado como motor de carga no destino:

- o `SqlObjectSpace` faz uma consulta de existência por `save`, um `MAX + 1` por `new_object` e um `INSERT` por registro, numa única transação longa (`crates/ravenrs-app-sql/src/lib.rs:508-565`);
- não há inserção em lote nem commit parcial;
- `commit()` não roda regras de validação (quem valida são formulários e actions).

O motor fala com o destino pelos adaptadores, com lotes e transação por lote.

### Actions, controllers, Module e Application

| Peça do app | Uso no Data Studio |
|---|---|
| `Module` (`module_api.rs:10`) | `DataStudioModule`: entidades administrativas, controllers, textos, registros iniciais (modelos de regras). |
| `Application::builder` | O módulo entra em qualquer app RavenRS; também há um exemplo independente que só monta o módulo. |
| `Action` (`action.rs:460`) | Simular, Validar, Aprovar, Executar, Pausar, Retomar, Cancelar, Exportar rejeitados. Disponibilidade por `Reasons`: ex. `Executar` só com `REASON_CRITERIA` = projeto `Aprovado` e `REASON_SECURITY` = permissão. `confirmation` para aprovar e executar. |
| `ViewController` | Controlador da vista do plano: liga a seleção da grade e do canvas, mantém as actions atualizadas. |
| `WindowController` | Atalhos globais (paleta de comandos), estado da execução em segundo plano na barra de status. |
| Segurança (`security/`) | Permissões por operação nas entidades do módulo e por action (`action_permission`): quem pode criar conexão, aprovar, executar. Ver [13-seguranca-privacidade-a11y-i18n.md](13-seguranca-privacidade-a11y-i18n.md). |
| Auditoria (`AuditTrail`, `AuditedObjectSpace`) | Eventos de alteração nas entidades do módulo; versões do plano e aprovações gravadas como registros auditados. |
| `ravenrs-app-ui` | Telas administrativas (lista de projetos, conexões, execuções, rejeitados) geradas pelo padrão: listas `DataGrid`, formulários `FormLayout`, comandos `Toolbar`. As telas de trabalho (mapeamento, canvas, simulação) são vistas próprias do módulo, compostas com os mesmos widgets. |

### Persistência do módulo × sistemas externos

| | Persistência do módulo | Sistemas externos |
|---|---|---|
| O quê | Projetos, conexões (sem segredo), versões do plano, simulações, jobs, rejeitados, relatórios | Origem e destino da importação |
| Como | `ObjectSpace` sobre `ravenrs-app-sqlite` (desktop) ou `ravenrs-app-postgres` (servidor) | `trait Connector` dos adaptadores |
| Limites relevantes | Geração de chave `MAX + 1` por processo: só um processo deve criar registros por tabela. Entidades do módulo usam `FieldKind::Guid` como chave para evitar o problema. | Ids do destino gerados pelo próprio banco de destino (`IDENTITY`), nunca pelo ravenrs-app. |

Os crates `ravenrs-app-sql`, `-sqlite` e `-postgres` **não** são apresentados como conectores universais nem como motores de carga em massa. O adaptador PostgreSQL do Data Studio pode reaproveitar a conexão (`PostgresBackend::connect_with`, TLS) se ela for exposta publicamente; a escrita em lote e o DDL completo (índices, FKs, `UNIQUE`, `NOT NULL`) são do adaptador, porque `SqlBuilder::managed` cria só tipos e chave primária.

## Expressões

`ravenrs-criteria` é a linguagem das regras `Expression` e das validações `Check::Expression`.

- Avaliação em memória: `Scope::new(&env).row(&r).evaluate(&c)` sobre `Record`.
- Validação de nomes: `validate(&Criteria, &Schema)` aponta campo, coleção, parâmetro ou função desconhecidos. **Não confere tipos.**
- 129 funções escalares e 26 agregados (`functions::catalog()`), com assinaturas traduzidas para o `ExpressionEditor` via `Labels::expression_signature`.
- Tradução para SQL (`to_sql`) é parcial; o motor **avalia no cliente**, por lote, e não depende de empurrar a transformação para o banco.

O que a linguagem não cobre e fica como `TransformOp` próprio: divisão em vários campos, tabela de valores, leitura de decimal com separadores, validação de CPF/CNPJ, lookup em outra entidade, mescla de duplicados. Detalhes em [11-schema-mapeamento-transformacoes.md](11-schema-mapeamento-transformacoes.md). Uma alternativa futura é registrar funções próprias com `Environment::register` (ex.: `KeepDigits`, `IsCpf`), o que as torna disponíveis no `ExpressionEditor` também.

## IA

Ver [10-mcp-e-conectores.md](10-mcp-e-conectores.md) para a separação entre MCP de desenvolvimento, cliente MCP futuro e agente.

`ravenrs-ai` oferece modelos locais em CPU (`LocalEngine::generate`, `LocalAssistant`), um turno por pergunta, e chamada de ferramenta simplificada (JSON `{"tool","args"}` com argumentos em texto). Para o agente de planejamento isso significa:

- **Saída estruturada por validação, não por confiança.** O agente pede JSON de `PlanPatch`; o domínio faz o parse estrito, recusa o que não valida e devolve o erro ao modelo para uma nova tentativa (no máximo 2).
- **Contexto montado a cada turno.** Sem histórico no motor, o agente monta o contexto (resumo do plano, item em foco, pergunta) a cada pedido. O histórico da conversa é guardado pelo módulo, não pelo modelo.
- **Planejamento em partes.** Um modelo de 1 a 8 bilhões de parâmetros não planeja tudo de uma vez com qualidade. O agente divide: propor schema → mapear campos por entidade → propor regras por campo com problema → propor conflitos. Cada parte é um patch.
- **Heurísticas primeiro.** Casamento por nome e tipo, detecção de padrões (CPF, CEP, data) e mapeamento de valores por semelhança rodam no domínio, sem modelo; a IA refina e explica. O produto funciona sem IA.
- **Provedor remoto** é evolução, por escolha explícita, com a mesma política do que pode sair da máquina.

## Plataformas

- Desktop (macOS, Windows, Linux): MVP completo.
- Web (WebAssembly): as telas funcionam, mas o navegador não abre conexão direta com bancos. Na web, conectores e motor rodam num servidor (o mesmo motor sem interface), e a interface fala com ele. Decisão D-07.
