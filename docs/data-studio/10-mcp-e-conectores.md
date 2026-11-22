# Raven Data Studio: MCP, agente e conectores

Seis peças diferentes costumam ser chamadas de "MCP" ou "IA". Este documento separa cada uma, diz o que existe hoje e define os contratos.

| # | Peça | Existe hoje? | Quem usa | Pode escrever dados? |
|---|---|---|---|---|
| 1 | MCP de apoio ao desenvolvimento (`ravenrs-mcp`) | **Sim** | Assistentes de código de quem desenvolve com RavenRS | Não. Sem efeitos colaterais. |
| 2 | Cliente MCP da aplicação | **Não** | O Data Studio, para falar com servidores externos | Só pelo que o servidor oferecer e o usuário permitir |
| 3 | Servidores MCP externos | Fora do repositório | Expõem sistemas de terceiros | Depende do servidor |
| 4 | Agente de planejamento | **Não** (`ravenrs-ai` é a base) | O Data Studio | **Não.** Só produz `PlanPatch`. |
| 5 | Adaptadores de execução | **Não** | Motor e simulador | Sim, o destino, com plano aprovado |
| 6 | Motor determinístico de importação | **Não** | Interface e servidor | Sim, pelos adaptadores |

## 1. MCP de apoio ao desenvolvimento

`crates/ravenrs-mcp`: servidor MCP sobre stdio (`rmcp` com as features `server`, `macros`, `transport-io`). O README diz que é uma ferramenta de desenvolvimento, sem efeitos colaterais, sem rede e sem arquivos; tudo é embutido na compilação.

Ferramentas (`crates/ravenrs-mcp/src/server.rs:144-354`): `list_components`, `component_api`, `component_examples`, `design_tokens`, `parity_status`, `search_api`, `guides`, `app_contracts`, `validate_expression` (nunca avalia), `expression_functions`, `report_layout_format`, `create_report_layout`, `check_report_layout`, `report_layout_templates`.

**Papel no Data Studio:** nenhum em produção. Serve a quem for implementar o módulo, para consultar APIs e validar expressões durante o desenvolvimento. O servidor não estava instalado no ambiente desta análise e não foi instalado; as APIs foram conferidas lendo o código.

`docs/APP-PARITY.md:111` registra como pendente "um servidor MCP da aplicação" (issue #266), que é outra coisa: um servidor que exporia os dados do app com as permissões do usuário. Também não é cliente.

Não há cliente MCP no repositório. O outro servidor encontrado (`crates/ravenrs-deck/src/mcp/server.rs`, JSON-RPC escrito à mão, para apresentações) também é servidor.

## 2. Cliente MCP da aplicação (Avançado)

Componente novo, num crate próprio (`ravenrs-mcp-client`, nome provisório), genérico e reaproveitável por outros módulos. O `rmcp` já está no workspace; habilitar a feature `client` é a base técnica.

Responsabilidades:

- Conectar a servidores configurados pelo usuário (stdio local; HTTP no servidor), com segredo no cofre.
- Listar ferramentas e recursos; mostrar ao usuário o que cada ferramenta declara.
- **Permissão por ferramenta**: cada ferramenta do servidor externo começa bloqueada; o usuário libera, por projeto, as de leitura e, separadamente, as de escrita.
- Registrar em auditoria cada chamada, com ferramenta, argumentos (sem segredos) e tamanho da resposta.
- Tratar todo retorno como **dado**: o texto de resposta nunca é concatenado às instruções do agente sem marcação de origem e nunca dispara outra chamada sozinho.

O cliente MCP **não** é o caminho padrão para CSV e bancos relacionais: esses têm adaptadores nativos. MCP entra para sistemas heterogêneos que já ofereçam um servidor.

## 3. Servidores externos

Vistos pelo Data Studio através de um adaptador (`McpConnector`) que traduz o contrato de conector abaixo em chamadas de ferramentas do servidor. Um servidor externo que não oferece descoberta de estrutura ou leitura paginada simplesmente declara menos capacidades, e a interface oferece menos.

Tela 3 ("Conexões MCP e capacidades", [06-telas.md](06-telas.md#3-conexões-mcp-e-capacidades)) mostra cada servidor, suas ferramentas, a permissão de cada uma e a capacidade de conector derivada.

## 4. Agente de planejamento

### Contrato

```rust
trait Planner {
    /// Propõe alterações ao plano para um pedido, sobre um contexto montado pelo domínio.
    fn propose(&self, request: PlanRequest, context: PlanContext) -> Result<PlanProposal, PlannerError>;
    /// Explica um item do plano em linguagem natural, citando ids.
    fn explain(&self, item: PlanItemRef, context: PlanContext) -> Result<Explanation, PlannerError>;
}

struct PlanRequest { text: String, focus: Option<PlanItemRef>, stage: Stage } // Stage: Schema | Fields | Rules | Conflicts | Free
struct PlanContext {
    plan_summary: PlanSummary,          // entidades, campos, mapeamentos, pendências
    structures: Vec<StructureSummary>,  // da origem e do destino
    profiles: Vec<ProfileSummary>,      // agregados
    sample: Option<MaskedSample>,       // só se o projeto permitir; colunas sensíveis mascaradas
    allowed_ops: Vec<PlanOpKind>,       // o que este estágio pode propor
}
struct PlanProposal { patch: PlanPatch, confidence: Vec<(PlanOpId, f32)>, questions: Vec<PendingDecision> }
```

### Regras

- O agente **só** produz `PlanProposal`. Não tem ferramenta de escrita, não chama adaptadores, não chama o motor.
- O domínio valida o patch: tipos, existência de campos, exemplos antes/depois na amostra, compatibilidade com capacidades. O que não valida é recusado com motivo, visível na conversa.
- Sugestão com confiança abaixo do limite do projeto (padrão 0,8) vira `PendingDecision`, nunca é aplicada como aceita.
- O usuário aceita ou recusa cada operação do patch (ou todas de uma vez quando a política do projeto permitir para operações de baixo risco, ex.: mapeamento 1:1 por nome exato).
- **Conteúdo externo sem autoridade:** nomes de coluna, comentários de tabela, valores e respostas de servidores MCP entram no contexto dentro de um bloco marcado como dado. Um valor de célula com texto de instrução é tratado como texto. O agente não tem como agir sobre ele, porque não tem ferramenta de ação.

### Política do que vai ao modelo

| Nível | Conteúdo | Padrão |
|---|---|---|
| Estrutura | Nomes de tabelas e colunas, tipos, chaves | Sempre (local) |
| Perfil | Contagens, nulos, distintos, padrões detectados, mín./máx. de colunas não sensíveis | Sempre (local) |
| Amostra mascarada | Até 20 linhas, colunas sensíveis mascaradas (CPF, telefone, e-mail, nome) | Desligado; liga por projeto |
| Amostra crua | Linhas sem máscara | Só modelo local, por escolha explícita |

Com provedor remoto (Evolução), o padrão é "estrutura e perfil", e a interface mostra antes de cada envio o que vai sair.

## 5. Adaptadores de execução

### Contrato de conector

```rust
trait Connector {
    fn capabilities(&self) -> ConnectorCapabilities;
    fn check(&self) -> CheckResult;                                    // conecta e confere acesso
    fn discover(&self) -> Result<Vec<Structure>, ConnectorError>;      // tabelas/arquivos, colunas, tipos, chaves declaradas
    fn count(&self, s: &StructureRef) -> Result<Count, ConnectorError>;// exato ou estimado, dizendo qual
    fn profile(&self, s: &StructureRef, how: ProfileHow) -> Result<Profile, ConnectorError>; // pushdown quando declarado
    fn sample(&self, s: &StructureRef, n: u32) -> Result<Rows, ConnectorError>;
    fn read(&self, s: &StructureRef, from: Cursor, limit: u32) -> Result<(Rows, Cursor), ConnectorError>; // paginado
    fn fingerprint(&self, s: &StructureRef) -> Result<Fingerprint, ConnectorError>;
}
trait TargetConnector: Connector {
    fn ddl(&self, plan: &[DdlStatement]) -> Result<Vec<DdlOutcome>, ConnectorError>;
    fn render_ddl(&self, change: &SchemaChange) -> DdlStatement;       // texto para a tela de Comparação
    fn write_batch(&self, b: WriteBatch) -> Result<BatchOutcome, ConnectorError>; // transação por lote, inclui mapa de ids e checkpoint
    fn lookup(&self, entity: &str, keys: &[Value]) -> Result<Vec<Option<Row>>, ConnectorError>; // casar chaves para conflito
}
```

O desenho segue o padrão de especificação → verificação → descoberta → leitura → escrita usado por plataformas de integração (ver [14-referencias-e-roadmap.md](14-referencias-e-roadmap.md)).

### Adaptadores por fase

| Adaptador | Leitura | Escrita | DDL | Paginação | Fase |
|---|---|---|---|---|---|
| CSV | sim | não | não | por posição de byte (cursor = offset da linha) | MVP |
| PostgreSQL | sim | sim, `INSERT … ON CONFLICT`, `RETURNING id` | sim, transacional | por chave (seek) | MVP |
| SQLite | sim | sim, `INSERT … ON CONFLICT` | sim | por chave | MVP ou Evolução (D-01) |
| XLSX/ODS (via `ravenrs-sheet`) | sim | não | não | por linha | Evolução |
| MCP externo | conforme o servidor | conforme o servidor e a permissão | raro | conforme o servidor | Avançado |
| MySQL, SQL Server | sim | sim | sim | por chave | Avançado |

Regras para todos:

- Paginação **por chave** em tabelas; offset só em arquivo. Nunca `OFFSET n` crescente em tabela grande.
- `count` diz se é exato ou estimado (`pg_class.reltuples` é estimado).
- Erros com categoria (`Connection`, `Permission`, `Constraint`, `Type`, `Timeout`) para o motor decidir entre nova tentativa, isolar registro ou parar.

## 6. Motor determinístico

Contrato e comportamento em [12-execucao-e-recuperacao.md](12-execucao-e-recuperacao.md). Resumo do contrato:

```rust
trait Engine {
    fn simulate(&self, plan: &PlanVersion, mode: SimulationMode, coverage: Coverage) -> Result<SimulationResult, EngineError>;
    fn start(&self, job: ExecutionJob) -> Result<JobHandle, EngineError>;       // recusa sem pré-condições
}
trait JobHandle {
    fn events(&self) -> Receiver<ProgressEvent>;
    fn cancel(&self);   // termina o lote em curso
    fn pause(&self);    // Evolução
}
```

O motor é **determinístico**: mesma versão do plano + mesmas origens = mesmo resultado. Nada nele chama modelo de IA.
