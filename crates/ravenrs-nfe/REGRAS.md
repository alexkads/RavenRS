# Origem das regras do ravenrs-nfe

**BETA, prova de conceito.** Nenhuma regra aqui foi exercitada contra um servidor real da SEFAZ. Isto não é parecer jurídico nem fiscal.

O portal da Fazenda e o MOC não foram lidos nesta POC. Por isso, a coluna "Origem" diz o que de fato foi consultado:

- **XSD**: lido em `PL_010_V1.30` (exemplares do sped-nfe) e validado por `scripts/validar-xsd.sh`.
- **W3C**: recomendação pública, conferida com `xmllint` e `openssl`.
- **Terceiro**: página de terceiro que comenta a regra; não substitui o MOC.
- **ACBr**: comportamento lido no espelho `joaoduarte19/ACBr`, sem copiar código nem tabelas (o ACBr é LGPL).
- **Memória**: lembrança do autor, sem fonte conferida nesta POC. **Confirmar no MOC antes de confiar.**

Todo o código foi escrito do zero; nada foi copiado de ACBr, sped-nfe, Java_NFe ou fiscal-rs.

## Documento

| Regra | Origem |
|---|---|
| Ordem das tags, formatos, tamanhos e padrões de `infNFe`, `infNFeSupl`, `Signature` | XSD `leiauteNFe_v4.00.xsd` |
| `Id="NFe"+chave`, com `[0-9]{6}[0-9A-Z]{12}[0-9]{26}` (já alfanumérico) | XSD |
| Composição da chave de 44 posições | XSD (padrão do `Id`); conferida pelo sandbox |
| `cDV` em módulo 11, pesos 2 a 9 | Memória; o valor de cada caractere é o código ASCII menos 48, como no CNPJ alfanumérico |
| DV do CNPJ alfanumérico; exemplo `12ABC34501DE35` | Memória (publicação da Receita Federal); passa no teste |
| `cNF` diferente de `nNF` | Memória |
| `cNF` derivado por hash do documento, sem fonte de aleatoriedade | Decisão desta POC, para ser determinístico e funcionar no wasm; não foi conferida a regra de "não sequencial" |
| Texto `NF-E EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL` em `dest/xNome`, no modelo 55 | Plano; ACBr aplica só no 55 |
| Texto `NOTA FISCAL EMITIDA EM AMBIENTE DE HOMOLOGACAO - SEM VALOR FISCAL` no `xProd` do item 1, no modelo 65 | Plano; ACBr; rejeição 373 em Terceiro (Oobj) |
| Na NFC-e com destinatário, `xNome` fica como informado | ACBr. **Sem conferência no MOC**: a SEFAZ pode exigir o texto também aqui |
| PIS e COFINS: `PISOutr`/`COFINSOutr` CST 99 zerados no 55, omitidos no 65 | Só XSD (os grupos são opcionais). **A regra por CRT e modelo não foi conferida** |
| `modFrete=9`, `indFinal=1`, `indPres=1`, `tpImp` 4 no 65 e 1 no 55, `indIEDest=9` | Memória; passam no XSD |
| `card/tpIntegra=2` em cartão e Pix | Memória; passa no XSD |
| `infRespTec` opcional | XSD. **Quais UFs exigem não foi conferido** |
| Sem `vTotTrib` (Lei 12.741) | Lacuna declarada |
| Sem grupo `IBSCBS` | Decisão do plano: só CRT 1; rejeição 1115 em Terceiro (buscadorncm). **A obrigatoriedade para CRT 1 a partir de 2027 não foi conferida** |

## Arredondamento e totais

Regra adotada, em `src/totais.rs`:

- `vProd` do item é `qCom * vUnCom`, arredondado a 2 casas, com a metade para longe de zero.
- `vProd` e `vDesc` totais são a soma dos itens.
- `vNF` é `vProd - vDesc`, porque frete, seguro, outros, ST, IPI e II não existem na POC.
- `vTroco` é o que foi pago acima de `vNF`; pagar menos é recusado.
- Valor com mais casas do que o leiaute permite é recusado, nunca arredondado em silêncio.

**Sem fonte:** o modo de arredondamento e a tolerância da SEFAZ nas rejeições 629 e 610. Terceiros divergem: um cita R$ 0,50 na 610 e outros falam em centavo. O sandbox é mais rígido e exige o valor exato. A fórmula de `vNF` usada pelo sandbox (com `vICMSDeson`, `vST`, `vFCPST`, `vFrete`, `vSeg`, `vOutro`, `vII`, `vIPI`, `vIPIDevol`) vem de Terceiro e pode estar incompleta para outros regimes.

## QR Code da NFC-e

Implementados os dois modos online, com a **versão 3 como padrão**:

- v3: `URL?p=<chave>|3|<tpAmb>`, sem CSC.
- v2: `URL?p=<chave>|2|<tpAmb>|<cIdToken>|<hash>`, com hash SHA-1 de `<chave>|2|<tpAmb>|<cIdToken>` seguido do CSC.

Origem: padrões no XSD (`qrCode`), com comentário "QRCODE V3 ONLINE", e NT 2025.001 por Terceiro. O PDF da NT foi baixado, mas não pôde ser lido aqui, então a montagem do hash da v2 e a data de fim do CSC ficaram **sem conferência no texto da nota**. Terceiro cita a rejeição 444 (v2 proibido para pessoa física).

`urlChave` por UF: **sem tabela**, valores de memória. Um valor errado leva à rejeição 878 no servidor real. `EmissorBuilder::url_consulta` substitui.

## Assinatura e C14N

| Regra | Origem |
|---|---|
| C14N inclusiva 1.0 sem comentários | W3C `REC-xml-c14n-20010315`; comparada com `xmllint --c14n` |
| XMLDSig enveloped, RSA-SHA1, SHA-1, ordem de `Signature`, sem `KeyValue` | W3C; XSD `xmldsig-core-schema_v1.01.xsd` |
| `Signature` por último, depois de `infNFeSupl` | XSD |
| Digest e assinatura | Verificados com `openssl dgst -sha1` e `-verify`, sobre XML canonizado pelo `xmllint` |
| A SEFAZ aceita RSA-SHA1 e C14N sem `xmlns` do envelope | Memória: o sandbox confere o `NFe` isolado, sem herdar o prefixo do SOAP |

O crate `rsa` tem o aviso RUSTSEC-2023-0071, sem correção publicada. Fica ignorado em `deny.toml` porque aqui só se assina localmente e nada é decifrado. Quem levar a assinatura para um servidor precisa rever isso.

## SOAP e endereços

| Regra | Origem |
|---|---|
| SOAP 1.2, `nfeDadosMsg`, `nfeResultMsg`, namespace `.../nfe/wsdl/<Serviço>` | Plano; tabela do sped-nfe (`operation`, `method`) |
| Ação em `Content-Type: application/soap+xml;charset=utf-8;action="..."` | Plano; aspas por causa da RFC 2045 (o valor tem `/` e `:`). **Sem teste em servidor real** |
| Só `indSinc=1`; a rejeição 452 recusa assíncrono com uma nota | Terceiro (fórum do ACBr sobre a NT 2025.001) |
| Endereços de homologação | Tabela `storage/wsnfe_4.00_mod55.xml` e `mod65.xml` do sped-nfe, escritos à mão aqui. **Nenhum foi chamado** |
| SC, RJ, ES e DF no SVRS | Dedução: as tabelas não dão endereço próprio a elas. **Lista oficial de UFs do SVRS não lida** |
| Evento 110111: leiaute | XSD `leiauteEventoCancNFe_v1.00.xsd` |

**Conflito conhecido:** o `Id` do evento no XSD é `ID[0-9]{52}`, que não aceita chave com CNPJ alfanumérico. Até o esquema mudar, o cancelamento de nota com CNPJ alfanumérico não passa no XSD; o sandbox aceita.

**Risco de TLS:** o `rustls` não oferece troca de chaves RSA nem cifras antigas. Se algum servidor só falar essas, a conexão falha. Só um teste real com A1 responde.

## Códigos de retorno

Os textos de `descricao_c_stat` são rótulos nossos, de memória. O que vale é o `xMotivo` do servidor.

| Código | Uso no sandbox | Origem |
|---|---|---|
| 100, 104, 107, 108, 109, 128, 135, 204, 225, 297, 298, 539, 598 | significado corrente | Memória; 128 e 135 também em Terceiro |
| 101, 217, 222, 236, 243, 252, 253, 373, 394, 494, 502, 537, 564, 573, 588, 610, 629, 865, 869 | escolhidos para cada falha conferida | Memória; 373 e 610 também em Terceiro. **Conferir no Anexo I do MOC** |
| 452 | lote assíncrono | Terceiro |
| 1115 | CRT 3 sem `IBSCBS`, e cenário pronto | Terceiro |
| **129** | só cenário pronto, sem significado atribuído | **Sem fonte.** Consta do plano, mas nenhuma fonte consultada define o código. Serve para ver como o app reage a um código desconhecido |

O sandbox não confere pela tabela do MOC: confere os itens listados no README. Passar nele não garante autorização real.

## O que só um teste real comprova

Homologação de verdade exige certificado A1 e contribuinte credenciado. Até lá, nada aqui além dos itens marcados como validados é garantido: endereços, TLS, aspas da ação, `urlChave`, regras de PIS/COFINS e os códigos de retorno.
