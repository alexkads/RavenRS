# Cobertura do ravenrs-app para aplicações corporativas

Data: 15/11/2026. Base de código: `3cfedf720`; documentação de trabalho: `1700fe807`. Avaliação estática de APIs, guias, exemplos, testes existentes e issues consultadas. A execução de verificações deste documento não transforma os cenários propostos em capacidades entregues.

## Objetivo

Permitir que consumidores, inclusive assistidos por modelos de IA com menos recursos, construam aplicações corporativas **declarando o domínio, configurando capacidades e escrevendo principalmente as particularidades do negócio**.

A cobertura de [tipos de software](SOFTWARE-COVERAGE.md) pergunta que experiências visuais e motores estão disponíveis. Esta cobertura pergunta **quanto do fluxo corporativo o framework já absorve e quanto ainda obriga o consumidor a implementar ou integrar**.

A família inclui o núcleo sem GUI, UI gerada, adaptadores de dados, servidor/cliente remoto, relatórios, biblioteca de registros e módulos opcionais. Não concentrar tudo no crate ravenrs-app:

| Camada | Responsabilidade |
|---|---|
| ravenrs-app | Modelo, contratos, unidade de trabalho, ações, regras, segurança e módulos |
| ravenrs-app-ui | Experiência gerada e extensão de telas/editores |
| Adaptadores e serviço | Persistência, transporte e capacidades específicas do host |
| Pacotes opcionais | Processos, relatórios e funcionalidades compartilháveis |
| ravenrs-mcp e CLI | Descoberta, exemplos, validação disponível e preparação do projeto |
| Consumidor | Domínio, elegibilidade, alçadas, retenção, políticas e integrações próprias |

## Método e limites

São **24 áreas e 240 capacidades**, dez por área, com pesos iguais:

- **1:** suporte público/documentado para o item delimitado, disponível no código analisado.
- **0,5:** suporte parcial, caminho por extensão ou composição ainda exige integração relevante.
- **0:** entrega apenas planejada ou suporte não comprovado no recorte consultado. Não prova ausência exaustiva nem impossibilidade de construir.

Índice da área = soma dos pontos / 10 × 100. Trata-se de uma estimativa de cobertura do recorte, não percentual de produto pronto, esforço restante ou redução de tokens. Não há intervalo estatístico nem benchmark de modelos menores nesta análise.

**Não calcular uma média como “o ravenrs-app está X% pronto”.** Os recortes são diferentes e nem todo projeto precisa de SSO, multiempresa, agenda ou offline. “Núcleo” significa capacidade transversal, não dependência obrigatória; os pacotes continuam opcionais quando possível.

O estado de uma issue não substitui a leitura da entrega: uma issue fechada pode ter escopo menor que a área. Uma issue aberta pode conter itens já implementados. [APP-PARITY.md](APP-PARITY.md) descreve APIs/status, [APP-CONTRACTS.md](APP-CONTRACTS.md) define contratos e C01–C19/M01–M19; este documento organiza cobertura, lacunas e prioridade de consumo.

A inspeção consultou testes como evidência do contrato pretendido, sem afirmar que todos foram executados. Hosts/providers externos precisam de verificação própria. Uma capacidade descrita em memória ou SQLite não se estende automaticamente a web, PostgreSQL ou outro processo.

## Visão geral

| Área | Grupo | Índice na base auditada | O que ainda pesa no consumidor |
|---|---|---:|---|
| A01 Modelo de domínio declarativo | Núcleo | 85% | Campos adicionados dinamicamente ainda são simples; estruturas compostas e validação de todas as diferenças precisam de contratos mais completos. |
| A02 Telas geradas, navegação e acessibilidade | Núcleo | 75% | ListView remoto não significa que RemoteApp faça paginação integral do servidor. Auditar idiomas, acessibilidade por host, listas aninhadas e layout RTL. |
| A03 Ações e comandos de negócio | Núcleo | 80% | Completar posicionamento pelo modelo e contratos de comandos reversíveis; cancelamento de uma tarefa não desfaz efeitos externos já realizados. |
| A04 Validação e regras declarativas | Núcleo | 80% | Propagar estado inválido de edição e contextos nas coleções; validar e diagnosticar alterações de regras em execução. A aplicação continua declarando suas regras. |
| A05 Transações, agregados e concorrência | Núcleo | 60% | Prioridade: versão atômica na escrita, chaves concorrentes, unidade de trabalho do agregado e idempotência de commits cujo resultado se perdeu. |
| A06 Persistência, tipos e portabilidade | Núcleo | 70% | Não confundir NUMERIC no PostgreSQL com Decimal exato no Value do app: os números ainda passam por f64. Documentar limites temporais, numéricos e capacidades. |
| A07 Autorização em todas as superfícies | Núcleo | 80% | Verificar limites de criação em cada adaptador, mensagens de regras, exportações e anexos. #232 está fechada; sua entrega não prova todas as combinações de providers. |
| A08 Identidade, sessões e SSO | Núcleo | 60% | Providers OIDC/LDAP conforme o projeto, MFA, gestão visual de vínculos e contrato de sessões entre reinícios/instâncias. |
| A09 Multiempresa, unidades e contexto | Núcleo | 35% | TenantContext e propagação aos serviços, arquivos, cache, jobs, auditoria, provisionamento e troca de organização; combinar licença com permissões. |
| A10 Auditoria, histórico e privacidade | Núcleo | 50% | Separar trilha de alterações de histórico temporal, retenção e políticas de privacidade. Permissão de campo não substitui toda a gestão de dados pessoais. |
| A11 Processos, aprovações e tarefas | Núcleo | 75% | Concorrência atômica #241, escalonamento e designer versionado. Fluxos paralelos, compensações e regras especiais dependem do recorte; não se promete BPMN completo. |
| A12 Jobs, agendamento e notificações | Núcleo | 45% | Scheduler e execução durável, cancelamento cooperativo por provider, notificações com deduplicação e preferências. #66 cobre agendamento no app aberto, não toda a plataforma de jobs. |
| A13 API, eventos e integrações | Núcleo | 75% | Conectores específicos opcionais, worker independente, propagação de timeout/cancelamento e operação monitorável. Não prometer exactly-once distribuído por existir deduplicação local. |
| A14 Importação, exportação e busca | Núcleo | 35% | Wizard CSV/XLSX, referências e deduplicação, busca transversal e exportação remota. Toda entrada passa por ObjectSpace, regras e auditoria. |
| A15 Relatórios e saídas corporativas | Núcleo | 60% | Preservar filtro/escopo em todas as saídas, limitar memória, agendar com identidade e fornecer templates por entidade onde necessário. |
| A16 Painéis e análise de negócio | Dependente do projeto | 60% | Designer, crossfilter, pivot, layout persistente e agregação no provider; reaproveitar Chart e os controles existentes. |
| A17 Anexos e documentos emitidos | Núcleo | 40% | Download autorizado, ciclo de vida dos arquivos, sessão de upload e revisão imutável na emissão. Assinatura manuscrita não é assinatura criptográfica. |
| A18 Colaboração e comunicação no registro | Dependente do projeto | 30% | Histórico e participação, comentários/menções, não lidas, notificações e atualização de telas. #259 cobre conversas, sem cobrir toda colaboração de registro. |
| A19 Agenda, reservas e recursos | Dependente do projeto | 40% | Recorrência, exceções, fusos e reserva atômica; deixar elegibilidade, calendário e capacidade como políticas do consumidor. |
| A20 Customização e módulos reutilizáveis | Núcleo | 75% | Editor visual completo, propagação do guard, pontos de extensão e distribuição de pacotes. Não criar um sistema paralelo ao ModuleManifest. |
| A21 Cliente remoto, tempo real e offline | Dependente do projeto | 35% | Evitar perda de draft na rejeição remota; suportar paginação, referências em lote, eventos de atualização e sincronização com revalidação. |
| A22 Operação, recuperação e implantação | Núcleo | 55% | Documentar recuperação por adaptador, operação dos jobs, traces e ambientes. doctor verifica ferramentas, não substitui telemetria do app em produção. |
| A23 Consumo por IA, MCP e geração | Produtividade com IA | 60% | Receitas completas, diagnóstico estruturado, validação de modelo e bateria com modelos menores. O MCP operacional #266 é uma capacidade distinta e opcional. |
| A24 Testabilidade e contratos de consumo | Núcleo | 80% | Ampliar fluxos de referência e matriz de hosts/providers; um teste ignorado por faltar serviço não prova integração real. |

## Achados que mudam a leitura do backlog

- **O MCP documental já existe.** API, exemplos, guias, contratos, paridade e ferramentas de relatório estão disponíveis. #266 trata um MCP diferente: operar dados e ações de uma aplicação autenticada.
- **Aprovação durável não é uma lacuna inteira.** ravenrs-app-requests já tem etapas, tarefas, delegação, decisões idempotentes e retomada. Faltam especializações, concorrência atômica e escalonamento.
- **Backup SQLite já existe.** backup.rs implementa snapshot e restauração com stamp; #229 está fechada. Não confundir recuperação de SQLite com a operação de todos os providers.
- **CLI já existe.** new, ai e doctor estão documentados; a amplitude de #245 continua além do que esses comandos entregam.
- **Outbox e webhook já existem.** #254/#256 e a entrega local de #255 fornecem capacidades reais; não equivalem a um scheduler ou worker distribuído.
- **Concorrência permanece um bloqueio transversal.** A bateria dos adaptadores declara lost_updates_detected = false; merge de formulário e erro Conflict não garantem comparação de versão atômica.
- **Alguns trackers têm linhas antigas/repetidas.** Há, por exemplo, descrições de relatórios e CLI como futuros ao lado de recursos existentes. Não converter checklist antigo em nota sem conferir código e documentação.

## O que declarar, configurar, estender e ainda implementar

| Fluxo | O consumidor declara/configura hoje | Trabalho ainda relevante |
|---|---|---|
| Cadastro corporativo | Entidade, campos, relações, regras, permissões e layouts | Comportamentos particulares e limites de agregados/edição remota |
| Ação de negócio | Escopo, seleção, critérios e controlador | Algoritmo do negócio e efeitos externos específicos |
| Aprovação | Pacote requests, etapas, papéis, expressões e política | Desenho visual, escalonamento e limites de concorrência |
| Relatório | Entidades, parâmetros, permissões e template | Volume, distribuição agendada e documento emitido versionado |
| Integração | Eventos, provider, webhook e política do worker | Conector concreto, operação do worker e coordenação entre processos |
| App multiempresa/offline | Políticas e peças de dados disponíveis | Contexto de tenant e sincronização ainda não consumíveis como fluxo completo |
| Assistência ao desenvolvimento | CLI, MCP documental, guias e checks | Bateria de tarefas/modelos e validação estruturada de diferenças completas |

## A01 Modelo de domínio declarativo: 85%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Entidades por derive BusinessObject | 1 |
| Entidades em tempo de execução | 1 |
| Campos tipados e metadados de apresentação | 1 |
| Referências e coleções | 1 |
| Campos calculados e funções da aplicação | 1 |
| Singletons e registros não persistidos | 1 |
| Chaves numéricas, texto e GUID | 1 |
| Campos adicionais por diferenças do modelo | 0,5 |
| Expansão automática de structs em grupos compostos | 0 |
| Validação do modelo antes de abrir telas | 1 |

**Já absorvido pelo framework:** Entidades, campos, relações e metadados geram a estrutura comum; a regra de negócio continua no consumidor.

**Lacunas e integração restante:** Campos adicionados dinamicamente ainda são simples; estruturas compostas e validação de todas as diferenças precisam de contratos mais completos.

**Backlog relacionado:** [#70](https://github.com/alexkads/RavenRS/issues/70), [#79](https://github.com/alexkads/RavenRS/issues/79), [#81](https://github.com/alexkads/RavenRS/issues/81), [#437](https://github.com/alexkads/RavenRS/issues/437). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/entity.rs](../crates/ravenrs-app/src/entity.rs), [crates/ravenrs-app/src/model/generate.rs](../crates/ravenrs-app/src/model/generate.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Declarar duas entidades relacionadas, gerar lista/detalhe e acrescentar um campo calculado sem duplicar a definição entre UI e serviço.

## A02 Telas geradas, navegação e acessibilidade: 75%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Listas e formulários gerados | 1 |
| Navegação por metadados | 1 |
| Busca, filtro, ordem e resumo de listas locais | 1 |
| Layouts de detalhe, grupos e abas | 1 |
| Variantes de lista e preferências | 1 |
| Editores de campo substituíveis | 1 |
| Listas grandes pelo caminho remoto de ListView | 0,5 |
| Teclado e semântica acessível das telas geradas | 0,5 |
| Idioma/cultura coerentes em todos os editores | 0,5 |
| Espelhamento de layout RTL | 0 |

**Já absorvido pelo framework:** O fluxo cadastro → lista → detalhe usa App, ListView, DetailView, EditorRegistry e SystemModule.

**Lacunas e integração restante:** ListView remoto não significa que RemoteApp faça paginação integral do servidor. Auditar idiomas, acessibilidade por host, listas aninhadas e layout RTL.

**Backlog relacionado:** [#72](https://github.com/alexkads/RavenRS/issues/72), [#73](https://github.com/alexkads/RavenRS/issues/73), [#184](https://github.com/alexkads/RavenRS/issues/184), [#231](https://github.com/alexkads/RavenRS/issues/231), [#401](https://github.com/alexkads/RavenRS/issues/401), [#403](https://github.com/alexkads/RavenRS/issues/403), [#437](https://github.com/alexkads/RavenRS/issues/437). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-ui/src/list_view.rs](../crates/ravenrs-app-ui/src/list_view.rs), [crates/ravenrs-app-ui/src/detail_view.rs](../crates/ravenrs-app-ui/src/detail_view.rs), [crates/ravenrs-app-ui/src/editors.rs](../crates/ravenrs-app-ui/src/editors.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Um consumidor declara campos e layouts e usa as mesmas telas com teclado, idioma e permissões, sem montar CRUD à mão.

## A03 Ações e comandos de negócio: 80%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Novo, salvar, cancelar, excluir e atualizar | 1 |
| Ações simples, popup, escolha e parâmetro | 1 |
| Escopo e dependência da seleção | 1 |
| Critérios e razões de ativação | 1 |
| Permissão da ação | 1 |
| Ações dentro do formulário | 1 |
| Mensagens e resultado da ação | 1 |
| Ações longas com progresso/cancelamento | 1 |
| Mapeamento de containers pelo modelo | 0 |
| Undo/redo genérico e semântica de efeitos de comandos | 0 |

**Já absorvido pelo framework:** Ações comuns e extensões ficam nos controladores, com estado de ativação e feedback já fornecidos.

**Lacunas e integração restante:** Completar posicionamento pelo modelo e contratos de comandos reversíveis; cancelamento de uma tarefa não desfaz efeitos externos já realizados.

**Backlog relacionado:** [#79](https://github.com/alexkads/RavenRS/issues/79), [#81](https://github.com/alexkads/RavenRS/issues/81), [#82](https://github.com/alexkads/RavenRS/issues/82). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/action.rs](../crates/ravenrs-app/src/action.rs), [crates/ravenrs-app/src/controller.rs](../crates/ravenrs-app/src/controller.rs), [crates/ravenrs-app-ui/src/system.rs](../crates/ravenrs-app-ui/src/system.rs).

**Fluxo para comprovar o consumo:** Acrescentar uma ação específica sem reimplementar botões, seleção, permissão e feedback; declarar quando a ação não pode ser desfeita.

## A04 Validação e regras declarativas: 80%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Obrigatoriedade, faixas e padrões | 1 |
| Comparações e critérios de expressão | 1 |
| Unicidade e regras envolvendo outros registros | 1 |
| Regras de salvar/excluir e contextos de ações | 1 |
| Warnings/informações com continuação explícita | 1 |
| Mensagens parametrizadas por campo | 1 |
| Validação também sem UI | 1 |
| Contextos nas coleções aninhadas | 0 |
| Validação de draft tipado integrada em todos os editores | 0,5 |
| Regras no modelo em execução | 0,5 |

**Já absorvido pelo framework:** Rule, validate e critérios evitam repetir validação em cada formulário e no serviço.

**Lacunas e integração restante:** Propagar estado inválido de edição e contextos nas coleções; validar e diagnosticar alterações de regras em execução. A aplicação continua declarando suas regras.

**Backlog relacionado:** [#79](https://github.com/alexkads/RavenRS/issues/79), [#444](https://github.com/alexkads/RavenRS/issues/444), [#435](https://github.com/alexkads/RavenRS/issues/435). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/validation.rs](../crates/ravenrs-app/src/validation.rs), [crates/ravenrs-app/src/entity/rules.rs](../crates/ravenrs-app/src/entity/rules.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Uma regra rejeita igualmente a escrita pela tela e pelo serviço; um draft inválido não confirma silenciosamente um valor anterior.

## A05 Transações, agregados e concorrência: 60%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Commit e rollback de unidade de trabalho | 1 |
| Validação e estados no caminho de gravação | 1 |
| Merge de alterações em campos diferentes | 1 |
| Erro tipado de conflito | 1 |
| Auditoria/evento na fronteira documentada de commit | 1 |
| Contrato de adaptadores declara limites de concorrência | 1 |
| Versão otimista atômica por registro | 0 |
| Relações filhas confirmadas/canceladas junto com o formulário pai | 0 |
| Commit remoto reenviável sem duplicar | 0 |
| Reserva de chaves/numeração robusta entre processos | 0 |

**Já absorvido pelo framework:** ObjectSpace, merge_changes e a bateria de contrato já existem; eles não garantem isolamento que o adaptador não declara.

**Lacunas e integração restante:** Prioridade: versão atômica na escrita, chaves concorrentes, unidade de trabalho do agregado e idempotência de commits cujo resultado se perdeu.

**Backlog relacionado:** [#241](https://github.com/alexkads/RavenRS/issues/241), [#332](https://github.com/alexkads/RavenRS/issues/332), [#79](https://github.com/alexkads/RavenRS/issues/79). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/space.rs](../crates/ravenrs-app/src/space.rs), [crates/ravenrs-app-testkit/src/contract.rs](../crates/ravenrs-app-testkit/src/contract.rs), [docs/APP-PARITY.md](../docs/APP-PARITY.md), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Dois escritores da mesma revisão não perdem uma alteração silenciosamente; pai e filhos cancelam juntos; reenvio de um commit produz um efeito.

## A06 Persistência, tipos e portabilidade: 70%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Memória e persistência simples via Storage | 1 |
| SQLite gerenciado ou sobre tabelas existentes | 1 |
| PostgreSQL pelo contrato SQL | 1 |
| PostgREST pelo adaptador próprio | 1 |
| Migrações e erros de compatibilidade | 1 |
| Inspeção de schema/modelo/anexos nos diferentes bancos | 0,5 |
| Decimal exato de ponta a ponta no modelo e nos critérios | 0,5 |
| Instante e fuso preservados como contrato uniforme | 0,5 |
| Mesmas capacidades em desktop/web nos adaptadores | 0,5 |
| Migração de tenant entre bancos com conferência | 0,5 |

**Já absorvido pelo framework:** Vários caminhos de armazenamento e testes de contrato já reduzem o código de integração; suportes variam por adaptador e host.

**Lacunas e integração restante:** Não confundir NUMERIC no PostgreSQL com Decimal exato no Value do app: os números ainda passam por f64. Documentar limites temporais, numéricos e capacidades.

**Backlog relacionado:** [#243](https://github.com/alexkads/RavenRS/issues/243), [#323](https://github.com/alexkads/RavenRS/issues/323), [#331](https://github.com/alexkads/RavenRS/issues/331), [#337](https://github.com/alexkads/RavenRS/issues/337), [#453](https://github.com/alexkads/RavenRS/issues/453). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-sqlite/src/lib.rs](../crates/ravenrs-app-sqlite/src/lib.rs), [crates/ravenrs-app-postgres/src/lib.rs](../crates/ravenrs-app-postgres/src/lib.rs), [crates/ravenrs-app-postgrest/src/lib.rs](../crates/ravenrs-app-postgrest/src/lib.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Executar a mesma regra nos adaptadores declarados e preservar os valores dentro dos limites publicados, recusando perda de precisão.

## A07 Autorização em todas as superfícies: 80%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Papéis e políticas de acesso | 1 |
| Permissão por entidade e operação | 1 |
| Permissão por registro | 1 |
| Permissão por campo | 1 |
| Permissão de navegação e ação | 1 |
| Aplicação no SecuredObjectSpace e na API | 1 |
| Mudança de política em execução | 1 |
| Autorização uniforme em criação com valores resolvidos no armazenamento | 0,5 |
| Ocultação de informação em mensagens/contagens/exports | 0,5 |
| Download de anexo com autorização do domínio | 0 |

**Já absorvido pelo framework:** A política é reutilizável e aplicada na camada de dados; ocultar um botão não é a fronteira de autorização.

**Lacunas e integração restante:** Verificar limites de criação em cada adaptador, mensagens de regras, exportações e anexos. #232 está fechada; sua entrega não prova todas as combinações de providers.

**Backlog relacionado:** [#232](https://github.com/alexkads/RavenRS/issues/232), [#261](https://github.com/alexkads/RavenRS/issues/261), [#226](https://github.com/alexkads/RavenRS/issues/226), [#253](https://github.com/alexkads/RavenRS/issues/253). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/security/space.rs](../crates/ravenrs-app/src/security/space.rs), [crates/ravenrs-app-server/src/lib.rs](../crates/ravenrs-app-server/src/lib.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** A mesma tentativa proibida é recusada na UI, API, exportação e arquivo, sem revelar campos ou registros protegidos.

## A08 Identidade, sessões e SSO: 60%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Login local e verificação de senha | 1 |
| Troca de senha e bloqueio de tentativas | 1 |
| Administração de usuários e papéis | 1 |
| Authenticator substituível | 1 |
| Identidade externa por issuer/subject | 1 |
| Revogação de sessão no contrato externo | 1 |
| Provedor OIDC pronto | 0 |
| MFA pronto | 0 |
| Vínculo/desvínculo externo pela UI | 0 |
| Sessões API persistentes e ciclo distribuído | 0 |

**Já absorvido pelo framework:** Login local e integração abstrata com identidade externa estão presentes. Aplicações ainda precisam fornecer o provider real.

**Lacunas e integração restante:** Providers OIDC/LDAP conforme o projeto, MFA, gestão visual de vínculos e contrato de sessões entre reinícios/instâncias.

**Backlog relacionado:** [#257](https://github.com/alexkads/RavenRS/issues/257), [#79](https://github.com/alexkads/RavenRS/issues/79). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/security/external.rs](../crates/ravenrs-app/src/security/external.rs), [crates/ravenrs-app/src/security/directory.rs](../crates/ravenrs-app/src/security/directory.rs), [crates/ravenrs-app-server/src/lib.rs](../crates/ravenrs-app-server/src/lib.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Um consumidor configura um provider aceito; revogar acesso impede a próxima operação; claims não ampliam as permissões locais.

## A09 Multiempresa, unidades e contexto: 35%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Filtragem por unidade com política de registros | 1 |
| Fixture com organizações e chaves iguais | 1 |
| Contexto imutável de tenant no runtime inteiro | 0 |
| Resolução autenticada de tenant no serviço | 0 |
| Banco por tenant com fábrica/provisionamento | 0 |
| Troca de tenant limpa sessão, caches e drafts | 0 |
| Configurações isoladas e defaults por tenant | 0,5 |
| Jobs/eventos/anexos sempre no tenant correto | 0,5 |
| Migrações e recuperação por tenant | 0,5 |
| Módulos/limites habilitados por licença do tenant | 0 |

**Já absorvido pelo framework:** Políticas, fixtures e eventos com organização dão parte da base. Um campo organization ou bancos separados no teste não implementam o contexto completo.

**Lacunas e integração restante:** TenantContext e propagação aos serviços, arquivos, cache, jobs, auditoria, provisionamento e troca de organização; combinar licença com permissões.

**Backlog relacionado:** [#225](https://github.com/alexkads/RavenRS/issues/225), [#242](https://github.com/alexkads/RavenRS/issues/242), [#243](https://github.com/alexkads/RavenRS/issues/243), [#253](https://github.com/alexkads/RavenRS/issues/253). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/settings.rs](../crates/ravenrs-app/src/settings.rs), [crates/ravenrs-app/src/outbox.rs](../crates/ravenrs-app/src/outbox.rs), [crates/ravenrs-app-testkit/src/lib.rs](../crates/ravenrs-app-testkit/src/lib.rs).

**Fluxo para comprovar o consumo:** T1 e T2 com as mesmas chaves nunca se misturam; trocar organização elimina contexto anterior e não reaproveita permissões.

## A10 Auditoria, histórico e privacidade: 50%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Autor e valores antes/depois | 1 |
| Configuração de auditoria por entidade/campo | 1 |
| Auditoria de coleções e eventos próprios | 1 |
| Auditoria no caminho transacional documentado | 1 |
| Consulta/exportação auditadas por política | 0 |
| Correlação entre UI, API, job e integração | 0,5 |
| Proteção contra alteração do histórico por contrato | 0 |
| Classificação de dados e mascaramento uniforme | 0,5 |
| Retenção, anonimização e exclusão por política | 0 |
| Visão temporal reconstruível do negócio | 0 |

**Já absorvido pelo framework:** AuditTrail, AuditSettings e AuditedObjectSpace registram alterações que passam por eles.

**Lacunas e integração restante:** Separar trilha de alterações de histórico temporal, retenção e políticas de privacidade. Permissão de campo não substitui toda a gestão de dados pessoais.

**Backlog relacionado:** [#79](https://github.com/alexkads/RavenRS/issues/79), [#253](https://github.com/alexkads/RavenRS/issues/253), [#261](https://github.com/alexkads/RavenRS/issues/261). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/security/audit.rs](../crates/ravenrs-app/src/security/audit.rs), [crates/ravenrs-app/src/security/space.rs](../crates/ravenrs-app/src/security/space.rs), [docs/APP-CONTRACTS.md](../docs/APP-CONTRACTS.md).

**Fluxo para comprovar o consumo:** Uma ação de UI, API ou job mantém autoria/correlação; retenção e acesso ao histórico seguem configuração da aplicação. Requisitos jurídicos não são aferidos por esta matriz.

## A11 Processos, aprovações e tarefas: 75%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Máquina de estados e transições em código | 1 |
| Regras de aprovação fornecidas pela aplicação | 1 |
| Etapas e decisões persistidas | 1 |
| Caixa de tarefas por papel | 1 |
| Delegação com vigência | 1 |
| Retomada após reabrir armazenamento | 1 |
| Decisão idempotente por identificador | 1 |
| Decisões simultâneas protegidas atomicamente por versão | 0,5 |
| Escalonamento automático por prazo | 0 |
| Designer de processo versionado em execução | 0 |

**Já absorvido pelo framework:** O pacote opcional ravenrs-app-requests já resolve uma fatia durável e reutilizável, demonstrada em dois setores.

**Lacunas e integração restante:** Concorrência atômica #241, escalonamento e designer versionado. Fluxos paralelos, compensações e regras especiais dependem do recorte; não se promete BPMN completo.

**Backlog relacionado:** [#258](https://github.com/alexkads/RavenRS/issues/258), [#238](https://github.com/alexkads/RavenRS/issues/238), [#66](https://github.com/alexkads/RavenRS/issues/66), [#241](https://github.com/alexkads/RavenRS/issues/241), [#268](https://github.com/alexkads/RavenRS/issues/268). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-requests/README.md](../crates/ravenrs-app-requests/README.md), [crates/ravenrs-app-requests/src/lib.rs](../crates/ravenrs-app-requests/src/lib.rs), [crates/ravenrs-app/src/workflow.rs](../crates/ravenrs-app/src/workflow.rs).

**Fluxo para comprovar o consumo:** Uma aprovação com delegação reinicia na etapa correta; decisão repetida não duplica efeitos; uma proposta de mudança de processo preserva instâncias existentes.

## A12 Jobs, agendamento e notificações: 45%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Trabalho de ação fora da UI nativa | 1 |
| Progresso e pedido de cancelamento | 1 |
| Lembretes no aplicativo | 1 |
| Resultado e erro devolvidos à sessão | 1 |
| Agendamento periódico configurável | 0 |
| Fila durável de jobs com worker independente | 0 |
| Coordenação entre processos e recuperação de jobs | 0 |
| Notificações por canais com preferências | 0,5 |
| Caixa de notificações e leitura persistidas | 0 |
| Escalonamento e relatórios agendados sob identidade | 0 |

**Já absorvido pelo framework:** Action::background, Session::run_background e Reminder já existem. No web, o trabalho documentado roda inline; não confundir com um worker independente.

**Lacunas e integração restante:** Scheduler e execução durável, cancelamento cooperativo por provider, notificações com deduplicação e preferências. #66 cobre agendamento no app aberto, não toda a plataforma de jobs.

**Backlog relacionado:** [#66](https://github.com/alexkads/RavenRS/issues/66), [#79](https://github.com/alexkads/RavenRS/issues/79), [#255](https://github.com/alexkads/RavenRS/issues/255), [#258](https://github.com/alexkads/RavenRS/issues/258). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/job.rs](../crates/ravenrs-app/src/job.rs), [crates/ravenrs-app-ui/src/session/background.rs](../crates/ravenrs-app-ui/src/session/background.rs), [crates/ravenrs-app-ui/src/reminders.rs](../crates/ravenrs-app-ui/src/reminders.rs).

**Fluxo para comprovar o consumo:** Reiniciar um worker não perde uma tarefa; reexecutar respeita idempotência; o destinatário recebe a notificação permitida pelo canal configurado.

## A13 API, eventos e integrações: 75%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| CRUD protegido e filtros/páginas da API | 1 |
| Batch transacional e endpoints da aplicação | 1 |
| Descrição OpenAPI e chaves de API | 1 |
| Outbox gravado com a alteração | 1 |
| Webhook assinado e deduplicado | 1 |
| Dispatcher com retries, lease e reconciliação | 1 |
| Contexto autorizado do worker | 1 |
| Providers/conectores prontos para os principais serviços | 0,5 |
| Cancelamento efetivo de envio em andamento | 0 |
| Servidor concorrente e escala horizontal aferidos | 0 |

**Já absorvido pelo framework:** API, Provider/EventSink, outbox e webhooks já evitam reimplementar boa parte das integrações. O dispatcher atual é chamado no processo da aplicação.

**Lacunas e integração restante:** Conectores específicos opcionais, worker independente, propagação de timeout/cancelamento e operação monitorável. Não prometer exactly-once distribuído por existir deduplicação local.

**Backlog relacionado:** [#226](https://github.com/alexkads/RavenRS/issues/226), [#254](https://github.com/alexkads/RavenRS/issues/254), [#255](https://github.com/alexkads/RavenRS/issues/255), [#256](https://github.com/alexkads/RavenRS/issues/256), [#270](https://github.com/alexkads/RavenRS/issues/270), [#82](https://github.com/alexkads/RavenRS/issues/82). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-server/src/lib.rs](../crates/ravenrs-app-server/src/lib.rs), [crates/ravenrs-app-server/src/hooks.rs](../crates/ravenrs-app-server/src/hooks.rs), [crates/ravenrs-app/src/outbox.rs](../crates/ravenrs-app/src/outbox.rs), [crates/ravenrs-app/src/provider.rs](../crates/ravenrs-app/src/provider.rs).

**Fluxo para comprovar o consumo:** Uma mudança rejeitada não publica evento; resposta perdida é reconciliada; webhook repetido faz um efeito; nenhuma integração usa privilégio universal.

## A14 Importação, exportação e busca: 35%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Busca/filtros nas listas locais | 1 |
| Exportações comuns da lista | 1 |
| JSON de registros na persistência simples | 1 |
| Assistente CSV/XLSX por modelo | 0 |
| Conversão de referências e política de duplicados na importação | 0 |
| Erros por linha, prévia e atomicidade de lote | 0 |
| Exportação integral de consulta remota estável | 0,5 |
| Busca global autorizada entre entidades | 0 |
| Busca facetada com contagens autorizadas | 0 |
| Busca textual/semântica em anexos com escopo | 0 |

**Já absorvido pelo framework:** A lista e a persistência simples têm saídas próprias. Importar JSON que substitui a base não equivale a importar registros de negócio por usuário.

**Lacunas e integração restante:** Wizard CSV/XLSX, referências e deduplicação, busca transversal e exportação remota. Toda entrada passa por ObjectSpace, regras e auditoria.

**Backlog relacionado:** [#228](https://github.com/alexkads/RavenRS/issues/228), [#454](https://github.com/alexkads/RavenRS/issues/454), [#79](https://github.com/alexkads/RavenRS/issues/79), [#391](https://github.com/alexkads/RavenRS/issues/391). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-ui/src/list_view.rs](../crates/ravenrs-app-ui/src/list_view.rs), [crates/ravenrs-app/src/space.rs](../crates/ravenrs-app/src/space.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Importar uma planilha com linha inválida apresenta prévia e erros, sem escrita parcial indevida; busca/export respeitam exatamente o escopo do usuário.

## A15 Relatórios e saídas corporativas: 60%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Relatório da lista | 1 |
| Relatórios guardados como registros | 1 |
| Designer embutido | 1 |
| Dados de entidades, parâmetros e permissões | 1 |
| Saída PDF/XLSX do motor | 1 |
| Impressão por integração disponível | 0,5 |
| Filtro/busca iguais entre lista e relatório | 0,5 |
| Leitura em volume sem carregar tudo em memória | 0 |
| Entrega agendada executada com autorização | 0 |
| Mail merge integrado ao modelo | 0 |

**Já absorvido pelo framework:** ravenrs-app-report já une relatórios ao aplicativo. A avaliação inclui a integração e não somente formatos suportados pelo motor.

**Lacunas e integração restante:** Preservar filtro/escopo em todas as saídas, limitar memória, agendar com identidade e fornecer templates por entidade onde necessário.

**Backlog relacionado:** [#181](https://github.com/alexkads/RavenRS/issues/181), [#261](https://github.com/alexkads/RavenRS/issues/261), [#79](https://github.com/alexkads/RavenRS/issues/79). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-report/src/lib.rs](../crates/ravenrs-app-report/src/lib.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md), [docs/REPORTING-PARITY.md](../docs/REPORTING-PARITY.md).

**Fluxo para comprovar o consumo:** Lista, prévia, PDF e envio reproduzem o mesmo filtro e autorização; um relatório grande não pressupõe memória ilimitada.

## A16 Painéis e análise de negócio: 60%

Grupo: **Dependente do projeto**.

| Capacidade avaliada | Pontos |
|---|---:|
| DashboardInfo e DashboardView | 1 |
| Painéis analíticos declarados em código | 1 |
| KPIs e gráficos sobre entidades | 1 |
| Painéis editados como registros | 0,5 |
| Intervalos/filtros por tile | 1 |
| Mesma seleção/filtro entre card, gráfico e export | 0,5 |
| Designer visual com layout persistido | 0 |
| Pivot integrado ao modelo | 0 |
| Agregação remota e cache com escopo | 0,5 |
| Falha de um tile isolada e diagnosticável | 0,5 |

**Já absorvido pelo framework:** Há dashboards e AnalyticsModule/AnalyticsPanel. Editar os registros de um painel não fornece automaticamente um designer visual.

**Lacunas e integração restante:** Designer, crossfilter, pivot, layout persistente e agregação no provider; reaproveitar Chart e os controles existentes.

**Backlog relacionado:** [#68](https://github.com/alexkads/RavenRS/issues/68), [#203](https://github.com/alexkads/RavenRS/issues/203), [#403](https://github.com/alexkads/RavenRS/issues/403), [#432](https://github.com/alexkads/RavenRS/issues/432). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-ui/src/dashboard.rs](../crates/ravenrs-app-ui/src/dashboard.rs), [docs/APP-PARITY.md](../docs/APP-PARITY.md), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Card, lista, gráfico e exportação usam o mesmo período/escopo; dado protegido não aparece como zero; uma falha não inutiliza todo o painel.

## A17 Anexos e documentos emitidos: 40%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| AttachmentStore desacoplado do registro | 1 |
| Arquivos/imagens em formulários | 1 |
| Implementação de anexos em memória e SQLite | 1 |
| Download com autorização do registro no contrato | 0 |
| Limpeza/retenção de anexos substituídos | 0 |
| Upload grande em blocos com cancelamento | 0 |
| Documento emitido congelado na revisão usada | 0 |
| Hash/verificação do documento emitido | 0 |
| Ciclo de assinatura por contrato de provider | 0,5 |
| Preview de arquivos externos no app | 0,5 |

**Já absorvido pelo framework:** Anexos têm armazenamento próprio e UI. O contrato de armazenamento por chave não decide sozinho quem pode baixar um documento.

**Lacunas e integração restante:** Download autorizado, ciclo de vida dos arquivos, sessão de upload e revisão imutável na emissão. Assinatura manuscrita não é assinatura criptográfica.

**Backlog relacionado:** [#261](https://github.com/alexkads/RavenRS/issues/261), [#447](https://github.com/alexkads/RavenRS/issues/447), [#404](https://github.com/alexkads/RavenRS/issues/404), [#405](https://github.com/alexkads/RavenRS/issues/405). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/files.rs](../crates/ravenrs-app/src/files.rs), [crates/ravenrs-app-ui/src/editors.rs](../crates/ravenrs-app-ui/src/editors.rs), [docs/APP-PARITY.md](../docs/APP-PARITY.md).

**Fluxo para comprovar o consumo:** Conhecer a chave de um arquivo não permite baixá-lo; uma emissão antiga conserva dados, modelo e conteúdo mesmo após editar o template.

## A18 Colaboração e comunicação no registro: 30%

Grupo: **Dependente do projeto**.

| Capacidade avaliada | Pontos |
|---|---:|
| Widgets de conversa e anexos | 0,5 |
| Entidades genéricas de notas/pessoas | 1 |
| Trilha de alteração do registro | 1 |
| Conversa persistida e membros | 0 |
| Não lidas por participante | 0 |
| Comentários/menções ligados ao registro | 0 |
| Seguir registro e preferências | 0 |
| Política de histórico ao sair de um grupo | 0 |
| Entrega de mensagens com estado e retries | 0,5 |
| Presença e assinatura de mudanças autorizadas | 0 |

**Já absorvido pelo framework:** Widgets, LibraryModule, auditoria e eventos são peças aproveitáveis; a composição de colaboração precisa de módulo próprio.

**Lacunas e integração restante:** Histórico e participação, comentários/menções, não lidas, notificações e atualização de telas. #259 cobre conversas, sem cobrir toda colaboração de registro.

**Backlog relacionado:** [#259](https://github.com/alexkads/RavenRS/issues/259), [#79](https://github.com/alexkads/RavenRS/issues/79), [#255](https://github.com/alexkads/RavenRS/issues/255). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-library/src/lib.rs](../crates/ravenrs-app-library/src/lib.rs), [crates/ravenrs-app/src/security/audit.rs](../crates/ravenrs-app/src/security/audit.rs), [docs/APP-PARITY.md](../docs/APP-PARITY.md).

**Fluxo para comprovar o consumo:** Quem saiu de uma conversa vê somente o histórico permitido; mensagem reenviada não duplica; comentário e menção obedecem à permissão do registro.

## A19 Agenda, reservas e recursos: 40%

Grupo: **Dependente do projeto**.

| Capacidade avaliada | Pontos |
|---|---:|
| Lista como calendário mensal | 1 |
| Scheduler por recurso | 1 |
| Movimentação/redimensionamento pelo editor | 1 |
| Tipos de recorrência nos widgets | 0,5 |
| Série/ocorrência persistidas no modelo de app | 0 |
| Edição isolada de ocorrência e exceções | 0 |
| Política de fuso e horário de verão | 0,5 |
| Reserva concorrente da última vaga | 0 |
| Disponibilidade, buffers e bloqueios | 0 |
| Dia/semana por horas integrados e consistentes | 0 |

**Já absorvido pelo framework:** Listas podem usar Calendar/Scheduler. A presença de RecurrenceRule/RecurrenceEditor no widget não entrega persistência de série no aplicativo.

**Lacunas e integração restante:** Recorrência, exceções, fusos e reserva atômica; deixar elegibilidade, calendário e capacidade como políticas do consumidor.

**Backlog relacionado:** [#227](https://github.com/alexkads/RavenRS/issues/227), [#241](https://github.com/alexkads/RavenRS/issues/241), [#441](https://github.com/alexkads/RavenRS/issues/441). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-ui/src/list_editors.rs](../crates/ravenrs-app-ui/src/list_editors.rs), [docs/APP-PARITY.md](../docs/APP-PARITY.md), [docs/APP-CONTRACTS.md](../docs/APP-CONTRACTS.md).

**Fluxo para comprovar o consumo:** Duas reservas da última capacidade confirmam apenas uma; mudar uma ocorrência conserva a série; a hora permanece correta nas mudanças de fuso.

## A20 Customização e módulos reutilizáveis: 75%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| ModelLayers e diferenças persistíveis | 1 |
| Preferências/configurações por escopo | 1 |
| Editores, controladores e serviços substituíveis | 1 |
| ModuleManifest com versão/dependências | 1 |
| Validação de targets, capabilities e conflitos | 1 |
| Registros comuns prontos em LibraryModule | 1 |
| ModelEditor de todas as camadas/estruturas | 0,5 |
| Feature/license guard integrado em todas as superfícies | 0,5 |
| Distribuição de módulos e ferramentas de instalação | 0,5 |
| Extensões em runtime isoladas e versionadas | 0 |

**Já absorvido pelo framework:** Módulos e diferenças já evitam copiar funcionalidades; o modelo por camadas e manifestos formam uma base importante.

**Lacunas e integração restante:** Editor visual completo, propagação do guard, pontos de extensão e distribuição de pacotes. Não criar um sistema paralelo ao ModuleManifest.

**Backlog relacionado:** [#82](https://github.com/alexkads/RavenRS/issues/82), [#271](https://github.com/alexkads/RavenRS/issues/271), [#242](https://github.com/alexkads/RavenRS/issues/242), [#437](https://github.com/alexkads/RavenRS/issues/437), [#245](https://github.com/alexkads/RavenRS/issues/245). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app/src/manifest.rs](../crates/ravenrs-app/src/manifest.rs), [crates/ravenrs-app/src/model/layers.rs](../crates/ravenrs-app/src/model/layers.rs), [crates/ravenrs-app-ui/src/model_editor.rs](../crates/ravenrs-app-ui/src/model_editor.rs), [crates/ravenrs-app-library/src/lib.rs](../crates/ravenrs-app-library/src/lib.rs).

**Fluxo para comprovar o consumo:** O mesmo módulo é configurado em dois projetos sem copiar código; desativá-lo preserva leitura/exportação permitidas e bloqueia escrita também no serviço.

## A21 Cliente remoto, tempo real e offline: 35%

Grupo: **Dependente do projeto**.

| Capacidade avaliada | Pontos |
|---|---:|
| RemoteApp/RemoteDatabase sobre API | 1 |
| Persistência simples local/browser | 1 |
| Batch remoto com revalidação no serviço | 1 |
| Lista remota com paginação sem carregar toda a réplica | 0 |
| Falha assíncrona mantém draft/formulário recuperável | 0 |
| Notificações de mudanças entre clientes | 0 |
| Fila offline persistida e reconciliação | 0 |
| Permissão revogada durante offline é reavaliada | 0,5 |
| Referências a novos registros no mesmo commit remoto | 0 |
| Conflitos de sincronização com decisão explícita | 0 |

**Já absorvido pelo framework:** Há cliente remoto e armazenamento local. A réplica em memória do cliente remoto não é uma solução offline durável.

**Lacunas e integração restante:** Evitar perda de draft na rejeição remota; suportar paginação, referências em lote, eventos de atualização e sincronização com revalidação.

**Backlog relacionado:** [#226](https://github.com/alexkads/RavenRS/issues/226), [#240](https://github.com/alexkads/RavenRS/issues/240), [#241](https://github.com/alexkads/RavenRS/issues/241), [#332](https://github.com/alexkads/RavenRS/issues/332). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-remote/src/lib.rs](../crates/ravenrs-app-remote/src/lib.rs), [docs/APP-FRAMEWORK.md](../docs/APP-FRAMEWORK.md).

**Fluxo para comprovar o consumo:** Sem rede, alterações compatíveis ficam pendentes; ao retornar, o servidor confirma ou apresenta recusa/conflito sem perder o trabalho digitado.

## A22 Operação, recuperação e implantação: 55%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Backup SQLite consistente e inspecionável | 1 |
| Restore SQLite com verificação de stamp | 1 |
| Migrações e recusa de incompatibilidade | 1 |
| Health básico e falhas tipadas | 1 |
| CLI de diagnóstico da instalação/versões | 1 |
| Plano de backup/recuperação dos demais providers | 0,5 |
| Observabilidade correlacionada de ações/queries/jobs | 0 |
| Painel de operação e incidentes | 0 |
| Migração entre bancos/tenants e rollback operacional | 0 |
| Capacidade e implantação concorrente aferidas | 0 |

**Já absorvido pelo framework:** Backup/restore SQLite já existem em backup.rs (#229 fechada); cargo ravenrs doctor já diagnostica o ambiente de desenvolvimento.

**Lacunas e integração restante:** Documentar recuperação por adaptador, operação dos jobs, traces e ambientes. doctor verifica ferramentas, não substitui telemetria do app em produção.

**Backlog relacionado:** [#229](https://github.com/alexkads/RavenRS/issues/229), [#243](https://github.com/alexkads/RavenRS/issues/243), [#79](https://github.com/alexkads/RavenRS/issues/79), [#245](https://github.com/alexkads/RavenRS/issues/245). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-sqlite/src/backup.rs](../crates/ravenrs-app-sqlite/src/backup.rs), [crates/cargo-ravenrs/src/mcp.rs](../crates/cargo-ravenrs/src/mcp.rs), [crates/ravenrs-app-server/src/lib.rs](../crates/ravenrs-app-server/src/lib.rs).

**Fluxo para comprovar o consumo:** Restaurar numa instalação limpa confere aplicação, tenant e migrações; erro de provider tem diagnóstico útil; capacidade publicada vem de medição.

## A23 Consumo por IA, MCP e geração: 60%

Grupo: **Produtividade com IA**.

| Capacidade avaliada | Pontos |
|---|---:|
| MCP documental de API e exemplos | 1 |
| Busca de guias por seção e filtros | 1 |
| Contratos e paridade do app pelo MCP | 1 |
| Validação de expressões e layouts de relatório | 1 |
| CLI new/ai/doctor e instruções versionadas | 1 |
| Validação estruturada de uma diferença completa do app via ferramenta dedicada | 0 |
| Receitas curtas verificadas para fluxos corporativos | 0,5 |
| Geração incremental de entidade/módulo e saída JSON uniforme | 0,5 |
| Benchmark de tarefas com modelos menores e tokens medidos | 0 |
| MCP operacional gerado para dados/ações do aplicativo | 0 |

**Já absorvido pelo framework:** O MCP existente já reduz pesquisa e adivinhação; a CLI configura assistentes e versões. O servidor documental não opera os dados dos consumidores.

**Lacunas e integração restante:** Receitas completas, diagnóstico estruturado, validação de modelo e bateria com modelos menores. O MCP operacional #266 é uma capacidade distinta e opcional.

**Backlog relacionado:** [#81](https://github.com/alexkads/RavenRS/issues/81), [#245](https://github.com/alexkads/RavenRS/issues/245), [#266](https://github.com/alexkads/RavenRS/issues/266), [#274](https://github.com/alexkads/RavenRS/issues/274). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-mcp/README.md](../crates/ravenrs-mcp/README.md), [crates/ravenrs-mcp/src/server.rs](../crates/ravenrs-mcp/src/server.rs), [crates/cargo-ravenrs/README.md](../crates/cargo-ravenrs/README.md), [docs/AI_DEVELOPMENT.md](../docs/AI_DEVELOPMENT.md).

**Fluxo para comprovar o consumo:** Um modelo menor cria e modifica um fluxo corporativo usando somente APIs públicas, MCP e checks; medir todos os tokens e intervenções, não apenas o código final.

## A24 Testabilidade e contratos de consumo: 80%

Grupo: **Núcleo**.

| Capacidade avaliada | Pontos |
|---|---:|
| Testes de UI sem janela | 1 |
| Fixture com organizações/chaves repetidas | 1 |
| Relógio controlado | 1 |
| Provider falso com falhas e resposta perdida | 1 |
| Bateria de contrato reutilizável por adaptador | 1 |
| Capacidades/limites declarados pelo adaptador | 1 |
| Dois consumidores de setores distintos | 1 |
| Exemplo de referência para cada fluxo desta matriz | 0,5 |
| Matriz real de hosts/providers em cada entrega | 0,5 |
| Regressão de usabilidade com modelos menores por release | 0 |

**Já absorvido pelo framework:** Testkit, contratos de armazenamento, cenários C01–C19 e testes de UI já fornecem uma base de verificação reutilizável.

**Lacunas e integração restante:** Ampliar fluxos de referência e matriz de hosts/providers; um teste ignorado por faltar serviço não prova integração real.

**Backlog relacionado:** [#268](https://github.com/alexkads/RavenRS/issues/268), [#336](https://github.com/alexkads/RavenRS/issues/336), [#253](https://github.com/alexkads/RavenRS/issues/253), [#81](https://github.com/alexkads/RavenRS/issues/81). As referências podem cobrir somente parte da lacuna; não representam promessa de fechamento integral da área.

**Evidência:** [crates/ravenrs-app-testkit/README.md](../crates/ravenrs-app-testkit/README.md), [crates/ravenrs-app-testkit/src/contract.rs](../crates/ravenrs-app-testkit/src/contract.rs), [crates/ravenrs-app-ui/tests/suite/e2e.rs](../crates/ravenrs-app-ui/tests/suite/e2e.rs), [examples/contracts/associacao/Cargo.toml](../examples/contracts/associacao/Cargo.toml), [examples/contracts/cursos/Cargo.toml](../examples/contracts/cursos/Cargo.toml).

**Fluxo para comprovar o consumo:** Um pacote externo executa a bateria com um adaptador/provider falso e prova sucesso, recusa e retomada sem precisar dos dados de um produto.

## Gaps prioritários e encaminhamento

| Gap | Entrega reutilizável necessária | Encaminhamento |
|---|---|---|
| G01 Concorrência e commit desconhecido | Versão atômica, chaves robustas e reenvio idempotente | #241 e #332; não encerrar pelo merge de campos atual |
| G02 Unidade de trabalho do agregado | Pai/filhos, coleções e ações com fronteira de commit/cancelamento explícita | #79 para decompor; #434 melhora grid, mas não resolve sozinho a semântica do app |
| G03 Cliente remoto sem perda de edição | Preservar draft na rejeição, referências novas no lote e paginação do servidor | Lacunas documentadas em APP-FRAMEWORK; relacionar #226/#240/#241, sem reabrir automaticamente #226 |
| G04 Contexto de empresa | Resolver e propagar tenant a sessão, dados, arquivos, jobs e caches | #225; #242 para recursos/licença; #243 para migração |
| G05 Precisão do domínio | Tipos exatos por todas as camadas, critérios e formatos | #337 trata armazenamento; #453 trata grid; detalhar camada de app em #79 |
| G06 Importação e exportação de negócio | Wizard, referências, duplicados, validação por linha e export remoto | #228 e #454 |
| G07 Execução durável e notificações | Worker, fila, retomada, escalonamento, canais e preferências | #66 cobre scheduler no app; complementar via #79/#253 para o restante |
| G08 Identidade pronta para configurar | Provider OIDC, MFA conforme necessidade e gestão de vínculos | #257 está fechada e exclui provider real/MFA; decompor complemento, não declarar SSO pronto por completo |
| G09 Documento e arquivo autorizado | Download autorizado, revisão imutável, hash e ciclo de vida | #261 e #447; #404/#405 fornecem editores/viewers, sem substituir autorização |
| G10 Colaboração do registro | Comentários, participantes, menções, não lidas e política de histórico | #259 cobre conversas; outros recortes em #79 |
| G11 Histórico/privacidade e operação | Correlação, retenção, observabilidade e recuperação por provider | #79/#253; #229 já entrega backup SQLite, não todo o programa de operação |
| G12 Sincronização e telas atualizadas | Fila offline, autorização na volta, conflitos e notificações de alterações | #240/#241; assinaturas em tempo real precisam de contrato próprio |
| G13 Designer e customização | Designer sobre ModelLayers e guard aplicado em UI/serviço | #437/#402, #242 e #82; não gerar outro framework de telas |
| G14 Consumo por modelos menores | Receitas, validação estruturada, descoberta curta e medição | #81/#245; #266 é operação de dados por IA, distinta da assistência para construir apps |

Esta é uma lista de encaminhamentos. Onde há apenas épico ou issue relacionada, falta decompor o escopo antes da implementação. A busca não foi exaustiva a ponto de afirmar que não existe outra issue específica. Não foram abertas ou alteradas issues nesta revisão.

A #238 evolui o FlowCanvas e contribui com ferramentas de edição de grafos; ela
não especifica sozinha um designer de processos executáveis e versionados. Esse
complemento de A11 precisa de escopo próprio antes de ser considerado coberto.

## Priorização pelo trabalho poupado ao consumidor

**Primeiro: corrigir contratos que afetam vários fluxos.** G01–G06 evitam que cada projeto tenha de inventar concorrência, agregados, recuperação de formulário remoto, multiempresa, precisão e importação. Aplicações locais simples não precisam esperar por todos os itens.

**Depois: consolidar recursos corporativos reutilizáveis.** G07–G13 cobrem jobs, identidade, documentos, colaboração, operação, sincronização e customização. Escolher o recorte necessário aos próximos projetos e entregar o fluxo integrado.

**Durante todas as entregas: A23/A24.** Um recurso não está simples de consumir só porque tem uma API. Seu contrato, receita, exemplos e diagnóstico precisam ser encontrados por MCP e verificados com um consumidor externo.

Evitar criar dezenas de componentes para substituir recursos que já existem. UI vai em widgets/app-ui; motor e contratos ficam sem GUI; adaptadores e funcionalidades de domínio ficam em pacotes opcionais.

## Como verificar a economia de tokens e o uso de modelos menores

Definir uma bateria pequena de tarefas reais, com dados sintéticos e critérios automáticos. Fixar revisão do RavenRS, configuração do MCP, modelo/versão, ferramentas, orçamento e prompt; repetir para observar variabilidade.

| Tarefa de referência | Configuração e comportamento esperado |
|---|---|
| T01 Cadastro relacionado | Entidades, regra, lista/detalhe, relação e permissão por registro |
| T02 Ação de negócio | Transição válida, recusa explicada e auditoria |
| T03 Relatório | Filtro, parâmetros e PDF reproduzíveis sob autorização |
| T04 Aprovação | Etapas, delegação, reinício e decisão repetida sem efeito duplicado |
| T05 Importação | Mapeamento, duplicados e linha inválida; manter como tarefa bloqueada até entregar o gap |
| T06 Alteração incremental | Acrescentar campo/regra/relação em projeto existente sem regenerar seu código específico |
| T07 Integração | Evento, provider falso, resposta perdida e reconciliação |
| T08 Reuso em outro setor | Mesmo módulo em segundo app, com políticas diferentes e sem cópia |

Medir por tarefa: sucesso nos critérios, tokens totais de entrada/saída incluindo ferramentas e tentativas, uso de cache informado separadamente, chamadas MCP, falhas de compilação/teste, intervenções humanas, tempo e código repetitivo restante. Custo monetário depende do modelo e não é sinônimo de tokens.

Comparar o mesmo objetivo e os mesmos critérios, com repetições. Uma tarefa bloqueada por capacidade ausente não é “falha do modelo”. Um resultado que só funcionou após intervenção humana não entra como sucesso autônomo.

**Não há ganho percentual de tokens demonstrado nesta cobertura.** O objetivo é transformar a proposta em evidência: um modelo menor conseguir compor uma capacidade que já está resolvida, com pouco contexto e sem reinventar infraestrutura.

## Consulta pelo MCP existente

O build.rs de ravenrs-mcp inclui automaticamente os Markdown de docs no binário. Após reconstruir/instalar a versão que contém esta documentação, usar a ferramenta guides com estes argumentos:

Listar seções do documento:

```json
{"document":"APP-COVERAGE"}
```

Ler somente a área de consumo por IA:

```json
{"document":"APP-COVERAGE","section":"A23","limit":1}
```

Para a orientação de implementação, cruzar component_api/search_api/component_examples com guides de APP-FRAMEWORK, app_contracts e parity_status com document = application. As notas desta cobertura são retrato de uma base; API e limites da versão instalada continuam sendo o contrato de consumo.

O novo guia não adiciona ferramenta nem muda permissões do servidor. Um binário instalado antes desta revisão não recebe o documento sem atualização.

## Atualização e conclusão de uma capacidade

Para cada entrega: atualizar evidência e base auditada; revisar somente os itens afetados; recalcular a nota; ligar o exemplo/contrato público e registrar os hosts/providers realmente verificados.

“100%” significa que os dez itens daquele recorte foram comprovados como consumíveis, não que todos os recursos de todo software corporativo existam. Reavaliar o recorte quando um projeto real revelar requisito recorrente.

O resultado desejado é que o consumidor declare seu negócio e use os contratos existentes. Não exigir que ele reimplemente infraestrutura genérica; não exigir que toda regra específica de um setor entre no núcleo.

## Capacidades opcionais por domínio

Cobrança/recebimento (#262), passes/dispositivos (#263), formulários publicados (#260), páginas/portal (#264), planejamento (#457), fiscal e pagamentos por providers têm valor, mas não são requisitos obrigatórios para todo app. Mantê-los em pacotes opcionais, sobre autorização, eventos, documentos e contratos comuns.

A profundidade setorial deve vir de um projeto consumidor e de dois usos distintos antes de generalizar. A [biblioteca existente](../crates/ravenrs-app-library/README.md) já oferece registros comuns; ela não deve fixar alçadas, elegibilidade, impostos ou políticas de um setor.

## Verificação desta revisão documental

- Recalculadas as 240 notas e os 24 índices; conferidos os links locais de evidência.
- Check padrão do repositório aprovado em 18 s (17 s na tabela de etapas), com build reutilizado; alteração documental não selecionou testes de pacotes.
- Executados separadamente os testes de ravenrs-mcp: 11 unitários e 22 de integração aprovados.
- Reconstruído o binário do MCP e consultado por um cliente stdio real: guides lista o documento e devolve cada uma das 24 áreas por seção, sem truncamento.
- A consulta de A23 retornou 1.913 caracteres; isso demonstra consulta pontual, sem constituir contagem de tokens nem ganho comprovado para um modelo.
- Não foram executados os futuros fluxos corporativos, benchmarks de modelos ou integrações externas desta matriz.
