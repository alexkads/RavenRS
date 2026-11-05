---
name: ravenrs-fiscal
description: Roteiro da parte fiscal e de automação comercial do RavenRS - motor de NF-e e NFC-e, sandbox da SEFAZ, diagnóstico de rejeição, certificados, XSD, DANFE e cupom, etiquetas e impressoras, pinpad e TEF, caixa e o mini-PDV. Use sempre que o pedido citar NF-e, NFC-e, NFS-e, SEFAZ, DANFE, cupom, etiqueta, ZPL, ESC/POS, pinpad, TEF, PDV, caixa, certificado A1, CSOSN, cStat ou rejeição, mesmo que não cite esta skill. Diz onde mexer, de onde tirar cada regra e o que só a homologação prova.
---

# Fiscal e automação comercial no RavenRS

Tudo aqui é **beta, prova de conceito**. Nenhuma nota foi a um servidor real da SEFAZ, nenhuma etiqueta saiu em impressora física, nenhum pagamento passou por pinpad ou provedor real. Nunca escreva o contrário em código, documento ou relatório.

| Documento | Quando ler |
|---|---|
| `docs/AUTOMACAO-COMERCIAL.md` | O que existe, como foi verificado e o que falta para vender |
| `crates/ravenrs-nfe/README.md` | Uso do emissor e do sandbox |
| `crates/ravenrs-nfe/REGRAS.md` | De onde veio cada regra e o que ficou sem fonte. **Atualize junto com qualquer regra nova** |
| `crates/ravenrs-report-label/FONTES.md` | O que cada manual de impressora confirmou |
| `docs/PAGAMENTOS.md` | Pagamentos: origem das regras e homologação de cada provedor |
| `docs/TEF-COMPARATIVO.md`, `docs/PERIFERICOS.md` | Comparativo de TEF; periféricos como dados, em `ravenrs-perifericos` |
| `docs/NFSE.md` | Pesquisa do Padrão Nacional. A NFS-e ainda não tem código |

## Onde mexer

| Tarefa | Onde | Também |
|---|---|---|
| Regra do documento, totais, chave, XML | `crates/ravenrs-nfe/src/`: `modelo`, `totais`, `chave`, `documento`, `xml` | `REGRAS.md`, testes em `tests/fluxo.rs`, XSD |
| Assinatura e C14N | `assinatura`, `c14n` | `tests/externos.rs`, que confere com `xmllint` e `openssl` |
| Serviço novo da SEFAZ | `soap`, `retorno`, `urls`, depois um handler em `sandbox/` | O sandbox tem de responder igual ao formato real |
| Conferência ou rejeição nova do sandbox | Regra em `sandbox/catalogo.rs`, verificação em `sandbox/analise.rs`; o que depende do que o servidor guarda fica em `sandbox/conferencia.rs` | Regra com data vai em `sandbox/vigencia.rs` |
| Cenário pronto | `sandbox/cenarios.rs` | Aparece no painel de `crates/ravenrs-app-pos/src/telas.rs` |
| Rede real, A1 | `http.rs`, atrás da feature `http`, só nativo | `tests/http_mtls.rs` |
| Tela e registro das notas | `crates/ravenrs-app-nfe/src/`: `module`, `service`, `cupom` | O motor não conhece `ravenrs-app` |
| Regra do carrinho, sem tela | `crates/ravenrs-pos/src/` | Testes no próprio crate |
| Tela do caixa, produtos, etiquetas, entrada | `crates/ravenrs-app-pos/src/` | `examples/mini-pdv/tests/caixa.rs` |
| Recurso que não é só de caixa | `ravenrs-app`, `ravenrs-app-ui` ou `ravenrs-widgets`, nunca em `examples/mini-pdv` | Demonstração em `examples/feature-center` |
| Layout de cupom, DANFE ou etiqueta | Relatório guardado, no editor padrão (`ravenrs-report-designer`) | **Não existe outro editor de layouts** |
| Linguagem de impressora, saída | `crates/ravenrs-report-label/src/`: `language`, `raster`, `output`, `preset` | `FONTES.md`, `tests/roundtrip.rs`, `escpos.rs`, `reading.rs` |
| Provedor de pagamento | `crates/ravenrs-pagamento/src/`, atrás de `ProvedorPagamento` | Simulador em `simulador/`, `docs/PAGAMENTOS.md` |
| Código de barras, GTIN | `crates/ravenrs-barcode/src/lib.rs` | `ravenrs-graphics` só desenha |

## Fronteiras

- **O motor fiscal não tem GUI.** `ravenrs-nfe` não conhece `ravenrs-app`, e compila em `wasm32` sem a feature `http`.
- **Toda saída passa por `Transport`.** O sandbox é um `Transport`: o emissor monta os mesmos bytes e chama os mesmos endereços. Não crie caminho especial para o sandbox dentro do emissor.
- **Produção está bloqueada no código.** `Ambiente` só tem `Sandbox` e `Homologacao`, e nada escreve `tpAmb=1`. Liberar produção é decisão do dono, depois de homologar. Não abra esse caminho por conta própria.
- **O beta não some.** `BetaAck` para compilar, aviso de módulo que não fecha, `infCpl` no XML e "SEM VALOR FISCAL" no cupom.
- **Dinheiro é decimal exato.** `rust_decimal`, sem passar por `f64` na conta. Chave e CNPJ são sempre texto, porque o CNPJ vira alfanumérico.
- **Regra de um módulo não vai para `ravenrs-criteria` nem para `ravenrs-report`.** A impressora só entra pelo modelo do relatório (`Report::printer`, `ControlKind::PrinterCommand`, `Item::BarcodeMark`) e por `ravenrs-report-label`.
- **Este crate nunca pede senha de cartão nem captura cartão.** Isso fica com o software do provedor.

## Regras de fonte

Licença e confiança valem tanto quanto o código.

- As regras saem do MOC, dos XSD e das notas técnicas. O ACBr (LGPL) serve para conferir comportamento: **nunca copie código nem tabelas dele**. O mesmo vale para sped-nfe e outros.
- Toda regra nova ganha uma linha em `REGRAS.md`, dizendo o que foi de fato consultado: XSD, MOC, nota técnica, terceiro, ACBr ou memória.
- Regra de memória é marcada como memória. Nunca a apresente como confirmada.
- Texto de `cStat` vindo da nossa tabela é rótulo. O que vale é o `xMotivo` do servidor.
- Isto não é parecer jurídico nem fiscal. Regra com impacto fiscal, como a troca de NFC-e por NF-e, precisa de contador.

## Sandbox

```rust
use ravenrs_nfe::sandbox::{OnUnhandled, SefazSandbox, autorizacao, cenarios};

let server = SefazSandbox::new();
server.use_once(autorizacao(cenarios::paralisado_108));
server.on_unhandled(OnUnhandled::Warn);
let enviados = server.requests();
server.reset_handlers();
```

Funciona como o MSW: `use_handler`, `use_once`, `reset_handlers`, `on_unhandled` e `requests`. Sem handler seu, ele confere de verdade o envelope, `tpAmb=2`, a assinatura, a chave, a ordem das tags, os totais e a duplicidade. **Passar no sandbox não garante autorização real**, porque ele não aplica o Anexo I inteiro do MOC.

## Diagnóstico

Para saber por que um XML seria rejeitado, ou por que uma assinatura não confere:

- `sandbox::diagnosticar(xml)` lista todas as recusas, em vez de parar na primeira como o servidor faz. `sandbox::analisar(xml, &opcoes)` traz o resto da análise.
- `sandbox::autopsia_assinatura(xml)` reconstrói as versões que um erro conhecido teria assinado. Se uma bate, a causa está provada. Se nenhuma bate, a resposta diz que há só indícios.
- `sandbox/catalogo.rs` diz, para cada regra, a fonte e o grau de confiança.
- `sandbox/mascara.rs` tira dados pessoais do que vai para log ou para fora. Use antes de gravar ou mostrar um XML de cliente.

## Certificados

- **Laboratório.** `sandbox::AcTeste` e `GeradorCertificado` emitem certificados no formato ICP-Brasil, com a autoridade `RAVENRS SANDBOX AC - SEM VALOR`. Gerar `vencido`, `revogado`, `cadeia_quebrada`, `cnpj_diferente` e outros serve para testar a reação do sistema. `Ambiente::Homologacao` e `HttpTransport` recusam essa autoridade, e o sandbox recusa as outras.
- **Chaves em `fixtures/` são públicas de propósito** e não valem nada. `scripts/gerar-fixtures.sh` e `scripts/gerar-chaves-laboratorio.sh` as regeneram.
- **A1 real.** `CertificadoA1::from_pfx` com `SenhaA1`. A senha fica só em memória e não aparece em `Debug`. As raízes ICP-Brasil vêm por arquivo, e a verificação de TLS não tem como ser desligada. Nunca grave PFX ou senha em repositório, teste ou log.

## XSD e conferências independentes

```bash
cargo test -p ravenrs-nfe --all-features
```

```bash
crates/ravenrs-nfe/scripts/validar-xsd.sh <pasta com nfe_v4.00.xsd e os demais esquemas do PL_010>
```

Os esquemas não ficam no repositório. Em `tests/externos.rs`, se `xmllint` ou `openssl` não existir, o teste avisa e passa: leia a saída antes de dizer que a conferência rodou.

## Caixa e etiquetas

```bash
cargo test -p mini-pdv
```

```bash
RAVEN_PDV_TELAS=pasta cargo test -p mini-pdv --test telas -- --ignored
```

```bash
cargo run -p ravenrs-report-label --example preview -- etiqueta.png
```

Tudo no caixa funciona sem mouse. Ao mudar tela, olhe as imagens geradas, porque teste sem janela não vê layout quebrado. Desempenho do caixa segue a skill `ravenrs-desempenho`.

## O que só a homologação prova

Diga isto com todas as letras quando alguém perguntar se funciona.

| Só se prova com | O quê |
|---|---|
| A1 real e contribuinte credenciado | Endereços, TLS e cifras, aspas da ação SOAP, `urlChave`, regras por UF, rejeições do Anexo I, QR Code |
| Impressora física de cada marca | Se o bitmap sai certo, se a imagem carrega na L42 Pro, densidade, corte, gaveta |
| Leitor físico | Se o código impresso é lido, e se o ritmo que distingue leitor de digitação serve |
| Pinpad e provedor real | ABECS, SiTef, PayGo, Mercado Pago, Stone |
| Windows e Linux | Teclas, impressão, SQLite no caixa. Só o Alex testa: não compile para esses sistemas no Mac |
| Contador e jurídico | Troca de NFC-e por NF-e, prazos, uso do ACBr como referência, ABECS |

## O que relatar ao terminar

- O que rodou, com o resultado, e o que não rodou.
- Quais regras novas entraram em `REGRAS.md` e com qual fonte.
- O que passou só no sandbox. Nunca escreva "homologado" nem "aceito pela SEFAZ" sem resposta real de servidor.
