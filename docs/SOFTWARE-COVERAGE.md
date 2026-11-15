# Cobertura por tipo de software

Data: 15/11/2026. Checkout: `6b50ff874`. Avaliação exploratória de código, documentação e backlog; sem testes de runtime nesta rodada.

Este é um retrato da base auditada. A integração desta documentação com um master
mais recente não recalcula as notas nem classifica entregas posteriores como ausentes.
Exceção: **Diagramas e quadros visuais** foi reavaliado em `35932d841`, depois do layout
de árvore, das ligações que desviam dos nós, da rotação e das views dentro dos nós do
FlowCanvas (#456, #479, #481). As demais categorias seguem a base `6b50ff874`.

A [cobertura do ravenrs-app](APP-COVERAGE.md) complementa esta matriz com recursos
corporativos recorrentes, fluxos declarativos, integração restante e consumo pelo
MCP por modelos de IA com menos recursos.

## Como interpretar

Este indicador mede **cobertura de dez capacidades escolhidas por cenário**, com pesos iguais. Não mede percentual de um produto comercial desenvolvido, esforço restante, equivalência comercial ou prontidão de produção. As categorias têm recortes diferentes; os índices ajudam a localizar gaps e não formam ranking de qualidade.

- **1:** API/modelo documentado no checkout para a capacidade delimitada. Não significa fluxo inteiro validado.
- **0,5:** suporte parcial ou ferramentas genéricas de composição; falta especialização/integração.
- **0:** capacidade especializada não comprovada no recorte, ou apenas prevista. Não é prova de impossibilidade nem auditoria exaustiva de ausência.

Índice = soma dos pontos / 10 × 100. As notas são julgamento técnico inicial; reservar aproximadamente ±10 pontos percentuais como faixa de incerteza qualitativa, sem interpretação estatística. Refinar cada item com documentação de consumo e aplicação de referência antes de usar como meta.

Uma issue não soma pontos de implementação. Motores, serviços da aplicação e componentes visuais são separados abaixo. Projetos com recursos novos fora deste checkout precisam ser reavaliados; o backlog remoto já contém descrição de entregas posteriores, por exemplo a atualização de #458 sobre o PR #433.

## Visão geral

| Categoria | Referência de cenário | Índice na base auditada | Componentes e experiências a completar |
|---|---|---:|---|
| Diagramas e quadros visuais | Whimsical / FigJam | 70% | Formas/toolbox, inspector de diagrama, wireframes, edição de mapa mental, pontos de dobra, exportação e comentários ancorados. |
| Design visual de interfaces | Figma Design | 30% | Editor vetorial, camadas de documento, texto no canvas, inspector, máscaras e editor de layout. |
| Construtor visual de sites | Wix | 35% | Paleta de seções, árvore de página, inspector, edição inline e simulador/editor de breakpoints. |
| Automação visual e chatbots | ManyChat | 35% | Blocos ricos, inspector de condições/mensagens, simulador e monitor de execuções. |
| Planejamento avançado de projetos | Primavera / Microsoft Project | 20% | Gantt, TreeGrid, WorkCalendarEditor, TimePhasedGrid e ActivityNetwork. |
| BI e dashboards analíticos | Power BI / Tableau, como referência de categoria | 45% | PivotGrid, editor de dashboard, painéis rearranjáveis e Chart ampliado. |
| Planilha de uso geral | Excel, em um recorte de edição comum | 70% | Completar comandos de Spreadsheet, edição de gráficos e experiência de pivot. |
| Editor paginado de documentos | Word, em um recorte de edição comum | 55% | DocumentEditor paginado, seleção entre blocos, ferramentas de tabela/imagem e cabeçalho/rodapé. |
| IDE de desenvolvimento | VS Code | 35% | Editor de código avançado, completion, diff, terminal e painéis de depuração. |
| Editor de imagens em camadas | Photoshop / Affinity Photo, como referência de categoria | 65% | ImageEditor integrado, painel de camadas, máscaras, pincéis e seleção de pixels. |
| Edição audiovisual geral | DaVinci Resolve / Premiere, como referência de categoria | 40% | Timeline audiovisual integrada, waveform, mixer e monitores de cor. |
| GIS e edição geográfica | QGIS / ArcGIS | 15% | Map, gerenciador de camadas, editor de feições, legenda e ferramentas cartográficas. |
| CAD 2D técnico | AutoCAD, recorte 2D | 25% | Viewport técnico, ferramentas geométricas, cotas, camadas e inspector especializado. |
| Modelagem e animação 3D | Blender | 15% | Viewport3D, gizmos, editor de malhas/UV, materiais e animação. |
| Produção musical / DAW | Ableton / Reaper | 20% | Waveform, piano roll, mixer, medidores e timeline musical. |

## Diagramas e quadros visuais: 70%

Referência de cenário: Whimsical / FigJam. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Pan, zoom e minimapa | 1 |
| Nós, portas e conexões | 1 |
| Seleção múltipla e grupos | 1 |
| Histórico e clipboard de diagrama | 1 |
| Documento de grafo JSON | 1 |
| Layouts especializados e roteamento | 1 |
| Formas e propriedades de diagrama | 0,5 |
| Biblioteca de wireframes | 0 |
| Edição própria de mapas mentais | 0,5 |
| Colaboração simultânea no documento | 0 |

**Componentes/experiências:** Formas/toolbox, inspector de diagrama, wireframes, edição de mapa mental, pontos de dobra, exportação e comentários ancorados.

**Infraestrutura e domínio:** Pontos de dobra editáveis, exportação do diagrama e colaboração de documento. O layout de árvore (organograma de cima para baixo ou mapa mental na horizontal) e o roteamento em degrau que desvia dos nós já existem; falta a edição própria do mapa mental, como criar o filho pelo teclado.

**Backlog relacionado:** #396, #397 (pontos de dobra e exportação) e #402; #195, #238 e #395 descrevem a base existente.

**Evidência local/documental:** crates/ravenrs-widgets-charts/src/flow.rs; crates/ravenrs-graphics/src/flow.rs.


## Design visual de interfaces: 30%

Referência de cenário: Figma Design. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Navegação de superfície 2D | 1 |
| Transformações de objetos compostos | 0,5 |
| Árvore de camadas de documento | 0,5 |
| Editor de caminhos e alças Bézier | 0 |
| Operações booleanas e máscaras editáveis | 0 |
| Componentes visuais com instâncias e overrides | 0 |
| Edição visual de layout e restrições | 0,5 |
| Texto rico editável sobre o canvas | 0 |
| Documento visual persistente integrado | 0,5 |
| Edição simultânea do documento | 0 |

**Componentes/experiências:** Editor vetorial, camadas de documento, texto no canvas, inspector, máscaras e editor de layout.

**Infraestrutura e domínio:** Modelo de documento vetorial, booleanas, instâncias/overrides e colaboração.

**Backlog relacionado:** #402 e #437 contribuem; não especificam um editor equivalente ao cenário inteiro.

**Evidência local/documental:** crates/ravenrs-widgets-imaging/README.md; crates/ravenrs-widgets-base/src/canvas.rs; #437.


## Construtor visual de sites: 35%

Referência de cenário: Wix. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Paleta de blocos de página | 0,5 |
| Inserção e reposicionamento visual de blocos | 0,5 |
| Árvore estrutural de página | 0,5 |
| Inspector tipado de propriedades | 0,5 |
| Layout editável responsivo | 0,5 |
| Overrides editáveis por breakpoint | 0 |
| Edição de conteúdo no contexto da página | 0,5 |
| Documento de site e versões integradas | 0 |
| Pipeline de geração e publicação do site | 0,5 |
| Saída HTML/CSS semântica do documento visual | 0 |

**Componentes/experiências:** Paleta de seções, árvore de página, inspector, edição inline e simulador/editor de breakpoints.

**Infraestrutura e domínio:** Documento de site, estilos por breakpoint, componentes reutilizáveis e geração de saída web.

**Backlog relacionado:** #437, #402, #245 e #264 ajudam em partes; #437 limita-se a navegação/lista/formulário sobre entidades.

**Evidência local/documental:** crates/ravenrs-project/src/lib.rs; #437. O gerador atual produz um app RavenRS com canvas, não prova um exportador de sites HTML/CSS..


## Automação visual e chatbots: 35%

Referência de cenário: ManyChat. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Editor de grafo | 1 |
| Representação de condições e ramificações | 0,5 |
| Configuração de blocos | 0,5 |
| Prévia conversacional integrada ao fluxo | 0,5 |
| Cadastros de contatos e campos | 0,5 |
| Gatilhos interpretados pelo fluxo | 0 |
| Execução durável com esperas e retomada | 0 |
| Conectores de canais ligados ao motor | 0 |
| Inspeção e depuração de execuções do fluxo | 0 |
| Versão publicável e testável da automação | 0,5 |

**Componentes/experiências:** Blocos ricos, inspector de condições/mensagens, simulador e monitor de execuções.

**Infraestrutura e domínio:** Motor de workflow, estado durável, filas, retomada, idempotência e conectores da aplicação.

**Backlog relacionado:** #395–#397 e #402 ajudam no editor; #82 prevê contratos de extensão. Isso não comprova motor de automação entregue.

**Evidência local/documental:** crates/ravenrs-widgets-charts/src/flow.rs; #82.


## Planejamento avançado de projetos: 20%

Referência de cenário: Primavera / Microsoft Project. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Estrutura de tarefas/WBS editável | 0,5 |
| Gantt com edição de dependências | 0 |
| Calendários úteis e jornadas | 0 |
| CPM e explicação de folgas | 0 |
| Atribuições e nivelamento de recursos | 0 |
| Baseline e cenários comparáveis | 0 |
| Grade temporal de uso/capacidade | 0 |
| Custos e valor agregado integrados | 0,5 |
| Intercâmbio de planejamento | 0 |
| Grade tabular de dados | 1 |

**Componentes/experiências:** Gantt, TreeGrid, WorkCalendarEditor, TimePhasedGrid e ActivityNetwork.

**Infraestrutura e domínio:** Modelo de planejamento, calendário útil, CPM, esforço, nivelamento, baseline, custos/EVM e importadores.

**Backlog relacionado:** #457 coordena #459–#478; também #401 e #406.

**Evidência local/documental:** docs/PROJECT-PLANNING-ROADMAP.md; docs/COMPONENT-AUDIT.md.


## BI e dashboards analíticos: 45%

Referência de cenário: Power BI / Tableau, como referência de categoria. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Gráficos básicos | 1 |
| Filtros e seleção de dados | 1 |
| Composição de dashboard | 0,5 |
| Designer visual persistente de dashboard | 0 |
| PivotGrid interativo | 0 |
| Drilldown de Chart | 0 |
| Eixos analíticos avançados | 0,5 |
| Séries extensas e dados remotos analíticos | 0,5 |
| Exportação analítica integrada | 0,5 |
| Crossfilter entre visualizações | 0,5 |

**Componentes/experiências:** PivotGrid, editor de dashboard, painéis rearranjáveis e Chart ampliado.

**Infraestrutura e domínio:** Modelo semântico e conectores conforme a aplicação, consultas/agregações e propagação de filtros.

**Backlog relacionado:** #203, #403, #410, #420–#427 e #432.

**Evidência local/documental:** docs/COMPONENT-AUDIT.md; docs/COMPONENT-INVENTORY.md.


## Planilha de uso geral: 70%

Referência de cenário: Excel, em um recorte de edição comum. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Edição de células | 1 |
| Fórmulas e recálculo | 1 |
| Estilos e formatos | 1 |
| Múltiplas folhas | 1 |
| Undo e clipboard | 1 |
| Comandos visuais de congelamento | 0,5 |
| Inserção/edição visual de gráficos | 0,5 |
| Comandos de formatação condicional | 0,5 |
| Preenchimento e busca completos na UI | 0,5 |
| Pivot interativo na experiência visual | 0 |

**Componentes/experiências:** Completar comandos de Spreadsheet, edição de gráficos e experiência de pivot.

**Infraestrutura e domínio:** Integração de UI com o motor existente; formatos avançados e compatibilidade exigem matriz própria.

**Backlog relacionado:** #411 e #403. Capacidade do motor ravenrs-sheet não implica comando disponível na UI.

**Evidência local/documental:** crates/ravenrs-spreadsheet/README.md; docs/COMPONENT-AUDIT.md.


## Editor paginado de documentos: 55%

Referência de cenário: Word, em um recorte de edição comum. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Modelo de documento | 1 |
| Edição textual reutilizável | 0,5 |
| Edição paginada WYSIWYG integrada | 0 |
| Estilos pela UI do documento | 0,5 |
| Edição contextual de tabelas | 0,5 |
| Edição contextual de imagens | 0,5 |
| Seções e quebras editáveis visualmente | 0,5 |
| Cabeçalhos/rodapés editáveis visualmente | 0,5 |
| Base de leitura e escrita de documentos | 1 |
| Impressão integrada ao editor | 0,5 |

**Componentes/experiências:** DocumentEditor paginado, seleção entre blocos, ferramentas de tabela/imagem e cabeçalho/rodapé.

**Infraestrutura e domínio:** Sessão de edição sobre ravenrs-word, layout e histórico coerentes, compatibilidade por formato.

**Backlog relacionado:** #405; #404 cobre visualização de PDF externo, outra experiência.

**Evidência local/documental:** crates/ravenrs-word/src/lib.rs; docs/COMPONENT-AUDIT.md.


## IDE de desenvolvimento: 35%

Referência de cenário: VS Code. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| CodeEditor básico | 1 |
| Explorador integrado de projeto | 0,5 |
| Workspace de documentos e painéis | 0,5 |
| Exibição de diagnósticos | 1 |
| Edição com suporte a várias linguagens | 0 |
| Completion/hover por LSP | 0 |
| Refatoração semântica | 0 |
| Terminal integrado | 0 |
| Depuração por DAP | 0 |
| Ciclo projeto/executar/exportar | 0,5 |

**Componentes/experiências:** Editor de código avançado, completion, diff, terminal e painéis de depuração.

**Infraestrutura e domínio:** LSP, DAP, processos/PTY por host e modelo de workspace.

**Backlog relacionado:** #399, #418, #153 e #82 contribuem; não fecham uma IDE completa.

**Evidência local/documental:** crates/ravenrs-widgets-chat/src/code_editor.rs; crates/ravenrs-project/src/lib.rs.


## Editor de imagens em camadas: 65%

Referência de cenário: Photoshop / Affinity Photo, como referência de categoria. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Pan/zoom de imagem | 1 |
| Recorte | 1 |
| Alças de transformação | 1 |
| Seletores de cor | 1 |
| Histograma e curvas | 1 |
| Comparação de imagens | 1 |
| Camadas editáveis de imagem | 0 |
| Máscaras de edição | 0 |
| Pincéis e seleção de pixels | 0 |
| Sessão integrada de edição/exportação | 0,5 |

**Componentes/experiências:** ImageEditor integrado, painel de camadas, máscaras, pincéis e seleção de pixels.

**Infraestrutura e domínio:** Documento raster em camadas, composição/filtros, histórico e gerenciamento de cor conforme escopo.

**Backlog relacionado:** #412 integra ferramentas existentes; não pressupõe todas as capacidades de um editor profissional.

**Evidência local/documental:** crates/ravenrs-widgets-imaging/README.md; docs/COMPONENT-INVENTORY.md.


## Edição audiovisual geral: 40%

Referência de cenário: DaVinci Resolve / Premiere, como referência de categoria. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Timeline reutilizável | 1 |
| Corte/trim em mídias arbitrárias | 0,5 |
| Overlays e legendas de filmes RavenRS | 1 |
| Animação por parâmetros/keyframes gerais | 0,5 |
| Ingestão integrada de mídias arbitrárias | 0 |
| Prévia sincronizada de áudio/vídeo geral | 0 |
| Mixagem multitrilha | 0 |
| Pipeline extensível de efeitos audiovisuais | 0,5 |
| Render/export de projeto audiovisual geral | 0,5 |
| Prévia com som no MovieEditor | 0 |

**Componentes/experiências:** Timeline audiovisual integrada, waveform, mixer e monitores de cor.

**Infraestrutura e domínio:** Pipeline de mídia geral, decodificação/seek/cache, sincronização A/V, DSP e render.

**Backlog relacionado:** #265 ajuda na reprodução. MovieEditor é base especializada em filmes de apps, não prova um editor audiovisual geral.

**Evidência local/documental:** crates/ravenrs-movie/README.md; crates/ravenrs-widgets-scheduler/src/timeline.rs.


## GIS e edição geográfica: 15%

Referência de cenário: QGIS / ArcGIS. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Controles de navegação de mapa | 0,5 |
| Componente Map integrado | 0 |
| Projeções no fluxo geográfico | 0 |
| Visualização vetorial geográfica | 0 |
| Camadas raster georreferenciadas | 0 |
| Edição de feições | 0 |
| Consultas espaciais | 0,5 |
| Validação topológica | 0 |
| Intercâmbio geográfico integrado | 0 |
| Composição cartográfica para impressão | 0,5 |

**Componentes/experiências:** Map, gerenciador de camadas, editor de feições, legenda e ferramentas cartográficas.

**Infraestrutura e domínio:** CRS, consultas/índices espaciais, topologia e raster georreferenciado.

**Backlog relacionado:** #170 prevê tiles e também mapas vetoriais, GeoJSON/TopoJSON e projeções; isso não equivale a GIS completo.

**Evidência local/documental:** docs/COMPONENT-INVENTORY.md; #170.


## CAD 2D técnico: 25%

Referência de cenário: AutoCAD, recorte 2D. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Viewport 2D navegável | 0,5 |
| Seleção geométrica precisa | 0,5 |
| Réguas/medição técnica | 0,5 |
| Entidades geométricas editáveis de CAD | 0 |
| Restrições geométricas | 0 |
| Cotas associativas | 0,5 |
| Camadas técnicas de documento | 0,5 |
| Blocos e referências de CAD | 0 |
| Intercâmbio de formatos CAD | 0 |
| Snapping geométrico preciso | 0 |

**Componentes/experiências:** Viewport técnico, ferramentas geométricas, cotas, camadas e inspector especializado.

**Infraestrutura e domínio:** Kernel geométrico, restrições, precisão/unidades, snapping e adaptadores de formatos.

**Backlog relacionado:** As ferramentas de imagem e #402 contribuem; não comprovam kernel de CAD.

**Evidência local/documental:** crates/ravenrs-widgets-imaging/README.md; crates/ravenrs-widgets-base/src/canvas.rs.


## Modelagem e animação 3D: 15%

Referência de cenário: Blender. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Viewport de cena 3D interativo | 0 |
| Hierarquia de objetos 3D | 0,5 |
| Gizmos de transformação 3D | 0,5 |
| Edição de malha | 0 |
| Materiais e UV | 0 |
| Iluminação/câmeras 3D | 0 |
| Timeline de animação 3D | 0,5 |
| Rigging | 0 |
| Render de cena 3D | 0 |
| Intercâmbio de cena 3D | 0 |

**Componentes/experiências:** Viewport3D, gizmos, editor de malhas/UV, materiais e animação.

**Infraestrutura e domínio:** Cena 3D, picking, geometria, render e animação. Renderer 2D em GPU não comprova cena 3D.

**Backlog relacionado:** Componentes genéricos contribuem; não foi comprovado conjunto especializado no recorte consultado.

**Evidência local/documental:** crates/ravenrs-widgets-imaging/README.md; crates/ravenrs-widgets-scheduler/src/timeline.rs.


## Produção musical / DAW: 20%

Referência de cenário: Ableton / Reaper. A referência delimita um uso, sem prometer reproduzir todo o produto.

| Capacidade avaliada | Pontos |
|---|---:|
| Timeline de áudio especializada | 0,5 |
| Waveform editável | 0 |
| Piano roll | 0 |
| Entrada/saída MIDI | 0 |
| Mixer com roteamento | 0 |
| DSP/plugins de áudio | 0 |
| Gravação de áudio geral | 0 |
| Transporte de áudio profissional | 0,5 |
| Curvas de automação de áudio | 0,5 |
| Documento de sessão musical | 0,5 |

**Componentes/experiências:** Waveform, piano roll, mixer, medidores e timeline musical.

**Infraestrutura e domínio:** Motor de áudio com baixa latência, MIDI, DSP, sincronização e plugins.

**Backlog relacionado:** #265 cobre parte da reprodução; não estabelece uma DAW.

**Evidência local/documental:** crates/ravenrs-movie/README.md; crates/ravenrs-widgets-scheduler/src/timeline.rs.


## Lacunas com reuso entre categorias

1. Documento editável com ids estáveis, hierarquia, referências, versões e migrações. Os documentos atuais de FlowCanvas, planilha, Word, relatório e filme têm contratos próprios: reusar conceitos e adaptadores sem exigir um formato universal.
2. Comandos/transações e integração de histórico. Já existem históricos especializados; falta comprovar sua extensão e composição em um editor consumidor.
3. Inspector/PropertyGrid, registro de editores e ferramentas contextuais.
4. Estrutura de documento visual, seleção, snapping, alinhamento e transformações. Seleção de pixels, seleção de feições e seleção de nós possuem semânticas diferentes.
5. Layout visual, estilos compartilhados, componentes/instâncias e overrides responsivos.
6. Colaboração de documento, presença e resolução de conflitos. Concorrência de registros #241 não equivale automaticamente a colaboração de canvas.
7. Contratos de extensão para ferramentas, tipos de bloco, conectores e exportadores, relacionados à #82.
8. Aplicações de referência que consumam APIs públicas e provem salvar/reabrir, editar, desfazer/refazer e produzir uma saída utilizável.

## Ordem de investigação sugerida

Após concluir os componentes atualmente em desenvolvimento, investigar o núcleo de edição visual pelos cenários diagramas → design de interfaces → sites, e a especialização de workflows em paralelo conceitual ao editor de grafos. Isso é uma prioridade proposta, não autorização para criar ou implementar novas issues nesta análise.

Os motores de planejamento já têm backlog detalhado. CAD, GIS avançado, áudio e 3D requerem decisões explícitas de domínio e integração; não presumir que todos esses motores devem pertencer ao núcleo RavenRS.

## Limites e referências

A avaliação não é uma lista exaustiva dos tipos de software. Aplicações administrativas, CRM, ERP, PDV e portais podem aproveitar a base existente; seu grau de cobertura depende do recorte e não foi pontuado aqui. Nenhuma família da matriz representa impossibilidade de desenvolvimento: representa trabalho ainda necessário para obter a experiência com os recursos comprovados do RavenRS.

Fontes do RavenRS: [auditoria](COMPONENT-AUDIT.md), [inventário](COMPONENT-INVENTORY.md), [roadmap de planejamento](PROJECT-PLANNING-ROADMAP.md), READMEs e código indicados em cada cenário. A busca remota de issues foi direcionada; não se declara inexistência de issue apenas porque não apareceu nos resultados.

Referências oficiais consultadas para os cenários recentes:

- [Whimsical: diagramas, mapas mentais e wireframes](https://whimsical.com/).
- [Figma: auto layout](https://help.figma.com/hc/en-us/articles/5731482952599-Toggle-on-auto-layout-in-designs).
- [Wix Studio: breakpoints](https://support.wix.com/en/article/studio-editor-managing-breakpoints).
- [ManyChat: construir automações](https://help.manychat.com/en/articles/17741989-how-to-build-a-manychat-automation).
- [ManyChat: condições](https://help.manychat.com/en/articles/17741964-condition-block).

Não foram alteradas issues nem implementados componentes nesta rodada.
