# ravenrs-nfse

> **BETA, prova de conceito. Não homologado. Produção bloqueada no código.** Não emita documento fiscal real com este crate.

NFS-e do Padrão Nacional: o prestador escreve a DPS, assina e envia à Sefin Nacional, que devolve a NFS-e. Não passa mais pela prefeitura, nem pela SEFAZ: o órgão é o CGNFS-e, com a Receita Federal. As fontes e o que ainda não foi confirmado estão em [`docs/NFSE.md`](../../docs/NFSE.md).

Usa o `ravenrs-dfe` para XML, C14N, assinatura, transporte, GZip + Base64, certificados e a cadeia de handlers do sandbox. Nada aqui é cópia do `ravenrs-nfe`.

## O que cobre

- Prestador ME/EPP do Simples Nacional (`opSimpNac=3`, `regApTribSN=1`, `regEspTrib=0`), só `tpAmb=2`.
- Um serviço por DPS, ISS tributável sem retenção (`tribISSQN=1`, `tpRetISSQN=1`), `indTotTrib=0`, tomador opcional.
- DPS na ordem do XSD v1.01, sem formatação e sem CR em `xDescServ` (causa relatada de E0714).
- `POST /nfse`, `HEAD` e `GET /dps/{id}`, `GET /nfse/{chave}`, cancelamento `e101101` em `POST /nfse/{chave}/eventos` e convênio do município.
- **Recuperação depois de resposta perdida.** Em timeout, queda de conexão, 5xx ou E0014, o emissor procura a NFS-e pelo Id da DPS antes de dizer qualquer coisa. Se não achar, devolve `NaoConfirmada`, que manda reenviar a mesma DPS, nunca com outro número.
- Assinatura com `Algoritmo::NFE` (RSA-SHA1, o padrão) ou `Algoritmo::SHA256`, porque o algoritmo vigente não está confirmado.

```rust
use ravenrs_nfse::sandbox::{SefinSandbox, cenarios};
use ravenrs_nfse::{BetaAck, Emissor, exemplo};

let sandbox = SefinSandbox::new();
sandbox.use_once(cenarios::resposta_perdida());
let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
    .signer(ravenrs_dfe::certificados::certificado_curinga()?)
    .transport(sandbox.clone())
    .build()?;
let emitida = emissor.emitir(&exemplo::dps(1))?;
assert!(emitida.recuperada);
assert_eq!(sandbox.emitidas(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Sandbox da Sefin

`SefinSandbox` implementa `Transport`, como o MSW: `post`, `get` e `head` com rota (`/dps/{id}`), `use_once`, `reset_handlers`, `on_unhandled` e `requests()`. Sem handler, ele confere de verdade:

- GZip + Base64 e JSON;
- raiz, namespace e versão;
- o Id contra os campos de que é feito (E0004);
- `tpAmb=2` (E0006) e data de emissão depois do processamento (E0008);
- a assinatura, com o hash que ela usou (E0714);
- convênio do município (E0037), DPS repetida (E0014) e cancelamento repetido (E0840).

Cada código tem fonte e confiança em `sandbox::REGRAS`. Todos os `E` vêm de catálogo de terceiros: a tabela oficial (Anexo I, `.xlsx`) não foi lida. Cenários prontos: `resposta_perdida`, `timeout`, `indisponivel`, `recusar(codigo)` e `certificado_recusado` (HTTP 403 do mTLS).

A NFS-e que o sandbox devolve é reduzida: `infNFSe` com Id, número, data de processamento e a DPS recebida. Não segue o `TCInfNFSe` inteiro.

## O que não foi confirmado

- Os nomes dos campos JSON e o formato dos erros vêm do ACBr e do nfse-php. O Swagger oficial não abriu.
- O prefixo da URL (`/SefinNacional` ou `/API/SefinNacional`). As rotas são comparadas pelo fim do caminho.
- O algoritmo de assinatura vigente e o DV da chave. A chave sempre vem da resposta; só o sandbox calcula um DV, por módulo 11.
- O Id do pedido de evento (`PRE` + chave + `101101`, como faz o ACBr) e o tamanho da justificativa (15 a 255, presumido igual ao da NF-e).
- A DPS não foi validada com `xmllint` contra o XSD oficial, porque os arquivos não foram baixados. A ordem dos elementos foi lida no XSD v1.01.
- Nenhum envio real foi feito. A produção restrita pede certificado ICP-Brasil e CNPJ reais.

Fora da POC: substituição, retenção, deduções, intermediário, exterior, obra, IBS/CBS e Lucro Presumido/Real.
