# Capacidades de planejamento para aplicações RavenRS

Data: 15/11/2026. Coordenação: [#457](https://github.com/alexkads/RavenRS/issues/457). Base auditada: `6b50ff874`. Este documento especifica backlog; nenhum motor/componente novo foi implementado nesta revisão.

## Resultado esperado

Uma aplicação de referência executável deve permitir estruturar projetos e WBS, planejar tarefas, calcular CPM, atribuir/nivelar recursos, lançar realizado, comparar baseline/cenários, analisar custos/EVM e trocar dados. As referências são **Primavera P6 Professional/EPPM** e **Microsoft Project desktop**, com diferenças de comportamento explícitas.

## Responsabilidades

| Camada | Entregas | Reuso obrigatório |
|---|---|---|
| Modelo/motores sem GUI | Calendários úteis, CPM, progresso, esforço, baseline, nivelamento e custos/EVM | Tipos temporais compatíveis, critérios e tipos exatos existentes; sem widgets ou banco no cálculo |
| Componentes | Gantt/TreeGrid/DataGrid evoluídos, WorkCalendarEditor, TimePhasedGrid e ActivityNetwork | Renderer, layout, foco, overlay, Chart, FlowCanvas e fachada pública |
| Aplicação | Módulo, comandos, revisão, autorização, auditoria e tarefas longas | ravenrs-app e adaptadores/serviço existentes |
| Intercâmbio/saída | P6 XML, XER, Project XML, PDF/XLSX/CSV | Providers e pipeline de relatório/exportação existentes |
| Referência | Fluxos integrados com fixtures e documentação | Consumir biblioteca; não alojar algoritmos no exemplo |

## Entregas novas

| Issue | Entrega | Dependências novas diretas |
|---|---|---|
| [#459](https://github.com/alexkads/RavenRS/issues/459) | Planejamento: modelo versionado de projetos, WBS, atividades, relações e dicionários | Modelo/base existente |
| [#460](https://github.com/alexkads/RavenRS/issues/460) | Calendário de trabalho: turnos, exceções, fusos e aritmética de duração útil | #459 |
| [#461](https://github.com/alexkads/RavenRS/issues/461) | WorkCalendarEditor: editar jornadas, exceções e atribuições de calendários | #460 |
| [#462](https://github.com/alexkads/RavenRS/issues/462) | Motor CPM: datas cedo/tarde, folgas, restrições e explicação do cronograma | #459, #460 |
| [#463](https://github.com/alexkads/RavenRS/issues/463) | Planejamento: data de corte, realizado, tarefas manuais e reprogramação do restante | #462 |
| [#464](https://github.com/alexkads/RavenRS/issues/464) | Planejamento: atribuições, capacidade e tipos de duração/esforço dos recursos | #459, #460 |
| [#465](https://github.com/alexkads/RavenRS/issues/465) | Planejamento: baselines, cenários what-if e comparação por atividade | #459, #462 |
| [#466](https://github.com/alexkads/RavenRS/issues/466) | Nivelamento de recursos: prioridades, limites e proposta explicável de replanejamento | #462, #464, #463 |
| [#467](https://github.com/alexkads/RavenRS/issues/467) | Planejamento: custos, tarifas vigentes, despesas e distribuição financeira por período | #464, #463 |
| [#468](https://github.com/alexkads/RavenRS/issues/468) | Planejamento: valor agregado, indicadores de desempenho e curvas de avanço | #465, #467 |
| [#469](https://github.com/alexkads/RavenRS/issues/469) | TimePhasedGrid: grade temporal de uso, capacidade e valores por período | #464, #467 |
| [#470](https://github.com/alexkads/RavenRS/issues/470) | ActivityNetwork: rede de precedências e rastreamento da lógica sobre FlowCanvas | #462 |
| [#471](https://github.com/alexkads/RavenRS/issues/471) | Intercâmbio de planejamento: importar e exportar Primavera P6 XML com matriz de fidelidade | #459, #460 |
| [#472](https://github.com/alexkads/RavenRS/issues/472) | Intercâmbio de planejamento: importar e exportar Primavera XER com versões explícitas | #459, #460 |
| [#473](https://github.com/alexkads/RavenRS/issues/473) | Intercâmbio de planejamento: Project XML com tarefas manuais, recursos e baselines | #459, #460 |
| [#474](https://github.com/alexkads/RavenRS/issues/474) | ravenrs-app: módulo de planejamento com comandos, snapshots e aplicação de cálculos | #459, #462, #463 |
| [#475](https://github.com/alexkads/RavenRS/issues/475) | Planejamento: impressão e exportação paginada de grade e cronograma sincronizados | #459, #465 |
| [#476](https://github.com/alexkads/RavenRS/issues/476) | Planejamento: apontamento de horas e avanço com aprovação e períodos encerrados | #464, #463, #474 |
| [#477](https://github.com/alexkads/RavenRS/issues/477) | Planejamento: EPS, portfólio e coordenação de múltiplos projetos com recursos compartilhados | #459, #462, #464, #474 |
| [#478](https://github.com/alexkads/RavenRS/issues/478) | Aplicação de referência de planejamento: fluxo completo com Gantt, recursos, baseline e intercâmbio | #474, #465, #464, #466, #468, #469, #470, #471, #472, #473, #475, #476, #477, #461 |

## Componentes existentes a melhorar

- [#401](https://github.com/alexkads/RavenRS/issues/401): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#406](https://github.com/alexkads/RavenRS/issues/406): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#410](https://github.com/alexkads/RavenRS/issues/410): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#434](https://github.com/alexkads/RavenRS/issues/434): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#435](https://github.com/alexkads/RavenRS/issues/435): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#436](https://github.com/alexkads/RavenRS/issues/436): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#441](https://github.com/alexkads/RavenRS/issues/441): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#451](https://github.com/alexkads/RavenRS/issues/451): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#452](https://github.com/alexkads/RavenRS/issues/452): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#453](https://github.com/alexkads/RavenRS/issues/453): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#454](https://github.com/alexkads/RavenRS/issues/454): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#455](https://github.com/alexkads/RavenRS/issues/455): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#203](https://github.com/alexkads/RavenRS/issues/203): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#399](https://github.com/alexkads/RavenRS/issues/399): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#241](https://github.com/alexkads/RavenRS/issues/241): cenário de planejamento acrescentado sem remover critérios anteriores.
- [#227](https://github.com/alexkads/RavenRS/issues/227): cenário de planejamento acrescentado sem remover critérios anteriores.

## Marcos verificáveis

1. **Planejar e persistir:** projeto/WBS → atividades/calendário/vínculos → CPM → grade/Gantt → salvar/reabrir.
2. **Controlar e replanejar:** baseline → realizado/data de corte → cálculo → variações/caminho crítico → proposta de nivelamento confirmável.
3. **Analisar e colaborar:** distribuição temporal/custos/EVM → apontamentos/aprovação → portfólio/cenários → concorrência e autorização.
4. **Intercambiar e publicar:** prévia/importação/exportação por formato, relatório paginado e matriz de fidelidade.

Abrir issues e entregar tipos não encerra os marcos. A aplicação [#478](https://github.com/alexkads/RavenRS/issues/478) precisa provar os fluxos com as mesmas APIs usadas por apps externos.

## Fixtures mínimas

- **CPM:** A=2; B=3 e C=1 após A; D=2 após B e C, em unidades úteis e calendário comum. Término 7, folga 2 para C. Somar testes dos quatro vínculos, lags, feriado, turno e restrição impossível.
- **Nivelamento:** duas tarefas independentes de dois dias, um recurso com capacidade 1. Proposta sequencial termina em quatro dias, com desempate definido e sem sobrecarga.
- **EVM:** BAC=1000, PV=400, EV=300, AC=500 → CV=-200, SV=-100, CPI=0,6 e SPI=0,75. ETC/EAC dependem da fórmula configurada.
- **Concorrência:** calcular snapshot R1, editar calendário em R2 e tentar aplicar R1 deve gerar conflito, preservando o trabalho atual.
- **Round-trip:** arquivos sintéticos com versão, ids, calendário, relações e progresso; relatório de campos preservados/transformados/recusados. Não pressupor resultado igual entre motores configurados de modo diferente.

## Diferenças relevantes do Microsoft Project

- Modo manual permite tarefa incompleta/estimada; automático usa dependências, restrições e calendários. Cobertura em [#463](https://github.com/alexkads/RavenRS/issues/463).
- Fixed Work/Units/Duration e effort-driven não são aliases dos tipos P6. Tabela de regras em [#464](https://github.com/alexkads/RavenRS/issues/464).
- Task Usage/Resource Usage precisam da grade temporal [#469](https://github.com/alexkads/RavenRS/issues/469), não só Gantt.
- Baselines e comparação [#465](https://github.com/alexkads/RavenRS/issues/465), redistribuição [#466](https://github.com/alexkads/RavenRS/issues/466), rede [#470](https://github.com/alexkads/RavenRS/issues/470) e impressão [#475](https://github.com/alexkads/RavenRS/issues/475) têm contratos próprios.
- [Project XML #473](https://github.com/alexkads/RavenRS/issues/473) não é P6 XML nem arquivo binário .mpp. Compatibilidade .mpp não foi declarada.

## Inventário de componentes

O [inventário e projeção](COMPONENT-INVENTORY.md) mantém a contagem comparável ao catálogo. Uma issue de edição, formato ou desempenho não acrescenta automaticamente um componente. Motores e módulos são capacidades separadas.

## Referências primárias

- [p6](https://docs.oracle.com/cd/G48902_01/client_help/en_US/toc.htm)
- [activities](https://docs.oracle.com/cd/G48902_01/client_help/en_US/activities_window.htm)
- [calendar](https://docs.oracle.com/cd/G48902_01/client_help/en_US/calendars.htm)
- [cpm](https://docs.oracle.com/cd/G48902_01/client_help/en_US/scheduling_projects.htm)
- [constraints](https://docs.oracle.com/cd/G48902_01/client_help/en_US/types_of_constraints.htm)
- [date](https://docs.oracle.com/cd/G48902_01/client_help/en_US/using_the_data_date.htm)
- [progress](https://docs.oracle.com/en/industries/construction-engineering/primavera-p6-project/26/p6-help/99348.html)
- [effort](https://docs.oracle.com/cd/G48902_01/client_help/en_US/synchronizing_activity_duration_units_and_resource_units_time.htm)
- [baseline](https://docs.oracle.com/cd/G48902_01/client_help/en_US/baselines.htm)
- [level](https://docs.oracle.com/cd/G48902_01/client_help/en_US/levelingresources.htm)
- [evm](https://docs.oracle.com/cd/G48902_01/client_help/en_US/earned_value.htm)
- [import](https://docs.oracle.com/cd/G48897_01/p6help/pt_BR/6604_1.htm)
- [profiles](https://docs.oracle.com/cd/G48902_01/client_help/en_US/resource_analysis_tab_-_user_preferences_dialog_box.htm)
- [network](https://docs.oracle.com/cd/G48902_01/client_help/en_US/create_relationships_in_the_activity_network.htm)
- [health](https://docs.oracle.com/cd/G48902_01/client_help/en_US/101393.htm)
- [ms](https://support.microsoft.com/en-us/project/how-project-schedules-tasks-behind-the-scenes)
- [msmode](https://support.microsoft.com/en-us/project/task-mode-task-field)
- [mseffort](https://support.microsoft.com/en-us/project/effort-driven-task-field)
- [msxml](https://learn.microsoft.com/en-us/office-project/xml-data-interchange/introduction-to-project-xml-data?view=project-client-2016)

## Verificação desta revisão

Foram criadas uma issue de coordenação e 20 entregas, e atualizadas 16 issues existentes. As respostas do GitHub confirmaram os textos salvos. Não foram executados os futuros cenários nem declarados recursos disponíveis; a implementação pertence às issues.
