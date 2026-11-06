# ravenrs-nfe

> **BETA, PROVA DE CONCEITO. NÃO HOMOLOGADO. NÃO USE EM PRODUÇÃO.**
> Este crate nunca falou com um servidor real da SEFAZ. Produção está bloqueada no código: `Ambiente` só tem `Sandbox` e `Homologacao`, e não existe caminho que escreva `tpAmb=1`. Um emissor só existe com `BetaAck::i_understand_this_is_experimental()`, e todo documento diz em `infCpl` que veio de uma POC beta.

Motor de NF-e e NFC-e sem interface: rascunho, totais em decimal exato, chave de acesso, XML determinístico, C14N inclusiva própria, XMLDSig, SOAP 1.2 e leitura dos retornos. Não conhece `ravenrs-app` e compila em `wasm32` sem a feature `http`.

## Limites da POC

- Só Simples Nacional na emissão: CRT 1, CSOSN 102 e 500.
- Só autorização síncrona (`indSinc=1`), modelos 55 e 65.
- Status, autorização, consulta, cancelamento, distribuição e manifestação. Sem inutilização, contingência ou DANFE.
- Poucas UFs na tabela de endereços: SP, RS, PR, MG, SC, RJ, ES e DF, todas em homologação.

`REGRAS.md` traz a origem de cada regra e o que ficou sem fonte.

## Uso

```rust
use ravenrs_nfe::sandbox::{SandboxSigner, SefazSandbox};
use ravenrs_nfe::{BetaAck, Emissor};

let server = SefazSandbox::new();
let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
    .emitente(emitente)
    .signer(SandboxSigner::new()?)
    .transport(server.clone())
    .build()?;
let emissao = emissor.emitir(&rascunho)?;
```

Todo envio bloqueia: no app, chame o emissor em uma thread separada.

## Sandbox estilo MSW

`SefazSandbox` implementa `Transport`. O emissor monta os mesmos bytes e chama os mesmos endereços; só o transporte muda. Sem handler seu, o sandbox confere de verdade: envelope e ação, `tpAmb=2`, assinatura recalculada com o certificado do próprio XML, DV e concatenação da chave, textos de homologação, ordem das tags, totais e duplicidade de chave e de número. A resposta vem no formato SOAP real, com `verAplic=RAVENRS-SANDBOX` e `xMotivo` começando em `[SANDBOX] `.

```rust
use ravenrs_nfe::sandbox::{OnUnhandled, autorizacao, cenarios};

server.use_once(autorizacao(cenarios::paralisado_108));
server.use_handler(autorizacao(|pedido| pedido.responder_com(999, "Texto meu")));
server.on_unhandled(OnUnhandled::Warn);
server.reset_handlers();
let enviados = server.requests();
```

Cenários prontos: 100, 107, 108, 204, 225, 297, 539, 598, 1115, 129 e timeout.

### Ferramenta de diagnóstico

Tudo sem rede e sem tela, em `ravenrs_nfe::sandbox`:

- `diagnosticar(xml)` lista todos os problemas, com campo, encontrado, esperado, correção, fonte e confiança. Vem do mesmo catálogo de que o `SefazSandbox` responde: ele devolve o primeiro, como a SEFAZ.
- `autopsia_assinatura(xml)` explica um 297: digest calculado e informado, o byte onde diverge e a causa, dizendo se ela foi comprovada ou é só indício.
- `Incidente::de_producao(xml, retorno)` compara a resposta real com o que o catálogo prevê e diz quando não reproduz, sem inventar causa. `relatorio_markdown()` sai mascarado.
- `Gravador` e `Reprodutor` levam um problema de campo para o laboratório, em JSON Lines mascarado.
- `injetar(RegraDeFalha)` provoca queda antes da resposta, latência, SOAP Fault, HTTP 500, TLS, resposta truncada, XML inválido, paralisação por janela de tempo e intermitência com semente fixa. `usar_relogio` controla o tempo.
- `linha_do_tempo()` e `relatorio_markdown(chave)` contam a história de uma chave.

### Certificados de laboratório

`sandbox::certificados` emite e-CNPJ de teste no formato ICP-Brasil, pela AC `RAVENRS SANDBOX AC - SEM VALOR`: A1 em PFX real (com a feature `http`), A3 simulado com `TokenSimulado`, e um construtor para cada falha. Eles não saem do laboratório: `Ambiente::Homologacao` e `HttpTransport` recusam certificado desta AC, e o sandbox recusa qualquer outro.

As chaves do laboratório ficam em `crates/ravenrs-dfe/fixtures/laboratorio/` e tudo em `fixtures/http-test/` é público de propósito e não vale nada. `crates/ravenrs-dfe/scripts/gerar-chaves-laboratorio.sh` e `scripts/gerar-fixtures.sh` regeneram os arquivos.

### Entrada de mercadoria

`entrada::ler_nota_recebida(xml)` lê `nfeProc` ou `NFe` como texto hostil. `Emissor::buscar_documentos`, `baixar_por_chave`, `manifestar` e `proxima_consulta_em` falam com o Ambiente Nacional: o resumo `resNFe` só vira XML completo depois da ciência, e o limitador evita a rejeição 656. No sandbox, `gerar_nota_de_compra` e `colocar_nota_recebida` alimentam esse fluxo.

## Homologação real (feature `http`, só nativo)

`HttpTransport` usa `ureq` e `rustls`, com certificado cliente vindo de um PFX A1. As raízes (cadeia ICP-Brasil) são informadas por arquivo PEM, a verificação não tem como ser desligada, HTTP simples é recusado e redirecionamento não é seguido. A senha fica só em memória e não aparece em `Debug`.

```rust
use ravenrs_nfe::Ambiente;
use ravenrs_nfe::http::{CertificadoA1, HttpTransport, SenhaA1};

let a1 = CertificadoA1::from_pfx(&pfx, &SenhaA1::new(senha))?;
let transporte = HttpTransport::builder(&a1).raizes_pem_arquivo(&raizes)?.build()?;
let emissor = Emissor::builder(BetaAck::i_understand_this_is_experimental())
    .ambiente(Ambiente::Homologacao)
    .emitente(emitente)
    .signer(a1)
    .transport(transporte)
    .build()?;
```

No ambiente `Sandbox`, o emissor recusa um transporte que chegue à rede.

## Conferências independentes

```sh
cargo test -p ravenrs-nfe --all-features
scripts/validar-xsd.sh <pasta com os esquemas PL_010>
```

Os testes de `tests/externos.rs` comparam a C14N com `xmllint --c14n` e verificam digest e assinatura com `openssl`, cortando o XML como texto para não reaproveitar nada do crate. Se a ferramenta não existir, o teste avisa e passa.
