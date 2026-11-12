# Periféricos

O RavenRS trata periféricos de automação comercial por drivers: impressora térmica, etiquetadora, pinpad, leitor de código, balança e gaveta. Um driver é um arquivo JSON versionado, em `crates/ravenrs-perifericos/drivers/`, que diz exatamente para qual modelo serve.

**Nenhum driver foi testado em aparelho até agora.** O campo `situacao` diz isso em cada um.

## Por que o driver é dado

Um driver baixado da internet roda na máquina do caixa. Se ele trouxesse código, quem conseguisse publicar um driver, ou roubar a chave, controlaria a loja inteira. Como dado, o pior que um driver pode fazer é o que a validação deixa:

- Só existem os campos conhecidos. Campo a mais recusa o manifesto.
- Não há caminho nem endereço. O único texto com endereço é a `fonte` da documentação, e nada no programa abre esse texto.
- Comando é hexadecimal, de no máximo 16 bytes, e precisa ser exatamente um dos comandos da lista daquele campo e daquele tipo. Um segundo comando escondido depois do primeiro é recusado.
- Pinpad, leitor e balança não enviam comando nenhum por manifesto.

Ficam de fora, de propósito, os comandos que gravam na memória da impressora ou mudam a rede dela, como `GS F9h 5 n` da MP-4200 TH.

Comandos aceitos, em `src/comando.rs`:

| Campo | Aceita | Tipos |
|---|---|---|
| `corte_total`, `corte_parcial` | `GS V m` com m em 0, 1, 48, 49; `GS V m n` com m em 65, 66; `ESC i`; `ESC m` | impressora térmica |
| `bipe` | `ESC ( A` de tamanho coerente, com função 01h, 31h ou 61h; `ESC B n t` | impressora térmica |
| `gaveta` | `ESC p m t1 t2`; `ESC v n`; `ESC 80h n` | impressora térmica, gaveta |
| `comando_modo` | `GS F9h SP n`, a troca que não grava | impressora térmica |

Um comando novo entra por mudança no programa, não por manifesto.

## Como escrever um driver

```json
{
  "id": "bematech.mp-4200-th.cupom",
  "versao": "1.0.0",
  "tipo": "impressora_termica",
  "fabricante": "Bematech",
  "modelos": [{ "nome": "Bematech MP-4200 TH", "variacoes": [], "firmware": "1.2" }],
  "conexoes": ["usb", "serial", "rede"],
  "padroes": { "dpi": 203, "largura_pontos": 576, "baud": 9600 },
  "protocolo": { "nome": "esc_pos", "versao": "opcional" },
  "protocolos_alternativos": [],
  "capacidades": { "corta": true, "corte_parcial": true, "gaveta": true, "bipe": true, "qr_nativo": false, "pix_no_pinpad": false },
  "ajustes": {
    "corte_total": "1D 56 42 18",
    "corte_parcial": "1D 56 42 18",
    "bipe": "1B 28 41 05 00 61 64 01 02 01",
    "gaveta": "1B 70 00 19 FA",
    "comando_modo": "1D F9 20 01",
    "tamanho_bloco": 1024,
    "polaridade_invertida": false
  },
  "situacao": { "estado": "so_pela_documentacao", "fonte": "Manual de programação, revisão 1.0" },
  "min_versao_programa": "27.1.13",
  "instalacao": "O que ajustar no aparelho, para quem instala.",
  "observacoes": "O que não está confirmado."
}
```

| Campo | Regra |
|---|---|
| `id` | `fabricante.modelo.função`, em minúsculas. Nunca muda |
| `versao` | semver. Cada versão é imutável: mudou o conteúdo, mudou o número |
| `tipo` | `impressora_termica`, `etiquetadora`, `pinpad`, `leitor_codigo`, `balanca`, `gaveta` |
| `modelos` | Obrigatório, com o nome exato. Recusa o nome igual ao fabricante, como "TSC", e nomes como "todos". Vale ter variações e firmware |
| `protocolo.nome` | `esc_pos`, `zpl`, `ppla`, `tspl`, `epl`, `abecs`, `hid`, `serial`, compatível com o `tipo`. ABECS precisa de versão |
| `padroes` | `dpi` em 180, 203 ou 300; `largura_pontos` de 8 a 4096; `baud` de 1200 a 115200 |
| `ajustes.tamanho_bloco` | de 1 a 4607 |
| Tamanho | manifesto até 16 KiB, textos curtos até 120 caracteres, frases até 1000, até 32 modelos |

Hoje `report-label` usa `corte_total`, `corte_parcial`, `bipe`, `gaveta`, `comando_modo` e `tamanho_bloco` em ESC/POS, e `polaridade_invertida` em todas as linguagens. `codepage` é aceito e não é usado, porque tudo sai como imagem.

O id no arquivo embutido precisa constar em `src/embutidos.rs`, e `cargo test -p ravenrs-perifericos` valida cada um.

## Situação

```json
{ "estado": "testado_no_aparelho", "data": "2026-11-05", "por": "Nome", "firmware": "1.2" }
{ "estado": "so_pela_documentacao", "fonte": "Manual X, https://..." }
{ "estado": "presumido" }
```

Use `presumido` para o que veio de loja, memória ou modelo parecido, e explique em `observacoes`.

## Como trazer um driver testado em aparelho

1. Rode um caso real: em impressora, cupom com QR e EAN-13, leitura com leitor físico, corte, gaveta e bipe. Em etiquetadora, etiqueta nas linguagens do driver, conferindo que não saiu invertida nem de ponta-cabeça.
2. Veja o firmware na página de autoteste do aparelho.
3. Ajuste o que precisou em `ajustes`. Se o raster saiu invertido, é `polaridade_invertida`.
4. Suba a `versao`, troque a `situacao` por `testado_no_aparelho` com data, nome e firmware, e registre em `FONTES.md` o que foi feito.
5. Só vale para o modelo e o firmware testados. Se outro modelo não foi testado, separe em outro driver.

## Catálogo e assinatura

O catálogo é um arquivo publicado onde a aplicação configurar. O conteúdo vai como texto dentro do envelope, e a assinatura cobre exatamente esses bytes:

```json
{ "conteudo": "{\"formato\":1,...}", "assinatura": "hexadecimal" }
```

O conteúdo, antes de assinar:

```json
{
  "formato": 1,
  "sequencia": 12,
  "validade": "2027-01-31",
  "drivers": [{ "manifesto": { "...": "..." }, "notas": "O que mudou nesta versão" }]
}
```

O catálogo é recusado nesta ordem, cada caso com seu erro: tamanho (`Grande`), envelope (`Formato`), assinatura (`AssinaturaInvalida`), formato, validade (`Vencido`), `sequencia` menor que a última vista (`SequenciaAntiga`, contra quem serve um catálogo velho de propósito), e driver inválido (`ManifestoInvalido`, que recusa o catálogo inteiro).

Regras para publicar:

- A `sequencia` só cresce.
- A `validade` é curta o bastante para limitar um catálogo retido. Publique outro antes de vencer.
- Publique com a mesma chave. Trocar de chave exige versão nova do programa.

### Assinar

A chave privada fica fora do repositório.

```sh
openssl rand -hex 32 > ~/chaves/perifericos.chave
cargo run -p ravenrs-perifericos --example assinar_catalogo -- ~/chaves/perifericos.chave catalogo.json catalogo.assinado.json
```

A ferramenta abre o resultado com as mesmas verificações do cliente antes de gravar, e imprime a chave pública. Essa chave entra no programa ao compilar:

```sh
RAVENRS_PERIFERICOS_CHAVE=<chave pública> cargo build
```

ou é passada pela aplicação com `ChavePublica::de_hex`. Sem isso, `chave_embutida()` devolve `None` e nenhum catálogo é aceito.

**Hoje não existe chave nem endereço de publicação.** Isso é decisão a tomar antes do primeiro catálogo real.

## Uso na aplicação

```rust
let armazem = ArmazemArquivo::novo(caminho);
let mut registro = Registro::carregar(versao_do_programa, &armazem)?;

let transporte = TransporteHttp::novo(url_da_configuracao); // feature http
let catalogo = registro.buscar_catalogo(&transporte, &chave, &hoje_utc())?;
registro.salvar(&armazem)?; // guarda a sequência
let ofertas = verificar_atualizacoes(&catalogo, &registro);

// Quando a pessoa aceitar:
registro.aceitar(&catalogo, &ofertas[0])?;
registro.salvar(&armazem)?;

// O que a aplicação guarda de cada aparelho:
let caixa = Dispositivo { apelido, driver_id, versao_fixada: None, conexao };
let job = LabelJob::do_driver(registro.driver_de(&caixa)?)?;
```

Salve logo depois de abrir um catálogo, ou o número de sequência se perde ao fechar o programa. Uma versão nova que vem com o programa fica só instalada: a ativa não muda sem a pessoa.

## O que isto não protege

- O arquivo local de drivers não é assinado. Quem escreve nele já controla a máquina, e a validação continua limitando o que um manifesto pode fazer.
- A assinatura prova quem publicou, não que o driver funciona. Isso é a `situacao`.
- A validade depende do relógio da máquina.
