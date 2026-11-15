# Raven Data Studio: descoberta, schema, mapeamento e transformações

Exemplos sempre do [cenário de referência](cenario-de-referencia.md).

## 1. Descoberta

### O que é lido

| Informação | Fonte | Base (`basis`) |
|---|---|---|
| Tabelas, colunas, tipos, PK, FK, índices | Catálogo do banco (`information_schema`, `pg_catalog`, `sqlite_master`/`PRAGMA`) ou cabeçalho do CSV | **Declarado** |
| Contagem de linhas | `COUNT(*)` (exato) ou estatística do banco (estimado) | **Calculado** (conjunto) ou **Estimado** |
| Nulos, vazios, distintos, mín./máx. | Consulta agregada no servidor quando `profile_pushdown`; senão, sobre a amostra | **Calculado no conjunto** ou **Calculado na amostra (n)** |
| Tipo inferido | Padrões nos valores (só dígitos, `dd/MM/yyyy`, `1.234,56`) | **Inferido** com confiança |
| Padrão semântico (CPF, CEP, telefone, e-mail) | Expressões regulares + dígito verificador | **Inferido** com confiança |
| Duplicidade por chave candidata | `GROUP BY` normalizado no servidor ou sobre amostra | **Calculado** |
| Relacionamento não declarado | Nome (`COD_CLI` nas duas), tipo compatível, inclusão de valores (% de `PEDIDOS.COD_CLI` presentes em `CLIENTES.COD_CLI`) | **Inferido** com evidência |

No cenário, o perfil de `PEDIDOS.COD_CLI → CLIENTES.COD_CLI` é: mesmo nome, mesmo tipo, 5.298 de 5.312 valores (99,7%) encontrados. A evidência mostrada é exatamente essa frase, e os 14 que faltam são os pedidos sem cliente (C5).

### Regras da descoberta

- Todo número da tela de Perfil mostra a base: "318 vazios · conjunto inteiro" ou "≈ 25% vazios · amostra de 200".
- Confiança de inferência em três faixas com palavra e ícone: **Alta** (≥ 0,9), **Média** (0,7 a 0,9), **Baixa** (< 0,7). Baixa nunca vira mapeamento sozinha.
- O perfil nunca puxa o conjunto para a interface: o cálculo é no servidor (pushdown) ou no serviço do módulo, que lê em páginas e acumula.

## 2. Schema

### Disposições por entidade

| Disposição | Significa | No cenário |
|---|---|---|
| `UseExisting` | Grava em tabela que já existe, sem alterar | — |
| `Alter` | Tabela existe; o plano acrescenta colunas ou índices | `customers` + `legacy_id` |
| `Create` | Tabela nova | `customer_addresses`, `orders` |
| `Staging` | Estrutura intermediária, só durante a execução | lista de mescla de CPF duplicado (tabela temporária) |

Estruturas `Staging` aparecem na tela de Schema com o selo "Intermediária: não fica no destino". Nunca aparecem no DDL permanente.

### Tipos lógicos

| Lógico | PostgreSQL | SQLite | Observação |
|---|---|---|---|
| `Text { max }` | `VARCHAR(max)` ou `TEXT` | `TEXT` | |
| `FixedText { len }` | `CHAR(len)` | `TEXT` + `CHECK(length)` | `document CHAR(11)`, `postal_code CHAR(8)` |
| `Integer { bits }` | `INTEGER` / `BIGINT` | `INTEGER` | |
| `Decimal { precision, scale }` | `NUMERIC(p,s)` | `TEXT` (exato) ou `REAL` (D-05) | `total NUMERIC(12,2)` |
| `Date` | `DATE` | `TEXT` ISO 8601 | |
| `DateTime { tz }` | `TIMESTAMP [WITH TIME ZONE]` | `TEXT` ISO 8601 | |
| `Boolean` | `BOOLEAN` | `INTEGER` 0/1 | |
| `Enum { values }` | `VARCHAR` + `CHECK IN (…)` | `TEXT` + `CHECK` | `orders.status` |
| `Identity` | `BIGINT GENERATED ALWAYS AS IDENTITY` | `INTEGER PRIMARY KEY` | chaves novas |

O tipo lógico **não** é `FieldKind` do ravenrs-app, porque `FieldKind::Decimal(u8)` é `f64` com 15 dígitos significativos. A projeção para `EntityInfo` (para validar expressões) usa o `FieldKind` mais próximo e marca a perda.

### Identificadores legados

- `legacy_id` nas entidades que vêm de uma tabela com chave (`customers`, `orders`).
- Mescla (C1): o sobrevivente guarda `legacy_id = 1042`; os mesclados ficam no mapa de ids como **aliases** (`2231 → mesmo id`), para a religação dos pedidos (C12) e para auditoria. A coluna `legacy_id` não vira lista.

### DDL proposto do cenário (texto revisável)

```sql
ALTER TABLE customers ADD COLUMN legacy_id VARCHAR(20) NULL;
CREATE UNIQUE INDEX customers_legacy_id_uq ON customers (legacy_id);

CREATE TABLE customer_addresses (
  id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  customer_id  BIGINT NOT NULL REFERENCES customers (id),
  kind         VARCHAR(10) NOT NULL CHECK (kind IN ('main', 'delivery')),
  street       VARCHAR(120) NOT NULL,
  number       VARCHAR(10) NULL,
  complement   VARCHAR(60) NULL,
  district     VARCHAR(60) NULL,
  city         VARCHAR(60) NOT NULL,
  state        CHAR(2) NOT NULL,
  postal_code  CHAR(8) NOT NULL,
  UNIQUE (customer_id, kind)
);

CREATE TABLE orders (
  id           BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  customer_id  BIGINT NOT NULL REFERENCES customers (id),
  ordered_at   DATE NOT NULL,
  total        NUMERIC(12,2) NOT NULL,
  status       VARCHAR(10) NOT NULL CHECK (status IN ('open', 'invoiced', 'cancelled')),
  legacy_id    VARCHAR(20) NOT NULL UNIQUE
);
CREATE INDEX orders_customer_id_ix ON orders (customer_id);
```

## 3. Mapeamento

### Cardinalidades

| Cardinalidade | Exemplo | Como o plano representa |
|---|---|---|
| 1:1 | `PEDIDOS` → `orders` | `EntityMapping { from: [PEDIDOS], to: orders, kind: OneToOne }` |
| N:1 (mescla) | 2 linhas de `CLIENTES` com mesmo CPF → 1 `customers` | `kind: ManyToOne { group_by: KeepDigits(CPF), survivor: OldestBy(DT_CADASTRO) }` |
| 1:N (divisão) | 1 linha de `CLIENTES` → 1 `customers` + até 2 `customer_addresses` (`main` de `ENDERECO`, `delivery` de `END_ENTREGA`) | dois `EntityMapping` da mesma origem; o de `delivery` com condição `Not IsNullOrEmpty([END_ENTREGA])` |

### Tipos de mapeamento de campo

| Tipo | Exemplo |
|---|---|
| `FromField` | `orders.legacy_id ← NUM_PED` |
| `FromExpression` | `customers.name ← Trim(…)` (ver regras) |
| `Constant` | `customer_addresses.kind ← 'main'` |
| `Lookup` | `orders.customer_id ← id de customers onde legacy_id (ou alias) = COD_CLI` |
| `Ignored` | `CLIENTES.DT_CADASTRO` não vai para colunas, só decide o sobrevivente |
| `Unmapped` (pendência) | campo obrigatório de destino sem origem: bloqueia aprovação |

### Mapeamento completo do cenário

| Destino | Origem | Regras | Política |
|---|---|---|---|
| `customers.document` | `CLIENTES.CPF` | `KeepDigits` → `Check::Cpf` (erro → revisão) | chave de casamento |
| `customers.name` | `CLIENTES.NOME` | `Trim` → `CollapseSpaces` → `Case(Title)` | não atualizável |
| `customers.phone` | `CLIENTES.FONE` | `EmptyToNull` → `KeepDigits` → formatar `(00) 00000-0000` | atualizável; vazio não apaga |
| `customers.created_at` | `CLIENTES.DT_CADASTRO` | `ParseDate("dd/MM/yyyy")` | só na inserção |
| `customers.legacy_id` | `CLIENTES.COD_CLI` | do sobrevivente | |
| `customer_addresses.*` (`main`) | `CLIENTES.ENDERECO`, `CIDADE_UF`, `CEP` | `Split(Address)`, `Split("/")`, `PadLeft(8,'0')` | ambíguo → revisão |
| `customer_addresses.*` (`delivery`) | `CLIENTES.END_ENTREGA`, `CIDADE_UF`, `CEP` | idem | só quando preenchido |
| `orders.customer_id` | `PEDIDOS.COD_CLI` | `Lookup` pelo mapa de ids (com aliases) | ausente → rejeitar |
| `orders.ordered_at` | `PEDIDOS.DATA` | `ParseDate("dd/MM/yyyy")` | |
| `orders.total` | `PEDIDOS.VALOR` | `ParseDecimal('.', ',', 2)` | |
| `orders.status` | `PEDIDOS.SITUACAO` | `Trim` → `Case(Upper)` → `MapValues` | sem correspondência → revisão |
| `orders.legacy_id` | `PEDIDOS.NUM_PED` | | chave de casamento |

### Sincronização entre canvas e grade

- Ambos leem o mesmo `MappingPlan`. Selecionar uma linha na grade seleciona a ligação no canvas e vice-versa (seleção é estado de vista compartilhado, `PlanSelection`).
- Toda edição vira `PlanCommand` → `PlanPatch`. A outra vista atualiza pelo binding do plano.
- Posição dos nós e zoom são `ProjectViewState`, fora do plano: mexer no canvas não invalida a simulação.

## 4. Transformações

### Catálogo

| Operação | Antes | Depois | Nulo | Erro |
|---|---|---|---|---|
| `Trim` + `CollapseSpaces` + `Case(Title)` | `MARIA  DA SILVA ` | `Maria da Silva` (preposições em minúsculas) | mantém nulo | — |
| `KeepDigits` | `123.456.789-09` | `12345678909` | mantém nulo | — |
| `PadLeft(8,'0')` | `1310100` | `01310100` | erro se obrigatório | mais de 8 dígitos → erro do registro |
| `ParseDate("dd/MM/yyyy")` | `15/03/2024` | `2024-03-15` | conforme política | `31/02/2024` → erro do registro |
| `ParseDecimal('.', ',', 2)` | `1.234,56` | `1234.56` | conforme política | `1,234.56` → erro (formato não esperado) |
| `EmptyToNull` | `""`, `"   "` | `NULL` | — | — |
| `Split(Address)` | `Rua das Flores, 120 - ap 31 - Centro` | `street=Rua das Flores`, `number=120`, `complement=ap 31`, `district=Centro` | — | sem número reconhecível → **revisão** (C9) |
| `Split("/")` | `São Paulo/SP` | `city=São Paulo`, `state=SP` | — | sem `/` → revisão |
| `MapValues` | `FAT` | `invoiced` | — | sem correspondência → revisão |
| `Expression` | qualquer expressão `ravenrs-criteria` | | | erro de avaliação → política da regra |

### O que vem do `ravenrs-criteria` e o que é próprio

| Necessidade | `ravenrs-criteria` | Decisão |
|---|---|---|
| Aparar, caixa, concatenar, substituir, tamanho | Sim (`Trim`, `Upper`, `Lower`, `Concat`, `Replace`, `Len`…) | `Expression` ou op dedicada (mesmo resultado) |
| Condições | Sim (`Iif`, `IsNullOrEmpty`, operadores) | `Expression` |
| Datas (componentes, somas) | Sim (67 funções de data e hora) | `Expression` |
| Ler data em formato dado | A verificar no catálogo; não presumido | `ParseDate` próprio |
| Ler decimal com separadores escolhidos | Não presumido | `ParseDecimal` próprio |
| Só dígitos, completar à esquerda | Parcial (`Replace` não cobre "tudo que não é dígito"; `PadLeft` a verificar) | Ops próprias; avaliar registrar `KeepDigits` / `PadLeft` como funções (`Environment::register`) |
| Validar CPF/CNPJ | Não | `Check::Cpf` próprio; registrar `IsCpf` como função |
| Dividir em vários campos | Não (uma expressão tem um resultado) | `Split` próprio |
| Tabela de valores | Possível com `Iif` encadeado, ilegível | `MapValues` próprio |
| Lookup em outra entidade, JOIN | **Não** (sem join, sem subconsulta) | `Lookup` próprio, resolvido pelo motor |
| Agregados por grupo (mescla) | Agregados existem sobre linhas do escopo, não `GROUP BY` | `ManyToOne` próprio |
| Checagem de tipo | **Não** | O domínio confere tipo lógico de entrada e saída de cada regra |

### Regras propostas pela IA

A IA propõe `TransformationRule` com `examples` tirados da amostra. O domínio:

1. aplica a regra aos exemplos e confere o resultado;
2. aplica à amostra inteira e conta afetados, erros e nulos;
3. recusa se algum exemplo não bate, ou aceita como sugestão mostrando "aplica a 806 de 1.248; 0 erros".

Regra em texto livre ("formate o CPF") não é aceita; o agente precisa produzir a operação.

## 5. Validações e conflitos

Validações e a política de conflitos estão modeladas em [09-modelo-de-dominio.md](09-modelo-de-dominio.md#validationrule). No cenário:

| Entidade | Chave | Em caso de existir | Atualizáveis | Vazio na origem | Duplicado na origem | Referência ausente |
|---|---|---|---|---|---|---|
| `customers` | `document` | Upsert | `phone` | mantém destino | Mesclar, sobrevive o mais antigo | — |
| `customer_addresses` | `customer_id + kind` | Upsert | todos | mantém destino | — | — (cliente vem antes) |
| `orders` | `legacy_id` | Upsert (reexecução) | `status`, `total` | mantém destino | — | Rejeitar (C5); reter se o cliente está em revisão (C3) |
