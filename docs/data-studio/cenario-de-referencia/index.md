# Raven Data Studio: cenário de referência

Todos os documentos desta pasta usam o mesmo cenário. **Este arquivo é a fonte das contagens**: quem citar um número de registros cita daqui, e quem mudar um número muda aqui primeiro.

O cenário é fictício. Nomes, CPFs e endereços abaixo são inventados e os CPFs são números de teste (os válidos passam no dígito verificador, o de 1777 foi escolhido para falhar).

## O pedido

> "Importe clientes e pedidos do sistema antigo. Separe os endereços, identifique duplicidades por CPF, preserve os vínculos dos pedidos e crie as tabelas necessárias."

- **Origem:** banco do sistema antigo (`legado`), tabelas `CLIENTES` e `PEDIDOS`. No MVP a mesma origem também chega como dois arquivos CSV exportados (`clientes.csv`, `pedidos.csv`), com as mesmas colunas.
- **Destino:** banco relacional do aplicativo novo (`loja`), com `customers` já existente e `customer_addresses` e `orders` a criar.

## Estrutura da origem

### CLIENTES (1.248 registros)

| Coluna | Tipo declarado | Exemplo | Observação |
|---|---|---|---|
| `COD_CLI` | `INTEGER` (PK) | `1042` | Identificador legado; vira `legacy_id`. |
| `NOME` | `VARCHAR(80)` | `MARIA  DA SILVA ` | Espaços duplos e no fim; tudo em maiúsculas. |
| `CPF` | `VARCHAR(14)` | `123.456.789-09` | Com e sem pontuação misturados. |
| `FONE` | `VARCHAR(20)` | `(11) 9 8765-4321` ou vazio | 318 vazios. |
| `ENDERECO` | `VARCHAR(200)` | `Rua das Flores, 120 - ap 31 - Centro` | Logradouro, número, complemento e bairro num campo só. |
| `CIDADE_UF` | `VARCHAR(60)` | `Campinas/SP` | Cidade e UF juntas. |
| `CEP` | `INTEGER` | `1310100` | Guardado como número: perdeu o zero inicial em 211 registros (`01310-100`). |
| `END_ENTREGA` | `VARCHAR(200)` | vazio ou endereço | Preenchido em 389 registros. |
| `DT_CADASTRO` | `VARCHAR(10)` | `03/02/2019` | Data brasileira em texto. |

### PEDIDOS (5.312 registros)

| Coluna | Tipo declarado | Exemplo | Observação |
|---|---|---|---|
| `NUM_PED` | `INTEGER` (PK) | `88017` | Vira `legacy_id`. |
| `COD_CLI` | `INTEGER` | `1042` | **Sem chave estrangeira declarada**; o vínculo é inferido. |
| `DATA` | `VARCHAR(10)` | `15/03/2024` | Data brasileira em texto. |
| `VALOR` | `VARCHAR(15)` | `1.234,56` | Número com milhar em ponto e decimal em vírgula. |
| `SITUACAO` | `VARCHAR(12)` | `FAT` | Valores diferentes para a mesma situação. |

Valores encontrados em `SITUACAO` (perfil sobre o conjunto inteiro):

| Valor na origem | Registros | Destino proposto |
|---|---:|---|
| `ABERTO` | 612 | `open` |
| `A` | 48 | `open` |
| `FATURADO` | 3.904 | `invoiced` |
| `FAT` | 377 | `invoiced` |
| `CANCELADO` | 241 | `cancelled` |
| `Cancelado` | 98 | `cancelled` |
| `CANC` | 26 | `cancelled` |
| `X` | 6 | **sem correspondência: revisão** |
| **Total** | **5.312** | |

## Estrutura do destino

| Tabela | Situação | Colunas principais |
|---|---|---|
| `customers` | **Existe**, com 4.310 registros, dos quais 142 têm o CPF de algum cliente da origem. | `id BIGINT PK`, `name VARCHAR(120) NOT NULL`, `document CHAR(11) UNIQUE NOT NULL`, `phone VARCHAR(20) NULL`, `created_at DATE`, `legacy_id VARCHAR(20) NULL` (proposta: coluna nova) |
| `customer_addresses` | **A criar** | `id BIGINT PK`, `customer_id BIGINT FK → customers.id`, `kind VARCHAR(10)` (`main`, `delivery`), `street`, `number`, `complement`, `district`, `city`, `state CHAR(2)`, `postal_code CHAR(8)` |
| `orders` | **A criar** | `id BIGINT PK`, `customer_id BIGINT FK → customers.id NOT NULL`, `ordered_at DATE NOT NULL`, `total NUMERIC(12,2) NOT NULL`, `status VARCHAR(10) NOT NULL` (`open`, `invoiced`, `cancelled`), `legacy_id VARCHAR(20) UNIQUE` |

## Os doze casos e as contagens

| # | Caso | Onde | Registros | Tratamento proposto |
|---|---|---|---:|---|
| C1 | CPF duplicado | CLIENTES | 37 grupos de 2 (74 registros) | Mesclar: sobrevive o de `DT_CADASTRO` mais antiga; o outro vira alias de `legacy_id`. |
| C2 | Documento com pontuação | CLIENTES | 806 | Remover tudo que não é dígito antes de validar e comparar. |
| C3 | CPF com dígito verificador inválido | CLIENTES | 9 | Revisão; o cliente e os seus pedidos ficam retidos. |
| C4 | CEP com zero inicial perdido | CLIENTES | 211 | Completar com zeros à esquerda até 8 dígitos. |
| C5 | Pedido sem cliente | PEDIDOS | 14 | Rejeitar com motivo `referencia_ausente`. |
| C6 | Data brasileira | ambas | todas | Ler `dd/MM/yyyy`; data impossível vira erro do registro (nenhuma no cenário). |
| C7 | Valor `1.234,56` | PEDIDOS | todos | Ler com separador de milhar `.` e decimal `,`; gravar `NUMERIC(12,2)`. |
| C8 | Telefone vazio | CLIENTES | 318 | Gravar `NULL` (não texto vazio). |
| C9 | Endereço ambíguo | CLIENTES | 23 | Revisão: a divisão não achou número nem separador confiável. |
| C10 | Status diferentes | PEDIDOS | 8 valores | Tabela de correspondência acima; `X` vai para revisão (6). |
| C11 | Cliente existente no destino | CLIENTES × customers | 142 | Upsert pela chave `document`: atualiza `phone` só se a origem não estiver vazia; nunca sobrescreve `name`. |
| C12 | Pedidos de cliente duplicado | PEDIDOS | 96 | Religar ao cliente sobrevivente de C1. |

## Conta de chegada

A conta precisa fechar em toda tela que mostra números (Simulação, Revisão final, Execução, Resultado).

### Clientes

| Passo | Registros |
|---|---:|
| Lidos de CLIENTES | 1.248 |
| − duplicados mesclados (C1) | −37 |
| = clientes únicos | 1.211 |
| − retidos em revisão por CPF inválido (C3) | −9 |
| = clientes a gravar | **1.202** |
| … dos quais atualizam um existente (C11) | 142 |
| … dos quais inserem um novo | 1.060 |

### Endereços

| Passo | Registros |
|---|---:|
| Endereços principais dos clientes a gravar | 1.202 |
| − ambíguos em revisão (C9) | −23 |
| = principais a inserir | 1.179 |
| + endereços de entrega (`END_ENTREGA` preenchido, todos de clientes a gravar) | 389 |
| = endereços a inserir | **1.568** |

Os 23 clientes com endereço ambíguo são gravados normalmente; só o endereço principal deles fica pendente.

### Pedidos

| Passo | Registros |
|---|---:|
| Lidos de PEDIDOS | 5.312 |
| − sem cliente (C5), rejeitados | −14 |
| − de clientes retidos por CPF inválido (C3), retidos | −31 |
| − com status `X` (C10), em revisão | −6 |
| = pedidos a inserir | **5.261** |
| … dos quais religados ao sobrevivente (C12) | 96 |

### Pendências que a execução deixa

| Destino | Motivo | Registros |
|---|---|---:|
| Revisão | CPF inválido (C3) | 9 clientes |
| Retidos | Pedidos desses clientes | 31 pedidos |
| Revisão | Endereço ambíguo (C9) | 23 endereços |
| Revisão | Status sem correspondência (C10) | 6 pedidos |
| Rejeitados | Pedido sem cliente (C5) | 14 pedidos |
| **Total fora da gravação** | | **83** (9 + 31 + 23 + 6 + 14) |

Conferência: `1.202 + 1.568 + 5.261 = 8.031` linhas gravadas no destino.

## Registros de amostra

Estes são os registros que as telas e os wireframes mostram. Eles cobrem cada caso.

### CLIENTES

| COD_CLI | NOME | CPF | FONE | ENDERECO | CIDADE_UF | CEP | DT_CADASTRO | Casos |
|---:|---|---|---|---|---|---:|---|---|
| 1042 | `MARIA  DA SILVA ` | `123.456.789-09` | `(11) 9 8765-4321` | `Rua das Flores, 120 - ap 31 - Centro` | `São Paulo/SP` | `1310100` | `03/02/2019` | C2, C4 |
| 2231 | `MARIA DA SILVA` | `12345678909` | vazio | `R. das Flores 120 ap31` | `São Paulo/SP` | `1310100` | `11/07/2021` | C1 (duplica 1042), C4, C8 |
| 1500 | `JOÃO PEREIRA` | `987.654.321-00` | `(19) 3232-1010` | `Av. Brasil, 900 - Jardim Paulista` | `Campinas/SP` | `13010000` | `20/05/2018` | C2, C11 (já existe no destino) |
| 1777 | `ANA COSTA` | `111.222.333-44` | `(21) 99999-0000` | `Travessa Sete` | `Niterói/RJ` | `24020000` | `09/09/2020` | C3 (CPF inválido), C9 |
| 1890 | `PAULO SOUZA` | `22233344402` | vazio | `Rua A Quadra 3 Lote 7 Setor Sul` | `Goiânia/GO` | `74083010` | `30/01/2022` | C8, C9 (ambíguo) |

### PEDIDOS

| NUM_PED | COD_CLI | DATA | VALOR | SITUACAO | Casos |
|---:|---:|---|---|---|---|
| 88017 | 1042 | `15/03/2024` | `1.234,56` | `FAT` | C6, C7, C10 |
| 88018 | 2231 | `16/03/2024` | `89,90` | `ABERTO` | C12 (vai para 1042) |
| 88019 | 9999 | `16/03/2024` | `45,00` | `FATURADO` | C5 (cliente 9999 não existe) |
| 88020 | 1777 | `17/03/2024` | `310,00` | `Cancelado` | Retido (cliente em revisão por C3) |
| 88021 | 1500 | `18/03/2024` | `2.000,00` | `X` | C10 (status sem correspondência) |

### Resultado esperado da amostra

| Origem | Destino | Ação |
|---|---|---|
| 1042 + 2231 | `customers` 1 linha (`document=12345678909`, `name=Maria da Silva`, `phone=(11) 98765-4321`, `legacy_id=1042`, alias `2231`) | Inserir |
| 1042 | `customer_addresses` (`main`, `Rua das Flores`, `120`, `ap 31`, `Centro`, `São Paulo`, `SP`, `01310100`) | Inserir |
| 1500 | `customers` existente: `phone` atualizado, `name` preservado | Atualizar |
| 1777 | nenhum | Revisão (C3) |
| 1890 | `customers` com `phone=NULL`; endereço pendente | Inserir + revisão (C9) |
| 88017 | `orders` (`ordered_at=2024-03-15`, `total=1234.56`, `status=invoiced`) | Inserir |
| 88018 | `orders` com `customer_id` do cliente de 1042 | Inserir (religado) |
| 88019 | `RejectedRecord` motivo `referencia_ausente` | Rejeitar |
| 88020 | retido até o cliente 1777 ser decidido | Reter |
| 88021 | `RejectedRecord` motivo `valor_sem_correspondencia`, fila de revisão | Revisão |
