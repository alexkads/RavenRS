# Origem das regras do ravenrs-nfe

**BETA, prova de conceito.** Nenhuma regra aqui foi exercitada contra um servidor real da SEFAZ. "Lida na fonte" quer dizer que o texto foi lido naquele documento, não que um servidor foi visto aplicando a regra. Isto não é parecer jurídico nem fiscal.

Todo o código foi escrito do zero; nada foi copiado de ACBr, sped-nfe, Java_NFe ou fiscal-rs. O ACBr é LGPL e só serviu para conferir comportamento.

## Fontes lidas

Baixadas do Portal DFe SVRS em novembro de 2026, pela rota `/NFE/DownloadArquivoEstatico` que a própria página usa, e lidas como texto:

| Documento | Serve para |
|---|---|
| MOC 7.0, Anexo I (`moc7-anexo-i-leiaute-e-rv.pdf`, MD5 `5d25be4d45659c64ac7d060cedd8026b`) | regras de validação e tabela de cStat |
| MOC 7.0, Visão Geral | assinatura, certificado, eventos, consulta, distribuição |
| NT 2023.002 v1.02 | fim do assíncrono (129), lote de NFC-e (126), inutilização |
| NT 2025.001 v1.02 | QR Code v3, 452, Pix em 391, 865 e 391 futuras no modelo 55 |
| NT 2023.004 v1.11 | grupo `card` ampliado, 963, 796 |
| NT 2025.002 v1.53 | 1115 |
| NT 2023.003 v1.40, NT 2026.009 | CFOP de NFC-e |
| NT 2014.002 v1.40 (cópia em reformatributaria.com) | NFeDistribuicaoDFe, 137, 138, 656 |
| XSD `PL_010_V1.30` (exemplares do sped-nfe) | leiaute, validado com `xmllint` |

## Cobertura do catálogo

`sandbox::catalogo::CATALOGO` é a única lista. O `SefazSandbox` e `diagnosticar` leem dela. Um teste compara estes números com o código.

| Medida | Regras |
|---|---|
| Total | 118 |
| Lidas na fonte | 110 |
| Inferidas | 7 |
| De memória | 1 |
| Do MOC | 92 |
| De nota técnica | 17 |
| Do XSD | 4 |
| Secundária | 1 |
| Decisão do sandbox | 4 |

As inferidas e a de memória:

| Regra | cStat | Por que não é "lida" |
|---|---|---|
| `SBX-SOAP` | sem cStat | o MOC não diz o que um servidor responde a envelope errado |
| `CERT-CHAVE-FRACA` | 290 | o MOC 7.0 ainda cita 1024 bits; o limite de 2048 vem da ICP-Brasil, não conferido |
| `CERT-SHA1` | 290 | nenhuma regra do MOC cita o hash do certificado |
| `SBX-FORA-DO-LABORATORIO` | 293 | decisão do sandbox: só confia na própria AC |
| `LOCAL-PIN` | sem cStat | comportamento do `TokenSimulado`; o limite real depende do fabricante |
| `SIG-SHA256` | 298 | a Visão Geral lista só SHA-1; o código da recusa vem de F01 por inferência |
| `F02-DIGEST` | 297 | F02 fala só do `SignatureValue`; o digest cair aqui é inferido |
| `SBX-RS-CAUT` (memória, alerta) | sem cStat | o pedido cita o RS exigindo `cAut` igual ao comprovante, em atendimento.receita.rs.gov.br; a página não foi localizada |

## O que a leitura do MOC corrigiu

Havia regras de memória erradas. Corrigidas com a fonte:

- **129** não era desconhecido: é "A autorização deve ser solicitada de forma síncrona" (NT 2023.002, GAP03a-6). Foi colocado na tabela de vigências.
- **598 vale para os modelos 55 e 65** (E04-20). A NFC-e com nome de destinatário agora usa o texto fixo; antes seguia só o ACBr.
- **Segundo cancelamento é 580, não 573.** A Tabela 5-38 verifica "NF-e cancelada" (2P12-22) antes da duplicidade (3P15-10).
- **Ordem da autorização:** 252 vem depois da assinatura, e 502 (A03-10) vem antes de 253 (B23-10).
- **Tolerâncias lidas:** 629 e 630 aceitam R$ 0,01 e 610 aceita R$ 0,50 (notas \*4 e \*3 do Anexo I). O sandbox exigia o valor exato.
- **`cNF`:** B03-10 (897) tem uma lista de valores proibidos. O hash do emissor agora foge dela; antes podia cair nela.
- **865 é do modelo 65** (a NT 2025.001 deixa o 55 para o futuro), com exceção para `tPag=90`; **866** separa "sem troco" de **869** "troco errado".
- **Pix no 391:** é regra lida na NT 2025.001, e não contradiz o 963 lido na NT 2023.004: o 17 está entre os que *aceitam* `card`. Um artigo secundário lia isso ao contrário.
- **Prazo de cancelamento:** 24 horas (2P12-14), com 501. No modelo 55 a UF pode responder 155.

Regras novas que a leitura trouxe: 126, 207, 239, 271, 280 a 286, 290 a 296, 516, 517, 565, 574, 577 a 580, 587, 630, 703, 704, 707, 709, 710, 715 a 717, 719, 724 a 726, 789, 806, 866, 897, 899, 904.

## Decisões sem fonte, que continuam marcadas

- **Arredondamento:** o Anexo I manda arredondar a duas casas, mas não diz o que fazer na metade. Aqui é metade para longe de zero.
- **Modelo 55 com PIS/COFINS `Outr` CST 99, omitidos no 65:** passam no XSD. A regra por CRT e modelo não foi conferida.
- **`modFrete=9`, `indFinal=1`, `indPres=1`, `indIEDest=9`:** passam no XSD; escolhas da POC.
- **`urlChave` por UF:** sem tabela, valores de memória. Um valor errado leva a 878 (ZX03-20, opcional por UF). `EmissorBuilder::url_consulta` substitui.
- **`cOrgao=91` na manifestação:** de memória, sem texto lido.
- **`nfeDistDFeInteresse` envolvendo `nfeDadosMsg`:** de memória do WSDL, que não foi lido.
- **`consChNFe` de chave inexistente:** a NT 2014.002 diz 137 no texto e 217 na regra H142. O sandbox responde 137.
- **Aspas na ação do `Content-Type`:** RFC 2045; sem teste em servidor real.
- **Rejeição 8xx do hash do QR v2:** a montagem do hash vem do leiaute do parâmetro na NT 2025.001; não há teste contra servidor.

## Conflitos conhecidos

- O `Id` do evento no XSD é `ID[0-9]{52}`, que não aceita chave com CNPJ alfanumérico. Até o esquema mudar, cancelar ou manifestar nota com CNPJ alfanumérico não passa no XSD. A NT 2026.004 trata do CNPJ alfanumérico e não foi aprofundada neste ponto.
- O MOC 7.0 é de 2020 e as NT o alteram. Para o texto de uma regra, a última NT lida prevalece e a fonte diz qual é.

## Certificados de laboratório

| Regra | Origem | Confiança |
|---|---|---|
| `otherName` 2.16.76.1.3.3 (CNPJ) e 2.16.76.1.3.1 (CPF) | MOC, Visão Geral 4.2.3, e regras A07, E03 | lida |
| 2.16.76.1.3.2 (nome do responsável), 2.16.76.1.3.4 (dados do responsável), 2.16.76.1.3.7 (CEI) | Resoluções CG ICP-Brasil 128, 138 e 211, vistas em resultado de busca | inferida. **O texto consolidado da DOC-ICP-04 não abriu**, e uma versão antiga da DOC-ICP-04.01 dava outro conteúdo ao `.2` |
| CN como `RAZAO:CNPJ` | prática de mercado | de memória |
| Autenticação Cliente | MOC, Visão Geral 4.2.3, e A01 | lida |
| Assinatura Digital e Não Recusa | MOC, E01 | lida |

Sem nome de AC real nem ICP-Brasil como emissora: tudo sai de `RAVENRS SANDBOX AC - SEM VALOR`. O gerador é DER escrito à mão sobre o crate `rsa`, sem dependência nova, porque as falhas pedem o que um gerador recusa fazer (SHA-1, 1024 bits, cadeia quebrada) e porque continua compilando em wasm. O PFX fica na feature `http`.

Token real exige PKCS#11, com código externo, proibido neste workspace. O contrato do processo auxiliar está no rustdoc de `sandbox::certificados`, sem implementação.

## Vigências

`sandbox::vigencia::VIGENCIAS`:

| Linha | Homologação | Produção | Fonte |
|---|---|---|---|
| Assíncrono dá 452 | sem data lida | 13/10/2025 | NT 2025.001 |
| Assíncrono extinto, 129 | 01/02/2027 | 05/04/2027 | NT 2023.002 v1.02 |
| Inutilização fora em AC, BA, SP | 11/11/2026 | 11/11/2026 | NT 2023.002 v1.02 |
| CRT 3 sem IBSCBS, 1115 | emissão a partir de 01/07/2026 | sem data | NT 2025.002 v1.53 |
| `urlChave`, 878 | 01/04/2019 | 01/04/2019 | Anexo I, ZX03-20 |

## Distribuição e entrada de mercadoria

| Regra | Origem |
|---|---|
| `distNSU`, `consNSU`, `consChNFe`, lote de até 50, GZip e base64, últimos 90 dias | NT 2014.002 v1.40 |
| 137, e espera de 1 hora; 656 com bloqueio de 1 hora; 656 por NSU anterior | NT 2014.002 v1.40, item 3.11.4 |
| `resNFe` até haver manifestação, salvo desconhecimento | NT 2014.002 v1.40, item 3 e fluxo do item 7 |
| 210200, 210210, 210220, 210240; `descEvento`; `xJust` na 210240 | XSD `leiauteConfRecebto_v1.00.xsd`, MOC Visão Geral 5.11 |
| 575, 595 | Anexo I, tabela; Visão Geral 5.11.3 |
| Endereços do Ambiente Nacional | entrada `AN` da tabela do sped-nfe. **Nenhum foi chamado** |
| Limite de 10 MB no XML e 20 MB no GZip | decisão desta POC |

## Pendente

- `Signer` ainda devolve `NfeError`: mudar agora quebraria o `ravenrs-app-nfe`. Separar fica para o crate comum com a NFS-e. SHA-256 já existe em `sign_rsa`.
- Geração real de chave RSA (`AcTeste::gerar`) tem teste, mas ignorado por ser lento em depuração, e não foi rodado.
- Texto consolidado da DOC-ICP-04 e a página do RS sobre `cAut`: não localizados.
- `consNSU` não tem método no emissor. `procEventoNFe` não entra na distribuição do sandbox nem na consulta de nota cancelada.

## O sandbox não é a SEFAZ

Passar no sandbox não garante autorização. O catálogo não cobre cadastro de contribuintes, tabela de NCM e municípios, IE e todas as regras do Anexo I: são centenas, e aqui estão as que esta POC emite ou recebe. `Incidente` diz quando um código não está no catálogo, em vez de inventar causa.

## O que só um teste real comprova

Homologação de verdade exige certificado A1 e contribuinte credenciado. Até lá ficam sem prova: endereços, TLS (o `rustls` não oferece troca de chaves RSA nem cifras antigas), aspas da ação, `urlChave`, o envelope da distribuição e o comportamento real de cada código.
