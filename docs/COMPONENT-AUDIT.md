# Auditoria técnica de componentes

Data: **15/11/2026**. Base local: **`6b50ff874`**. 53 cenários/capacidades revisados nesta rodada, incluindo 28 de componentes básicos. Não é uma contagem de componentes nem um percentual de paridade.

## Método e força da evidência

A auditoria parte do comportamento, da acessibilidade e dos limites que se esperam de cada família de controle em aplicações desktop e web, e cruza cada expectativa com a API e o código do RavenRS. Um recurso que só faz sentido numa plataforma é registrado como diferença de host, não como lacuna geral.

Cada lacuna registrada cruza a referência com API/código local ou uma limitação explicitamente documentada do RavenRS. **Base presente** não significa paridade completa. **Parcial** identifica uma fronteira observada. **Planejado** aponta backlog, não implementação. **Verificar interação** e **Verificar fronteira** indicam que inspeção estática não prova falha nem comportamento completo; não geram bug confirmado.

Esta rodada é leitura técnica e inspeção estática. Não foram executados aplicativos dos fornecedores, benchmarks, validação visual, leitor de tela ou testes de runtime do RavenRS. O aceite das issues precisa cobrir essas interações durante implementação. Não foi feita auditoria exaustiva de todas as propriedades de todos os catálogos.

## Componentes básicos

| Capacidade | Situação | Evidência e limite real | Encaminhamento |
|---|---|---|---|
| Button / IconButton | Base presente | loading, disabled, selected, full_width e nome acessível existem. Atalho exibido por .shortcut não registra comando. [Código](../crates/ravenrs-widgets-base/src/button.rs). | Manter; exemplo deve ligar o atalho por Hotkey/BuildCx::shortcut. |
| Checkbox | Parcial | Binding<bool>; não expõe mixed/indeterminate reutilizável. A seleção parcial própria da árvore não supre o controle público. [Código](../crates/ravenrs-widgets-base/src/inputs.rs). | [#439](https://github.com/alexkads/RavenRS/issues/439) |
| Toggle / Stepper | Base presente | EditorState já distingue disabled/readonly; serve de base para uniformizar outros controles. [Código](../crates/ravenrs-widgets-base/src/inputs.rs). | Reutilizar em [#440](https://github.com/alexkads/RavenRS/issues/440). |
| RadioGroup / SegmentedControl | Parcial | Não expõem estados do grupo nem itens indisponíveis; seleção por índice exige cuidado com lista mutável. [Código](../crates/ravenrs-widgets-base/src/inputs.rs). | [#440](https://github.com/alexkads/RavenRS/issues/440) |
| Slider | Parcial | Tem faixa, passo, marcas e formato; falta contrato público de disabled/readonly. [Código](../crates/ravenrs-widgets-base/src/inputs.rs). | [#440](https://github.com/alexkads/RavenRS/issues/440) |
| RangeSlider | Ausente | Slider único não equivale a intervalo com duas alças. Foco/limites dos dois valores precisam ser definidos. [Código](../docs/PARITY.md). | [#417](https://github.com/alexkads/RavenRS/issues/417) |
| TextField / TextArea | Base presente | Texto, clipboard, seleção e estados existem; não inferir validação tipada universal a partir da edição textual. [Código](../crates/ravenrs-widgets-base/src/text_field.rs). | Integração em [#444](https://github.com/alexkads/RavenRS/issues/444); sem novo editor de texto. |
| NumberField | Parcial | None e draft inválido já existem; valor confirmado pode continuar antigo enquanto texto é inválido. Money passa por f64, logo não tem precisão i64 ilimitada. [Código](../crates/ravenrs-widgets-kit/src/editors.rs). | [#444](https://github.com/alexkads/RavenRS/issues/444) e [#453](https://github.com/alexkads/RavenRS/issues/453); preservar parse atual e declarar limites. |
| MaskedField | Parcial | 0/a/* e escape; extensão para posições opcionais e estado completo não está exposta. [Código](../crates/ravenrs-widgets-kit/src/editors.rs). | [#445](https://github.com/alexkads/RavenRS/issues/445) |
| Select | Base presente | Há Select::of por valor, busca opcional e estados; não abrir outra issue apenas por faltar seleção por chave no Combobox. [Código](../crates/ravenrs-widgets-base/src/overlay.rs). | Preservar API existente; distinguir de [#442](https://github.com/alexkads/RavenRS/issues/442). |
| Combobox | Parcial | Pesquisa em Vec<String> e Binding<Option<usize>>; não oferece texto livre/autocomplete editável. [Código](../crates/ravenrs-widgets-base/src/floating.rs). | [#442](https://github.com/alexkads/RavenRS/issues/442) |
| LookupField | Parcial | Consulta remota e descarte por geração já existem. Request só contém query/limit, sem próxima página nem resolução pública por chave. [Código](../crates/ravenrs-widgets-kit/src/lookup.rs). | [#443](https://github.com/alexkads/RavenRS/issues/443) |
| TagBox | Parcial | allow_new e limite existem; opções são locais. Selecionados de catálogo remoto exigem conservação por chave. [Código](../crates/ravenrs-widgets-kit/src/tag_box.rs). | [#443](https://github.com/alexkads/RavenRS/issues/443) |
| Calendar | Parcial | range min/max já existe; não há política pública de datas bloqueadas com motivo compartilhada entre seletores. [Código](../crates/ravenrs-widgets-base/src/calendar.rs). | [#441](https://github.com/alexkads/RavenRS/issues/441) |
| DatePicker | Parcial | Não propaga limites da API Calendar; valor é opcional. Não confundir ausência do builder com ausência do cálculo de calendário. [Código](../crates/ravenrs-widgets-base/src/calendar.rs). | [#441](https://github.com/alexkads/RavenRS/issues/441) |
| DateRangePicker | Parcial | Faixa e presets já existem. Falta política de dias bloqueados dentro do intervalo e limpeza consistente. [Código](../crates/ravenrs-widgets-base/src/calendar.rs). | [#441](https://github.com/alexkads/RavenRS/issues/441) |
| TimePicker / DateTimePicker | Parcial | Tem passo de minutos e estado opcional; não há limites/bloqueios configuráveis equivalentes. [Código](../crates/ravenrs-widgets-kit/src/editors.rs). | [#441](https://github.com/alexkads/RavenRS/issues/441) |
| FormField / validação | Parcial | validate recebe Binding<String>; error_text já serve a erro externo. Falta estado tipado compartilhado com erro de parse/draft. [Código](../crates/ravenrs-widgets-base/src/extras.rs). | [#444](https://github.com/alexkads/RavenRS/issues/444) |
| ColorPicker | Base presente | Paleta, transparência, hexadecimal e estados. Gradientes da barra desenhada não significam um editor de gradientes. [Código](../crates/ravenrs-widgets-base/src/color_picker.rs). | Não assumir que a referência de ColorPicker exige editor de gradientes. |
| Tooltip | Parcial | label_control dá descrição acessível, mas a bolha visual depende apenas de is_hover_within. [Código](../crates/ravenrs-widgets-base/src/overlay.rs). | [#446](https://github.com/alexkads/RavenRS/issues/446) |
| Dialog / Popover | Verificar interação | Dialog configura fechamento; runtime já contém modal_root/modal_return e confinamento de foco. Não marcar foco modal como ausente. [Código](../crates/ravenrs-widgets-base/src/ui.rs). | Validar combinações modal → menu → popup por testes; nenhuma falha reproduzida nesta auditoria. |
| Menus e Tabs | Verificar interação | MenuItem suporta submenu/check/template; navegação e tabs existem. Verificar roving focus, Escape e opções indisponíveis na suíte real. [Código](../crates/ravenrs-widgets-base/src/overlay.rs). | Não criar duplicata por presença de nomes no catálogo; usar documentação de teclado. |
| FileUpload / DropZone | Parcial | Seleção, limites e recusa existem; bytes do arquivo ficam em memória. Transferência/progresso não fazem parte do controle. [Código](../crates/ravenrs-widgets-kit/src/files.rs). | [#447](https://github.com/alexkads/RavenRS/issues/447) |
| FileBrowser | Parcial | Lista virtual, seleção e breadcrumb existem; gerenciar operações não equivale a abrir um arquivo. [Código](../crates/ravenrs-widgets-lists/src/lists/file_browser.rs). | [#418](https://github.com/alexkads/RavenRS/issues/418) consome [#447](https://github.com/alexkads/RavenRS/issues/447). |
| ListBox / transferência | Ausente | VirtualList, CardView e SortableList não fornecem juntos seleção múltipla e transferência/reordenação virtualizadas. [Código](../crates/ravenrs-widgets-lists/src/lists/virtual_list.rs). | [#448](https://github.com/alexkads/RavenRS/issues/448) |
| FloatingActionButton | Ausente | FloatingPanel abre conteúdo; não é a ação flutuante simples planejada no PARITY. [Código](../docs/PARITY.md). | [#449](https://github.com/alexkads/RavenRS/issues/449) |
| SpeedDial | Ausente | Grupo curto de ações expansíveis precisa de estado/foco/menu; não basta desenhar botões num círculo. [Código](../docs/PARITY.md). | [#450](https://github.com/alexkads/RavenRS/issues/450) |
| Signature | Planejado | Captura manuscrita #416 é separada de assinatura digital, anotação PDF e certificados. [Código](../docs/PARITY.md). | [#416](https://github.com/alexkads/RavenRS/issues/416) |

## Dados, documentos, gráficos e planejamento

| Capacidade | Situação | Evidência e limite real | Encaminhamento |
|---|---|---|---|
| DataGrid / linhas e colunas | Parcial | A janela de linhas é virtualizada; cada GridRow ainda percorre todas as colunas mostradas. Scroll horizontal não é virtualização horizontal. [Código](../crates/ravenrs-widgets-grid/src/grid/view.rs). | [#451](https://github.com/alexkads/RavenRS/issues/451) |
| DataGrid / seleção e clipboard | Parcial | Cópia TSV de linhas já existe; intervalo de células e colagem validada precisam de modelo próprio. [Código](../ARCHITECTURE.md). | [#452](https://github.com/alexkads/RavenRS/issues/452) |
| DataGrid / Value | Parcial | Number(f64), Date e tipos básicos não preservam todo inteiro/decimal/datetime. Persistência decimal #337 não resolve a camada de UI/export. [Código](../crates/ravenrs-widgets-grid/src/grid/model.rs). | [#453](https://github.com/alexkads/RavenRS/issues/453) |
| DataGrid / export remoto | Parcial | toolbar_view exporta self.data.rows(); modo remoto tem QueryResult apenas com totais. Falta provider export_all do conjunto filtrado. [Código](../crates/ravenrs-widgets-grid/src/grid/view.rs). | [#454](https://github.com/alexkads/RavenRS/issues/454) |
| DataGrid / estrutura de colunas | Parcial | Pinned é bool/esquerda; bandas e lado direito estão explicitamente planejados. [Código](../crates/ravenrs-widgets-grid/src/grid/layout.rs). | [#455](https://github.com/alexkads/RavenRS/issues/455) |
| DataGrid / lote × virtualização | Planejado | Não aceitar checklist isolado. Há implementações que restringem essas combinações; o RavenRS deve testar suas próprias combinações, sem herdá-las como restrição automática. [Código](../crates/ravenrs-widgets-grid/src/grid/view.rs). | [#434](https://github.com/alexkads/RavenRS/issues/434) |
| DataGrid / editores tipados | Planejado | Edição padrão usa TextField e parse; template de exibição não equivale ao ciclo de editor tipado. [Código](../crates/ravenrs-widgets-grid/src/grid/view.rs). | [#435](https://github.com/alexkads/RavenRS/issues/435) depende de [#444](https://github.com/alexkads/RavenRS/issues/444)/[#453](https://github.com/alexkads/RavenRS/issues/453) quando aplicável. |
| Dados remotos / grupos | Planejado | GridRequest não leva árvore/caminho de agrupamento. Contagem/total de cache não pode representar o dataset filtrado completo. [Código](../crates/ravenrs-widgets-grid/src/grid/remote.rs). | [#410](https://github.com/alexkads/RavenRS/issues/410) |
| TreeGrid / relações | Planejado | TreeGrid de esquema comum #401 difere de filha com schema próprio #436; testar expansão, cache e foco com lote/virtualização. [Código](../crates/ravenrs-widgets-grid/src/grid/view.rs). | [#401](https://github.com/alexkads/RavenRS/issues/401) e [#436](https://github.com/alexkads/RavenRS/issues/436). |
| Pivot | Planejado | Agrupamento da grade não é cruzamento de dimensões; matriz, total geral e agregações exigem aceites próprios. [Código](../docs/PARITY.md). | [#403](https://github.com/alexkads/RavenRS/issues/403) |
| Spreadsheet / motor e UI | Parcial | execute/with_session permitem comandos do motor; a superfície visual não expõe automaticamente todos eles. Mesclas já são desenhadas. [Código](../crates/ravenrs-spreadsheet/src/surface.rs). | [#411](https://github.com/alexkads/RavenRS/issues/411) |
| Spreadsheet / proteção e validação | Verificar fronteira | Não declarar ausentes no motor por falta de Ribbon. Auditar comandos, enforcement no widget e round-trip separadamente. [Código](../crates/ravenrs-spreadsheet/src/state.rs). | [#411](https://github.com/alexkads/RavenRS/issues/411) |
| DocumentEditor | Planejado | ravenrs-word é headless; paginar/editar/selecionar na tela requer outro fluxo sobre o motor existente. [Código](../crates/ravenrs-word/README.md). | [#405](https://github.com/alexkads/RavenRS/issues/405) |
| PdfViewer | Planejado | ravenrs-pdf lê/renderiza, mas não há viewer integrado. Texto pesquisável, hit-test e posições após zoom/rotação precisam usar a mesma geometria. [Código](../crates/ravenrs-pdf/README.md). | [#404](https://github.com/alexkads/RavenRS/issues/404) |
| PDF / capacidades avançadas | Fora do viewer básico | Anotações, formulários, assinatura e remoção definitiva de conteúdo são capacidades distintas. Não reivindicar pela presença de viewer/PNG. [Código](../crates/ravenrs-pdf/src/lib.rs). | Matriz de capacidades em [#404](https://github.com/alexkads/RavenRS/issues/404); não inflar seu aceite básico. |
| Chart / eixos | Parcial | Modelo atual usa categorias/valores; implementar tempo/log/múltiplos eixos exige política de zero, negativos e intervalos irregulares. [Código](../crates/ravenrs-graphics/src/chart.rs). | [#420](https://github.com/alexkads/RavenRS/issues/420) |
| Chart / export | Planejado | Export offline/local requer capacidades reais; dependências e fallback remoto da referência não devem virar suposição no RavenRS. [Código](../crates/ravenrs-widgets-charts/src/chart.rs). | [#423](https://github.com/alexkads/RavenRS/issues/423) |
| Chart / grande volume | Planejado | Rasterização e cache não equivalem a redução de pontos. Modos acelerados costumam perder recursos; descrever troca de modo e fidelidade. [Código](../crates/ravenrs-widgets-charts/src/chart.rs). | [#424](https://github.com/alexkads/RavenRS/issues/424) |
| Stock / agrupamento | Planejado | Redução por pixels difere de agregação de negócio. Média para linha, soma para volume e OHLC não são intercambiáveis. [Código](../crates/ravenrs-graphics/src/chart.rs). | [#424](https://github.com/alexkads/RavenRS/issues/424) e [#426](https://github.com/alexkads/RavenRS/issues/426). |
| Stock / indicadores | Planejado | Candles existem; séries derivadas devem ligar dados por id/tempo e usar histórico suficiente, não só o viewport. [Código](../crates/ravenrs-widgets-charts/src/chart.rs). | [#426](https://github.com/alexkads/RavenRS/issues/426) |
| Gráficos / acessibilidade | Parcial | Resumo falado atual tem limite; tabela e sonificação complementam, não substituem, navegação e semântica. [Código](../crates/ravenrs-widgets-charts/src/chart.rs). | [#427](https://github.com/alexkads/RavenRS/issues/427) |
| Agenda / tempo e fuso | Parcial | TimePoint declara minuto sem fuso. RRULE só cobre o subconjunto explícito, com exceções por Date; não equivale a calendário corporativo completo. [Código](../crates/ravenrs-widgets-base/src/recurrence.rs). | [#227](https://github.com/alexkads/RavenRS/issues/227) |
| Agenda / ocorrência versus série | Parcial | RecurrenceSet já tem EXDATE por data; faltam integração e persistência coordenada dos casos editar/excluir uma ocorrência, série e fuso. [Código](../crates/ravenrs-widgets-scheduler/src/event_calendar.rs). | [#227](https://github.com/alexkads/RavenRS/issues/227) |
| Gantt / motor de planejamento | Planejado | A issue de UI não equivale a CPM, baseline, nivelamento e intercâmbio Primavera. Separar motor genérico de regras do produto. [Código](../crates/ravenrs-widgets-scheduler/src/scheduler/view.rs). | [#406](https://github.com/alexkads/RavenRS/issues/406) |
| Designer de apps | Planejado | Playground, CLI, publicação de formulário e dashboard já têm issues próprias. Designer #437 precisa consumir APIs básicas e não reimplementar controles. [Código](../crates/ravenrs-project/README.md). | [#437](https://github.com/alexkads/RavenRS/issues/437) |

## Correções em relação a comparações por catálogo

- Rolagem horizontal e virtualização horizontal da DataGrid são capacidades diferentes. O código atual virtualiza linhas, mas constrói todas as colunas mostradas de cada linha. A tabela de ARCHITECTURE foi corrigida para explicitar isso.
- Autocomplete editável e seletor pesquisável são capacidades diferentes. O Combobox atual oferece a segunda; Select::of já resolve seleção por valor em outro componente.
- Um controle que mostra um atalho não necessariamente o registra; Button::shortcut documenta que apenas exibe o texto.
- O Calendar já limita datas; o gap é a propagação pelo DatePicker e a política comum de datas/horários indisponíveis.
- O NumberField já tem nullable, parse por cultura, limites e draft inválido. Não reimplementar esses recursos; ligar o draft ao estado de formulário e declarar os limites da conversão monetária por f64.
- Dialog já tem infraestrutura de foco modal no runtime; sem reprodução, não abrir bug de falta de focus trap.
- A planilha já desenha mesclas. Ausência de uma ação na UI não prova ausência do comando ou da leitura/escrita no motor.
- Outras implementações documentam combinações que não suportam. Não exigir que o RavenRS herde essas restrições; exigir um contrato de compatibilidade e testes das combinações escolhidas.
- Grande volume, cache, redução de pontos e agregação temporal têm semânticas diferentes. A mesma fonte bruta deve permitir inspecionar e exportar sem confundir ponto agregado com observação original.

## Matriz mínima de combinações para o aceite

| Família | Combinação obrigatória de cenário | Resultado verificável |
|---|---|---|
| Seleção | valor selecionado + filtro + troca/reordenação das opções | Seleção segue chave; rótulo não muda de registro. |
| Edição básica | texto de rascunho inválido + binding tipado antigo + submit | Não gravar o valor antigo como se a entrada atual fosse válida. |
| Estado | readonly/disabled + teclado + ponteiro + atualização externa | Nenhuma mutação por input bloqueado; valor externo continua visível. |
| Calendário | intervalo + bloqueio de dias + teclado + mudança de mês | A mesma regra vale para todo caminho de entrada. |
| Overlays | diálogo → seletor/menu → Escape/fechamento | Foco volta ao acionador correto e não escapa atrás do modal. |
| Remoto | consulta A → B + respostas B → A | A resposta antiga não substitui B; erro pertence à requisição certa. |
| Grade | virtualização + edição/lote + filtro/ordenação + chave estável | Reciclagem não transfere rascunho ou erro para outra célula. |
| Grade larga | colunas fixadas + janela horizontal + editor com popup | Foco, posição e popup permanecem associados à célula. |
| Documento | abrir → modificar → desfazer → salvar → reabrir | Dados e elementos suportados persistem; limitações explícitas. |
| Exportação | filtro/zoom + mudança posterior + tarefa assíncrona | O arquivo corresponde ao snapshot solicitado e declara o universo exportado. |
| Série temporal | lacuna + calendário/fuso + reduzir/agrupar + indicador | Nenhum zero inventado, erro de fronteira ou indicador com histórico truncado. |

## Caso de uso: um produto no nível do Primavera P6

O RavenRS oferece a base de telas administrativas. TreeGrid (#401), edição (#434/#435), Gantt (#406) e os novos contratos desta auditoria ainda são entregas planejadas. Mesmo concluídas, não entregam automaticamente um produto equivalente ao P6.

| Camada | Necessidades |
|---|---|
| Componentes reutilizáveis | Árvore/grade de atividades, Gantt, calendários, histogramas/curvas, comparação visual de baseline, edição em lote e impressão. |
| Motor de planejamento | CPM, datas cedo/tarde, folgas, restrições, calendários combinados, reprogramação e nivelamento. |
| Modelo de produto | WBS/EPS, progresso e baseline, atribuições/custos, permissões, auditoria, versões e portfólio. |
| Integrações | P6 XML/XER com matriz real de campos e diferenças de formato. |

A documentação Oracle distingue [cálculo CPM e calendário](https://docs.oracle.com/cd/F88968_01/English/User_Guides/p6_pro_user/scheduling_projects.htm), [nivelamento](https://docs.oracle.com/cd/G48902_01/English/User_Guides/p6_pro_user/level_resources.htm) e [intercâmbio](https://docs.oracle.com/en/industries/construction-engineering/primavera-p6-project/26/p6-help/aboutimportingandexportingdata-6604a.html). Estes são limites de arquitetura da comparação, não autorização para iniciar um produto Primavera nem promessa de compatibilidade.

O escopo de planejamento foi ampliado depois desta auditoria pela solicitação do usuário: [épico #457](https://github.com/alexkads/RavenRS/issues/457), com motores, componentes e aplicação de referência. Ver [roadmap de planejamento](PROJECT-PLANNING-ROADMAP.md) e [inventário](COMPONENT-INVENTORY.md). As entregas continuam planejadas.

## Rastreamento desta rodada

- [#439 — Checkbox: estado indeterminado público e semântica de seleção parcial](https://github.com/alexkads/RavenRS/issues/439).
- [#440 — RadioGroup, SegmentedControl e Slider: estados disabled e read_only consistentes](https://github.com/alexkads/RavenRS/issues/440).
- [#441 — Seletores de data e hora: limites, datas bloqueadas e limpeza de valor](https://github.com/alexkads/RavenRS/issues/441).
- [#442 — Combobox: texto editável, autocomplete e seleção com identidade estável](https://github.com/alexkads/RavenRS/issues/442).
- [#443 — LookupField e TagBox: páginas remotas, lista virtualizada e restauração por chave](https://github.com/alexkads/RavenRS/issues/443).
- [#444 — FormField: validação tipada e estado de rascunho inválido compartilhado](https://github.com/alexkads/RavenRS/issues/444).
- [#445 — MaskedField: posições opcionais, regras customizadas e indicação de preenchimento completo](https://github.com/alexkads/RavenRS/issues/445).
- [#446 — Tooltip: abertura por foco, Escape, atrasos e comportamento em toque](https://github.com/alexkads/RavenRS/issues/446).
- [#447 — FileUpload: sessão de transferência com progresso, cancelamento e envio em blocos](https://github.com/alexkads/RavenRS/issues/447).
- [#448 — ListBox: seleção múltipla, reordenação e transferência entre listas](https://github.com/alexkads/RavenRS/issues/448).
- [#449 — FloatingActionButton: ação flutuante ancorada com nome e estados de botão](https://github.com/alexkads/RavenRS/issues/449).
- [#450 — SpeedDial: grupo de ações flutuantes com foco, Escape e estados por item](https://github.com/alexkads/RavenRS/issues/450).
- [#451 — DataGrid: virtualização horizontal de colunas com preservação de foco e edição](https://github.com/alexkads/RavenRS/issues/451).
- [#452 — DataGrid: seleção retangular e clipboard de células com colagem validada](https://github.com/alexkads/RavenRS/issues/452).
- [#453 — DataGrid: inteiros e decimais exatos e valores de data-hora no modelo de célula](https://github.com/alexkads/RavenRS/issues/453).
- [#454 — DataGrid: exportar conjunto remoto completo com consulta estável e cancelamento](https://github.com/alexkads/RavenRS/issues/454).
- [#455 — DataGrid: cabeçalhos em bandas e fixação de colunas à direita](https://github.com/alexkads/RavenRS/issues/455).

As issues anteriores recebem cenários adicionais apenas onde há evidência específica. Escopos, decisões de design e critérios anteriores são preservados; não se encerra uma issue por existir uma referência ou uma API proposta.

Nesta rodada foram criadas **17 issues** (12 de controles básicos e 5 de grade) e atualizadas **18 issues existentes**: [#227](https://github.com/alexkads/RavenRS/issues/227), [#401](https://github.com/alexkads/RavenRS/issues/401), [#404](https://github.com/alexkads/RavenRS/issues/404), [#405](https://github.com/alexkads/RavenRS/issues/405), [#406](https://github.com/alexkads/RavenRS/issues/406), [#410](https://github.com/alexkads/RavenRS/issues/410), [#411](https://github.com/alexkads/RavenRS/issues/411), [#417](https://github.com/alexkads/RavenRS/issues/417), [#418](https://github.com/alexkads/RavenRS/issues/418), [#420](https://github.com/alexkads/RavenRS/issues/420), [#423](https://github.com/alexkads/RavenRS/issues/423), [#424](https://github.com/alexkads/RavenRS/issues/424), [#426](https://github.com/alexkads/RavenRS/issues/426), [#427](https://github.com/alexkads/RavenRS/issues/427), [#434](https://github.com/alexkads/RavenRS/issues/434), [#435](https://github.com/alexkads/RavenRS/issues/435), [#436](https://github.com/alexkads/RavenRS/issues/436), [#437](https://github.com/alexkads/RavenRS/issues/437). Os retornos das mutações confirmaram os textos salvos.
