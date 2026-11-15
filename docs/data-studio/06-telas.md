# Raven Data Studio: especificação das telas

Cada tela segue o mesmo roteiro: **objetivo, composição, componentes, ações, estados, acessibilidade, dependências e critérios de aceitação**. Os números vêm do [cenário de referência](cenario-de-referencia.md). Os componentes são os da [matriz](03-matriz-de-componentes.md), usados pela fachada `ravenrs::widgets`. Os códigos N-x, E-x e M-x remetem a [04-lacunas-e-novos-componentes.md](04-lacunas-e-novos-componentes.md).

## Regras que valem para todas as telas

- **Moldura.** `Sidebar` à esquerda (grupos Projetos, Conexões, Execuções e Modelos), `Breadcrumb` no alto, `Toolbar` com as ações da tela e `StatusBar` embaixo. Nesta barra ficam a versão do plano, o estado da simulação e a execução em segundo plano.
- **Área de trabalho do projeto.** A partir da tela 5 o projeto abre num `DockArea`. O centro mostra o documento ativo (`DocumentTabs`). À direita fica o inspetor e embaixo a lista de problemas (`IssueList`, N-3). A conversa fica num painel à direita, que pode ser fechado. O layout do dock é salvo em `ProjectViewState` com `DockLayout::to_text`.
- **Assistente guiado.** As etapas 4 a 13 também aparecem como um `StepIndicator` vertical no topo da `Sidebar` do projeto. O indicador mostra `Done`, `Current`, `Upcoming` e `Error`, e o motivo vai em `IndicatorStep::note`. Clicar numa etapa leva à tela correspondente. O assistente e as telas livres mostram o mesmo plano.
- **Estados.** Sempre com ícone e palavra, nunca só cor: `Badge` com `Tone` e `icon`.
- **Problemas.** Ficam na `IssueList` ou numa coluna da grade. `Toast` só confirma uma ação ("Versão 7 gravada") e nunca é o único lugar de um erro.
- **Sugestões da IA.** Aparecem como provisórias, com o selo "Sugestão" e o ícone de faísca. Cada uma tem Aceitar e Recusar. "Aceitar todas as de confiança alta" só aparece quando a política do projeto permite.
- **Janela estreita (< 900 px).** A `Sidebar` vira gaveta (`narrow_below`). Inspetor e conversa viram `Sheet` e o canvas cede o lugar à grade.
- **Teclado global.**
  - Cmd/Ctrl+K abre a `CommandPalette`.
  - Cmd/Ctrl+Z e Shift+Cmd/Ctrl+Z desfazem e refazem `PlanPatch`.
  - F6 percorre as regiões (navegação, documento, inspetor, problemas, conversa).
  - Cmd/Ctrl+Enter faz a ação principal da tela.

---

## 1. Projetos

**Objetivo.** Ver todos os projetos de importação, retomar um deles ou criar outro.

**Composição.** É uma lista administrativa gerada pelo `ravenrs-app-ui` (`ListView` sobre `DataGrid`) a partir da entidade `ImportProject`. As colunas são nome, estado, origem → destino, última execução, versão aprovada e dono. Uma busca fica acima da lista.

**Componentes.**
- `ListView` (ravenrs-app-ui) e `DataGrid`.
- `Badge` para o estado.
- `Toolbar`: Novo projeto, Duplicar, Arquivar.
- `EmptyState`.
- `Skeleton`.

**Ações.**

| Ação | Disponível quando | Resultado |
|---|---|---|
| Novo projeto | permissão `Create` em `ImportProject` | abre a tela 2 |
| Abrir (Enter ou duplo clique) | sempre | abre a última tela visitada do projeto |
| Duplicar | um selecionado | cria uma cópia em `Rascunho`, com o plano sem aprovação e sem simulação |
| Arquivar | estado ≠ `Executando` | pede confirmação (`ConfirmDialog::ask_plainly`) |

**Estados.**
- **Vazio:** `EmptyState` com o texto "Nenhum projeto ainda" e a ação Novo projeto.
- **Carregando:** linhas `Skeleton`.
- **Erro do banco do módulo:** `Alert` com a ação Tentar de novo.
- **Projeto `Executando`:** fica com `Spinner` na coluna de estado.

**Acessibilidade.** A grade tem teclado completo e o estado é lido como texto ("Concluído com pendências").

**Dependências.** Nenhuma nova.

**Critérios de aceitação.**
- Com o projeto do cenário concluído, a lista mostra "Concluído com pendências · 83 fora".
- Um usuário sem `Create` não vê Novo projeto.
- Arquivar um projeto em execução não fica disponível, e o motivo aparece na dica.

## 2. Novo projeto por intenção

**Objetivo.** Começar escrevendo o pedido em linguagem natural ou começar vazio.

**Composição.**
- **Centro:** um campo grande com o pedido, os exemplos abaixo e as escolhas rápidas de origem e destino (conexões já cadastradas ou "Nova conexão").
- **Ao enviar:** a conversa ocupa a área e mostra o plano inicial como um card de proposta (M-5).

**Componentes.**
- `Chat`: `.intro`, `.examples`, `.on_send`, `.busy`.
- `ChatMessage::assistant_writing` para o texto em fluxo.
- `ChatMessage::card` para a proposta.
- `AttachmentStrip` para arquivos CSV anexados.
- `DropZone`.
- `Select` para as conexões.
- `Button` para "Começar vazio".

**Ações.**

| Ação | Resultado |
|---|---|
| Enviar pedido | o agente propõe a versão 1. O card lista o que entendeu (conexões, entidades, casos detectados) e o que precisa decidir. |
| Anexar CSV (arrastar ou escolher) | cria conexões CSV provisórias. Acima do limite de memória da `DropZone`, pede o `OpenDialog` nativo. |
| Aceitar proposta | cria o projeto em `Descoberto` e vai para a tela 4 |
| Ajustar ("separe também o telefone") | gera uma nova proposta sobre a anterior |
| Começar vazio | cria o projeto em `Rascunho`, sem IA |
| Parar | interrompe a geração. Fica fora do `Chat`, num botão ao lado, até existir a E-7. |

**Estados.**
- **Modelo local ausente:** `Alert` com o texto "Nenhum modelo instalado. Baixe um em Configurações de IA ou comece vazio".
- **Geração em curso:** `Chat::busy`.
- **Resposta inválida do modelo:** "Não consegui montar o plano; tente reformular", com o motivo técnico em detalhes. Depois de 2 tentativas, oferece começar vazio.

**Acessibilidade.**
- O texto em fluxo é anunciado por região viva educada, só ao terminar cada frase.
- O card de proposta é um grupo com título e as ações chegam por Tab.

**Dependências.** M-5, M-8, E-7 (parar).

**Critérios de aceitação.**
- O pedido de referência gera uma proposta com 2 origens (CLIENTES e PEDIDOS) e 3 entidades de destino.
- A proposta traz as pendências "CPF duplicado (37 grupos)", "Situação sem correspondência" e "Endereço sem divisão segura".
- Nenhuma conexão é criada sem o usuário confirmar.
- Sem modelo instalado, o produto continua utilizável por "Começar vazio".

## 3. Conexões MCP e capacidades

**Objetivo.** Cadastrar e testar conexões (CSV, banco relacional e, na fase Avançado, servidores MCP externos) e ver o que cada uma sabe fazer.

**Composição.**
- **Esquerda:** lista de conexões.
- **Direita:** formulário (`FormLayout`) com os parâmetros, botão Testar e a tabela de capacidades.
- **Para MCP:** lista de ferramentas do servidor, com a permissão de cada uma.

**Componentes.**
- `ListView` / `DataGrid`.
- `FormLayout`, `FormGroup`, `FormField`.
- `TextField`, com `secure()` para a senha.
- `NumberField` para a porta.
- `Select` para tipo, codificação e separador.
- `Button`.
- `DataGrid` das capacidades, com as colunas capacidade, situação e observação.
- `Badge`.
- `Alert`.

**Ações.**

| Ação | Resultado |
|---|---|
| Testar | chama `Connector::check`. Mostra o tempo, a versão do servidor e as capacidades, e grava `last_check`. |
| Salvar | grava os parâmetros. O segredo vai para o cofre do sistema e o formulário guarda só a referência. |
| Liberar ferramenta MCP (Avançado) | muda a permissão da ferramenta: bloqueada, leitura ou escrita. Escrita pede `ConfirmDialog::ask_typing` com o nome da ferramenta. |

**Capacidades mostradas.**
- **Ler estrutura:** CSV ✅ · PostgreSQL ✅
- **Ler em páginas:** CSV ✅ (por posição) · PostgreSQL ✅ (por chave)
- **Escrever:** CSV — · PostgreSQL ✅
- **Criar tabelas:** CSV — · PostgreSQL ✅ (transacional)
- **Upsert idempotente:** CSV — · PostgreSQL ✅
- **Execução isolada:** CSV — · PostgreSQL 🟡 (depende de permissão para criar schema)

**Estados.**
- **Falha de conexão:** `Alert` `Danger` com a categoria (Conexão, Permissão, Tempo esgotado) e a mensagem do servidor.
- **Teste em curso:** `Spinner` no botão.
- **Segredo ausente no cofre:** "Senha não encontrada neste computador".

**Acessibilidade.**
- Cada linha de capacidade é lida como "Criar tabelas: sim, transacional".
- O resultado do teste é anunciado.

**Dependências.** N-5 (só no Avançado).

**Critérios de aceitação.**
- A senha nunca aparece no JSON do plano, no log nem no relatório. O teste confere o JSON exportado.
- Uma conexão CSV não oferece escrita e a tela de destino não a lista.
- Uma ferramenta MCP começa bloqueada.

## 4. Seleção de origem e destino

**Objetivo.** Escolher quais estruturas da origem entram e qual conexão recebe.

**Composição.**
- **Duas colunas:** `TreeView` da origem (conexão → schema → tabela → coluna) e `TreeView` do destino.
- **Nas tabelas:** as marcas (`checked`) indicam o que entra.
- **Nó preguiçoso:** expandir lê a estrutura sob demanda.
- **Rótulo:** a contagem de linhas vai junto do nome ("CLIENTES · 1.248").

**Componentes.**
- `TreeView`: `.lazy()`, `on_expand`, `.checked`, `.filter`, `.height` (virtualizada).
- `TextField` para a busca.
- `Badge` para "estimado" ou "exato".
- `Toolbar`.

**Ações.**
- Marcar ou desmarcar tabela.
- Atualizar estrutura (gera um novo snapshot).
- Trocar conexão.
- Continuar: grava o `SourceSnapshot` e vai para o Perfil.

**Estados.**
- **Carregando nó:** indicador no nó (E-6). Até existir, um `Spinner` na linha "Carregando…".
- **Estrutura mudou desde o último snapshot:** `Alert` `Warning` com a diferença ("PEDIDOS: coluna nova OBS") e a ação Usar a nova estrutura.

**Acessibilidade.**
- Árvore com setas e espaço para marcar.
- A contagem é lida junto do nome.

**Dependências.** E-6.

**Critérios de aceitação.**
- No cenário, marcar CLIENTES e PEDIDOS grava um snapshot com 2 estruturas e o fingerprint de cada uma.
- Uma árvore com 5.000 tabelas rola sem travar, porque é virtualizada e preguiçosa.

## 5. Perfil dos dados

**Objetivo.** Entender os dados antes de decidir: tipos, nulos, padrões, duplicidades e relacionamentos.

**Composição.**
- **Centro:** grade de colunas da estrutura selecionada, com as colunas campo, tipo declarado, tipo inferido, confiança, nulos, vazios, distintos, padrão e base.
- **Abaixo:** amostra das linhas (`DataGrid` remoto).
- **Inspetor:** histograma simples de valores frequentes da coluna e exemplos.
- **Problemas:** "CPF com pontuação em 806", "CEP sem zero inicial em 211", "37 CPFs repetidos", "Vínculo PEDIDOS.COD_CLI → CLIENTES.COD_CLI inferido (99,7%)".

**Componentes.**
- `DataGrid`: `.cell_style`, `GridColumn::template` para ícone e texto da confiança.
- `DataGrid::remote(RemoteRows)` para a amostra.
- `IssueList` (N-3).
- `Badge`.
- `Stat` para linhas e colunas.
- `PropertyRow` no inspetor.

**Ações.**
- Recalcular sobre o conjunto inteiro, quando a origem permite pushdown.
- Marcar coluna como sensível, para mascarar na conversa e no relatório.
- Escolher chave candidata para a análise de duplicidade.
- Aceitar vínculo inferido.

**Estados.**
- **Perfil em cálculo:** `ProgressBar` por estrutura.
- **Perfil só sobre amostra:** um selo em cada número ("amostra de 200").
- **Coluna 100% nula:** aviso.

**Acessibilidade.** A base de cada número é lida junto ("318 vazios, conjunto inteiro").

**Dependências.** N-3, M-7.

**Critérios de aceitação.**
- `FONE` mostra "318 vazios · conjunto inteiro".
- `CEP` mostra o tipo declarado `INTEGER`, o tipo inferido "CEP (texto de 8)" e a confiança Alta.
- O vínculo inferido mostra a evidência "5.298 de 5.312".
- A amostra nunca pede mais de uma página de 200 linhas por vez.

## 6. Proposta de schema

**Objetivo.** Decidir a estrutura do destino: usar existente, alterar, criar ou intermediária.

**Composição.**
- **Esquerda:** lista de entidades de destino, com a disposição em `Badge` (Usar, Alterar, Criar, Intermediária).
- **Centro:** grade de campos da entidade selecionada, com as colunas nome, tipo lógico, precisão, nulo, padrão, chave, legado, origem da proposta (IA ou usuário) e estado.
- **Inspetor:** propriedades do campo.

**Componentes.**
- `VirtualList` ou `DataGrid` para as entidades.
- `DataGrid` com `on_cell_edit` para o nome. O tipo é editado no inspetor até existir a E-3.
- `FormLayout`.
- `Select` para o tipo lógico.
- `NumberField` para precisão e escala.
- `Checkbox` para nulo (`inputs.rs:166`).
- `Badge`.
- `Toolbar`: Nova entidade, Novo campo, Comparar e DDL.

**Ações.**
- Aceitar ou recusar sugestão.
- Mudar tipo.
- Marcar como chave ou como legado.
- Criar entidade.
- Marcar como intermediária.

Toda mudança é um `PlanPatch` relevante, que invalida a simulação.

**Estados.**
- **Tipo incompatível com a origem** (ex.: `CHAR(11)` para CPF com pontuação sem a regra `KeepDigits`): problema "Valores de origem não cabem: 806".
- **Entidade sem chave:** problema bloqueante.

**Acessibilidade.** O estado de cada campo é lido, e as sugestões trazem "sugestão da IA, confiança alta".

**Dependências.** E-3 (opcional).

**Critérios de aceitação.**
- O cenário propõe `customers` como Alterar (+ `legacy_id`), `customer_addresses` e `orders` como Criar, e a lista de mescla como Intermediária.
- Mudar `orders.total` para `NUMERIC(10,2)` invalida a simulação. A `StatusBar` passa a mostrar "Simulação desatualizada".

## 7. Comparação e DDL

**Objetivo.** Revisar exatamente o que mudará no destino.

**Composição.**
- **Abas** (`Tabs`): Por entidade e Comandos.
- **Por entidade:** grade destino atual × proposto, com as colunas campo, atual, proposto e mudança, marcadas com `+`, `~` e `−` além da cor.
- **Comandos:** DDL em `CodeEditor` somente leitura. No MVP fica em `CodeLanguage::Plain` e, depois da E-2, em `Sql`.
- **Versões:** um `TextDiff` (N-2) compara o DDL da versão aprovada com o da atual.

**Componentes.**
- `Tabs`.
- `DataGrid`.
- `CodeEditor` com `.read_only` e `.line_numbers`. Exige a feature `code`.
- `CodeDiagnostic` para avisos como "ADD COLUMN NOT NULL sem padrão em tabela com 4.310 linhas".
- `TextDiff` (N-2).
- `Button` para copiar e exportar `.sql`.

**Ações.**
- Copiar DDL.
- Exportar `.sql`.
- Ir ao campo: um clique na linha do DDL leva à tela 6.
- Comparar com outra versão.

**Estados.**
- **Sem mudanças de schema:** `EmptyState` "Nenhuma alteração de estrutura".
- **Adaptador sem DDL:** "Este destino não cria tabelas; crie-as antes ou escolha outro destino".

**Acessibilidade.**
- O `CodeEditor` é lido linha a linha.
- Cada diagnóstico é alcançável por F8.
- A comparação tem alternativa tabular na aba Por entidade.

**Dependências.** E-2, N-2.

**Critérios de aceitação.**
- O DDL do cenário é igual ao de [11-schema-mapeamento-transformacoes.md](11-schema-mapeamento-transformacoes.md#ddl-proposto-do-cenário-texto-revisável).
- O DDL é gerado pelo adaptador (`render_ddl`), nunca pela IA.
- Nenhum comando é executado nesta tela.

## 8. Canvas

**Objetivo.** Ver e editar as relações origem → destino de forma visual. É uma vista do mesmo plano da grade.

**Composição.**
- **Disposição:** entidades de origem à esquerda e de destino à direita, como cartões com a lista de campos (`SchemaDiagram`, N-1). No meio ficam os nós de regra quando há transformação.
- **Ligações:** cada ligação é um `FieldMapping`.
- **Apoio:** minimapa e controles de zoom no canto.
- **Barra do canvas:** filtro (Todas, Só pendências, Só a entidade selecionada), Organizar e Buscar campo.

**Componentes.**
- `FlowCanvas` por baixo do `SchemaDiagram` (N-1): `FlowViewport`, `FlowSelection`, `FlowValidator`, `.minimap(true)`, `.controls(true)`, `.grid`.
- `Toolbar`.
- `SegmentedControl` para o filtro.
- `TextField` para a busca.

**Ações (ponteiro).**
- Arrastar de uma porta de campo de origem até uma de destino cria o mapeamento. A porta incompatível fica vermelha e mostra o motivo ("texto não cabe em DATE sem regra de data").
- Clicar numa ligação a seleciona e o inspetor mostra o `MappingInspector`.
- Shift-arrastar no fundo seleciona por retângulo.
- Delete remove o mapeamento selecionado.

**Ações (teclado).**
- Tab percorre os nós, e as setas percorrem os campos do nó (N-1).
- Enter num campo de origem inicia uma ligação. As setas escolhem o destino entre os compatíveis, Enter confirma e Esc cancela (portas pelo teclado do `FlowCanvas`, já no PR #389).
- `+`, `-` e `0` controlam o zoom.

**Estados.**
- **Muitas ligações** (mais de 300 visíveis): o filtro vai para Só a entidade selecionada e um aviso diz quantas ficaram escondidas.
- **Entidade recolhida:** as ligações se juntam no cabeçalho com a contagem.

**Acessibilidade.**
- A **grade (tela 9) é a alternativa completa** e qualquer operação do canvas existe nela.
- Cada nó é lido como "customers, destino, 6 campos, 1 pendência".
- Cada ligação é lida como "CLIENTES.CPF para customers.document, regra Só dígitos".

**Dependências.** E-1 (organizar sozinho, grupos, desempenho), N-1, M-2. O `FlowHistory` do PR #389 não é usado: o histórico é o do plano.

**Critérios de aceitação.**
- Selecionar `orders.status` na grade seleciona a mesma ligação no canvas e vice-versa.
- Mover nós não invalida a simulação.
- Ligar `PEDIDOS.DATA` direto a `orders.ordered_at` é aceito com a sugestão de regra `ParseDate("dd/MM/yyyy")`.
- O teste de interação cobre criar, recusar e apagar ligação pelo ponteiro e pelo teclado.

## 9. Grade

**Objetivo.** Mapear e revisar campo a campo, com todas as informações em colunas. É a vista principal e acessível do mapeamento.

**Composição.**
- **Colunas:** entidade de destino, campo de destino, tipo, origem, regras (em resumo), política, estado, confiança, problemas e exemplo antes → depois.
- **Agrupamento:** por entidade.
- **Inspetor:** `MappingInspector` (M-3).

**Componentes.**
- `DataGrid`: `.group_by`, `.selection_mode(Multiple)`, `.cell_style`, `GridColumn::template` para `Badge` com ícone, `.row_detail` para exemplos, `.search_panel`.
- `LookupField::remote` no inspetor para escolher a origem (E-8 mostra o tipo).
- `ExpressionEditor` para regra `Expression`.
- `IssueList`.

**Ações.**
- Escolher origem.
- Definir constante.
- Marcar como ignorado.
- Adicionar regra.
- Aceitar ou recusar sugestão (uma, as selecionadas, ou as de confiança alta).
- Filtrar por "só pendências".

**Estados.**
- **Campo obrigatório sem origem:** linha com `Badge` Danger "Sem origem · bloqueia".
- **Sugestão:** `Badge` Accent "Sugestão · alta".
- **Mapeado:** `Badge` Success "Mapeado".

**Acessibilidade.**
- Teclado completo da grade.
- O estado e a confiança são texto.
- O detalhe da linha (antes/depois) abre com a seta para a direita.

**Dependências.** E-3, E-8, M-1, M-3.

**Critérios de aceitação.**
- O cenário mostra 22 campos de destino nas 3 entidades: 6 em `customers`, 10 em `customer_addresses` e 6 em `orders`, cada um com estado.
- Não fica nenhum obrigatório sem origem depois de aceitar a proposta.
- Aceitar a sugestão de `orders.status` cria `Trim` → `Case(Upper)` → `MapValues` com 6 correspondências (cobrindo os 7 valores conhecidos, já que `Cancelado` vira `CANCELADO`) e 1 pendência (`X`).

## 10. Transformações

**Objetivo.** Ver, ordenar e testar as regras de cada campo, com o antes e o depois.

**Composição.**
- **Esquerda:** campos com regra.
- **Centro:** a pilha de regras do campo, em ordem e arrastável, com política de nulo e de erro.
- **Abaixo:** grade antes → depois sobre a amostra, com as contagens afetados, sem mudança, erro e nulo.

**Componentes.**
- `VirtualList`.
- `DataGrid` para a pilha, com reordenação por menu "Mover para cima/baixo".
- `DataGrid` para o antes/depois.
- `ExpressionEditor`.
- `Select` para a operação.
- `TextField` para os parâmetros.
- `Stat`.
- `IssueList`.

**Ações.**
- Adicionar, remover, reordenar, testar e salvar como modelo (Evolução).
- Na tabela de valores (`MapValues`): editar as correspondências em grade.

**Estados.**
- **Regra com erro em parte da amostra:** "2 de 200 não leem a data", com as linhas filtráveis.
- **Exemplo da IA que não bate:** a regra é recusada com o motivo.

**Acessibilidade.** Ordem da pilha anunciada ("regra 2 de 3, Só dígitos").

**Dependências.** N-3.

**Critérios de aceitação.**
- `CLIENTES.NOME` `MARIA  DA SILVA ` vira `Maria da Silva`.
- `CEP` `1310100` vira `01310100`.
- `VALOR` `1.234,56` vira `1234.56`.
- `ENDERECO` de 1890 vai para revisão com o motivo "sem número reconhecível".

## 11. Conflitos

**Objetivo.** Decidir o que fazer quando o registro já existe no destino, quando há duplicados na origem e quando falta referência.

**Composição.**
- **Um cartão por entidade** (`FormGroup`) com:
  - chave de casamento;
  - ação ao existir (Inserir, Atualizar, Ignorar, Upsert ou Revisão);
  - campos atualizáveis;
  - vazio na origem;
  - duplicado na origem, com a regra de sobrevivente;
  - referência ausente.
- **Ao lado:** prévia com as contagens desta política e exemplos.

**Componentes.**
- `FormLayout`.
- `Select`.
- `TagBox` (`tag_box.rs:37`) ou lista de `Checkbox` para os campos atualizáveis.
- `SegmentedControl`.
- `DataGrid` para os exemplos (ex.: 1042 e 2231 mesclados).
- `Stat`.

**Ações.**
- Mudar política, o que invalida a simulação.
- Ver grupos duplicados.
- Escolher sobrevivente manualmente num grupo. Isso vira decisão registrada.

**Estados.** Upsert num destino sem `idempotent_upsert`: aviso "Idempotência por conferência; mais lento".

**Acessibilidade.** Cada política é lida com o efeito ("Upsert por document: 142 atualizam, 1.060 inserem").

**Dependências.** Nenhuma nova.

**Critérios de aceitação.**
- O cenário mostra customers com 142 atualizações e 1.060 inserções, e 37 grupos mesclados.
- Os pedidos mostram 14 rejeitados por referência ausente e 31 retidos.
- O campo `name` não pode ser marcado como atualizável sem confirmação, porque a política do projeto o protege.

## 12. Simulação

**Objetivo.** Ver o resultado previsto antes de executar e saber exatamente o tipo de simulação feita.

**Composição.**
- **Cabeçalho:** modo (`Badge`: "Validação local", "Comandos gerados" ou "Execução isolada"), cobertura ("conjunto inteiro" ou "amostra de 200"), versão do plano e data.
- **Corpo:** grade por entidade com as colunas lidos, inserir, atualizar, mesclar, rejeitar, revisar, reter e total gravado, mais a conta de chegada.
- **Abaixo:** `IssueList` com erros e pendências e um link para as mudanças de schema (tela 7).

**Componentes.**
- `Badge`.
- `Stat`.
- `DataGrid` com rodapé de totais (`.footer`, `GridColumn::summary`).
- `IssueList`.
- `ProgressBar` durante a simulação.
- `SplitButton`: Simular, com Simular em amostra e Execução isolada no menu.

**Ações.**
- Simular de novo.
- Trocar o modo.
- Abrir problema, que leva à regra ou ao campo.
- Ir para a Revisão final.

**Estados.**
- **Desatualizada:** `Alert` Warning "O plano mudou depois desta simulação (versão 7 → 8)". Os números ficam acinzentados com o rótulo "desatualizado", e a ação é Simular de novo.
- **Em curso:** `ProgressBar` por entidade.
- **Falhou:** `Alert` Danger com a causa.

**Acessibilidade.**
- Conclusão anunciada com o resumo: "Simulação concluída: 8.031 gravações, 83 fora, 4 pendências".

**Dependências.** N-3.

**Critérios de aceitação.**
- A conta fecha:
  - customers: 1.248 lidos = 1.202 gravados + 37 mesclados + 9 em revisão;
  - orders: 5.312 = 5.261 + 14 + 31 + 6;
  - customer_addresses: 1.568.
- A simulação em amostra nunca mostra "conjunto inteiro".
- Mudar qualquer regra marca a simulação como desatualizada.

## 13. Revisão final

**Objetivo.** Aprovar conscientemente o que será feito.

**Composição.**
- **Resumo em linguagem simples:**
  - "Vai alterar 1 tabela e criar 2";
  - "Vai gravar 8.031 linhas";
  - "83 registros ficam fora (lista)";
  - "Não há desfazer automático: cada lote é atômico e a execução é retomável".
- **Pendências:** bloqueantes em vermelho e não bloqueantes em amarelo, cada uma com a decisão.
- **Avisos de ambiente:** destino gerido por um app RavenRS não deve ter outro processo criando registros ao mesmo tempo.
- **Caixa de confirmação** e botão Aprovar.

**Componentes.**
- `RichText`.
- `IssueList`.
- `Alert`.
- `PropertyRow`.
- `ConfirmDialog::ask_typing`: digitar o nome do destino quando há DDL.
- `Button`.

**Ações.**
- Resolver pendência, que leva à tela da pendência.
- Aprovar, que grava a `Approval` com usuário e data.
- Voltar.

**Estados.**
- **Pendência bloqueante aberta:** Aprovar desabilitado, com o motivo.
- **Simulação desatualizada:** Aprovar desabilitado com "Simule de novo".
- **Sem permissão de aprovar:** o botão não aparece. A mensagem diz quem pode.

**Acessibilidade.** O motivo de Aprovar desabilitado é a descrição do botão, lida pelo leitor.

**Dependências.** N-3.

**Critérios de aceitação.**
- No cenário, as decisões C1 (sobrevivente) e C10 (status `X` para revisão) precisam estar resolvidas antes de aprovar.
- C3 e C9 são não bloqueantes e vão para a fila de revisão.
- Aprovar grava um evento de auditoria.
- Mudar o plano depois de aprovar revoga a aprovação.

## 14. Execução

**Objetivo.** Acompanhar a carga.

**Composição.**
- **Linha do tempo das fases:** Preparando, Schema, Carga, Reconciliação e Relatório, em `StepIndicator` horizontal.
- **Por entidade:** `ProgressBar` com lidos, gravados, rejeitados, em revisão e taxa, além de lote atual / total estimado.
- **Abaixo:** `IssueList` ao vivo.
- **Botões:** Pausar (Evolução) e Cancelar.

**Componentes.**
- `StepIndicator`.
- `ProgressBar` com `.label` e `.detail`.
- `Badge`.
- `Stat`.
- `IssueList`.
- `StatusBar` com o progresso global.
- `ConfirmDialog` para cancelar.
- `Toast` só para "Execução concluída" quando a janela não está em foco.

**Ações.**
- Cancelar: pede confirmação com o texto "termina o lote atual; os lotes gravados ficam".
- Pausar.
- Abrir rejeitados.
- Fechar a tela: a execução continua e a `StatusBar` mostra o progresso.

**Estados.**
- **Em curso:** o padrão.
- **Pausado.**
- **Cancelando…**
- **Nova tentativa** de lote: "lote 3 de customers, tentativa 2 de 3".
- **Bloqueado:** a origem mudou.
- **Falhou:** ação Ir para Recuperação.

**Acessibilidade.**
- Progresso anunciado a cada 10% e em toda mudança de estado, por região viva educada. Falha usa região assertiva.
- Os números da tela não são relidos a cada atualização.

**Dependências.** M-6, N-3.

**Critérios de aceitação.**
- A interface recebe no máximo 4 eventos por segundo e nunca recebe registros.
- Cancelar no meio do lote 3 deixa exatamente os lotes 1 a 3 gravados, com o checkpoint coerente.
- O total estimado aparece como "≈" até a leitura terminar.

## 15. Recuperação

**Objetivo.** Entender o que aconteceu numa falha ou cancelamento e continuar com segurança.

**Composição.**
- **Diagnóstico:** o que falhou, em qual lote e com qual categoria de erro.
- **Estado dos lotes:** uma grade com entidade, lote, faixa da origem e estado (`Committed`, `Failed`, `Pending`).
- **Ação recomendada:** retomar do checkpoint, corrigir e simular de novo, ou abandonar.

**Componentes.**
- `Alert`.
- `DataGrid`.
- `PropertyRow`.
- `Badge`.
- `Button`.
- `ConfirmDialog`.

**Ações.**
- **Retomar** (Evolução; no MVP, nova execução idempotente do mesmo plano).
- **Exportar diagnóstico.**
- **Abrir o plano.**

**Estados.**
- **Origem mudou:** retomar fica bloqueado, com o texto "simule de novo".
- **Falha de permissão:** "O usuário do destino não pode criar índices".

**Acessibilidade.** O diagnóstico é o primeiro item focado ao abrir.

**Dependências.** Nenhuma nova.

**Critérios de aceitação.**
- Uma falha simulada de conexão no lote 4 de orders mostra os lotes 1 a 3 como `Committed` e o 4 como `Failed`.
- Retomar termina com as mesmas contagens de uma execução sem falha (5.261 pedidos) e sem duplicar nenhum.

## 16. Resultado

**Objetivo.** Fechar a importação: o que entrou, o que ficou fora e a prova disso.

**Composição.**
- **Cabeçalho:** `Stat` para gravados, atualizados, fora e duração.
- **Reconciliação:** uma grade com as conferências e o estado de cada uma ("CLIENTES: 1.248 = 1.202 + 37 + 9 ✓").
- **Abas:**
  - Rejeitados: 14.
  - Em revisão: 9 + 23 + 6.
  - Retidos: 31.
  - Relatório.

**Componentes.**
- `Stat`.
- `DataGrid::remote` para rejeitados e revisão.
- `Tabs`.
- `Badge`.
- `SplitButton` Exportar: Relatório PDF, Relatório XLSX, Rejeitados (CSV fiel), Grade atual.
- `EmptyState`.

**Ações.**
- **Exportar relatório:** usa o motor de relatórios.
- **Exportar rejeitados (CSV fiel):** o motor escreve o arquivo a partir do registro original.
- **Exportar a grade atual:** `ravenrs-grid-export`, com o que está na tela.
- **Resolver revisão:** decisão por registro (Evolução), seguida de uma execução complementar.
- **Duplicar como modelo.**

**Estados.**
- **Sem rejeitados:** `EmptyState` "Nada ficou de fora".
- **Divergência na reconciliação:** `Badge` Danger "Diverge" com o detalhe.

**Acessibilidade.** O resumo é o título da página e é lido ao abrir.

**Dependências.** N-3.

**Critérios de aceitação.**
- Os números são idênticos aos do cenário: 1.202, 1.568, 5.261 e 83.
- O CSV fiel dos rejeitados tem as mesmas colunas de PEDIDOS, mais `motivo` e `regra`, e a linha 88019 com `VALOR` `45,00`, sem reformatar.
- Exportar a grade gera um arquivo diferente, com as colunas visíveis.
