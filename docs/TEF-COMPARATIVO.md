# TEF para o PDV: comparativo e recomendação

Pesquisa de 2026-11-05, encerrada antes do fim por prazo. A seção 10 lista o que não deu tempo de confirmar.

Marcação: **[C]** li na fonte, **[S]** fonte secundária ou só trecho de busca, **[L]** lembrança. Fórum e blog de fornecedor são [S] mesmo quando abertos. Nenhum preço abaixo é proposta.

## Resumo

1. Contratar primeiro a **PayGo, por troca de arquivos no Windows**: é o único caminho achado que junta documentação pública, sem FFI, pinpad USB, multiadquirência e prazo de análise publicado.
2. O lojista é quem contrata a PayGo; a software house faz a integração e recebe um certificado. O preço não é público, e o valor antigo não serve de base.
3. Ponto fraco: os arquivos da PayGo não trazem o CNPJ da credenciadora, e a documentação pede para não validar rede e bandeira. Precisamos de resposta da PayGo antes de montar `CNPJ` e `tBand`.
4. O TEF Express é o plano B direto: usa o mesmo tipo de arquivo e devolve o CNPJ da rede, mas o preço depende de contato e a documentação só cita Windows.
5. Linux não muda a decisão agora: o caminho por arquivo é Windows. macOS não apareceu em nenhum provedor.
6. Mercado Pago Point fica só para POC: a Orders API não mostra `cAut`. `AUTHORIZATION_CODE` e bandeira aparecem no relatório, e `authorization_code` na API Payments, sem prova de que batem com o comprovante.
7. Pix direto por PSP é viável na POC e cabe na NFC-e (`tPag=17`, `tpIntegra=1`, `cAut` = endToEndId), mas não resolve cartão.
8. RS é o risco concreto: multa de R$ 8.497,92 por equipamento ao mês, com autuações em 2026. Vender lá sem integração não é opção.
9. Para SP, PR, MG, RJ, ES e DF não achei norma oficial que exija integração, e os blogs se contradizem. SC tem outra barreira: homologação do PAF-NFC-e com laudo.
10. Acionar: chamado PayGo, tabela do TEF Express e proposta do SiTef Express. O cronograma fiscal de 16/11/2026 (NT 2026.006) não obriga nada em 2026.

## 1. Modelo comercial

| Provedor | Quem contrata | Preço | Adquirentes | Parceiros e homologação |
|---|---|---|---|---|
| **PayGo** (grupo C6, técnica Setis) | Lojista é o "Cliente" do contrato. O anexo TEF pede automação certificada no checkout. [C](https://openfiles.paygo.com.br/site/files/PayGo%20-%20Contrato%20Plataforma%20PayGo%20TEF_Pu%CC%81blico.pdf) | Adesão e mensalidade existem, sem valor no contrato, com correção anual por IGP-M; a credenciadora cobra à parte taxa de conectividade por número lógico. [C](https://openfiles.paygo.com.br/site/files/PayGo%20-%20Contrato%20Plataforma%20PayGo%20TEF_Pu%CC%81blico.pdf) Relatos antigos: adesão R$ 650, mensal R$ 135, VPN R$ 95 (2015). [S antigo](https://www.projetoacbr.com.br/forum/topic/20071-duvida-custos-tef-paygo/) | Lista dinâmica com Cielo, Rede, Stone, Getnet, Mercado Pago, PagBank, Safrapay, Sipag, BIN, C6 Pay e outras. [C](https://paygodev.readme.io/docs/redes-adquirentes-carteiras-digitais-e-pix-no-paygo) | Portal de parceiros citado na home; caminho comercial em [paygo.com.br/contato](https://paygo.com.br/contato/). [C](https://paygodev.readme.io/docs/ambiente-de-testes-sandbox) Taxa de certificação atual não publicada; R$ 750 e R$ 450 ou 550 na recertificação em 2014. [S antigo](https://regys.com.br/homologacao-tef-paygo/) |
| **SiTef / Fiserv** | Página cita "parceiro que contrata"; contrato com o gerente comercial. [C](https://www.softwareexpress.com.br/pt/ajuda/) [C](https://www.softwareexpress.com.br/pt/sitef-express/) Relato de 2015: contrato fixo pago pela software house. [S antigo](https://www.projetoacbr.com.br/forum/topic/20071-duvida-custos-tef-paygo/) | Sem preço oficial. Revenda anuncia R$ 135, 167 e 199 para 1, 2 e 3 PDVs, sem data. [S](https://www.ethamex.com.br/tef-software-express-sitef-web-mensalidade-para-1-ponto-de-venda) | Multiadquirente; Stone é módulo. [S](https://ajuda.stone.com.br/tef/o-que-e-tef) | SiTef Express: até 30 lojas ou 300 PDVs; licença local não é mais vendida. [C](https://www.softwareexpress.com.br/pt/sitef-express/) "Programa Parceiros de Valor" existe, sem detalhe. [C](https://www.fiserv.com.br/solucoes/sitef/) ACBr se declara distribuidor. [S](https://www.projetoacbr.com.br/forum/topic/92359-sitef-como-iniciar-a-homologa%C3%A7%C3%A3o-por-troca-de-arquivos-pdvclisitef) |
| **Stone TEF** | CNPJ com faturamento acima de R$ 40 mil ao mês; a integradora detém a licença. [C](https://ajuda.stone.com.br/tef/o-que-e-tef) | Não publicado. | Multiadquirente por TEF house: D-TEF, SiTef, VMPay, V$-Pague, CTF, Scope, PayGo, Phoebus, Aditum. [C](https://ajuda.stone.com.br/tef/o-que-e-tef) | Não é um produto para integrar diretamente: entra por uma TEF house. |
| **AutoTEF Slim** | Parceiro Stone mais Stone Code do lojista. [C](https://autotef.readme.io/reference/processo-de-integra%C3%A7%C3%A3o) | Não publicado. Não confirmei se o piso de R$ 40 mil vale aqui. | **Só Stone**: autorizador Stone. [C](https://autotef.readme.io/reference/introdu%C3%A7%C3%A3o) | Partner Program, souparceiro@stone.com.br. [C](https://autotef.readme.io/reference/processo-de-integra%C3%A7%C3%A3o) |
| **Mercado Pago Point** | Lojista com conta vendedor. [C](https://www.mercadopago.com.br/developers/pt/docs/mp-point/overview) | Taxa por transação; mensalidade não consta. Valor não lido. | Só Mercado Pago. [L] | Programa de parcerias no rodapé do portal. [C](https://www.mercadopago.com.br/developers/pt/reference/in-person-payments/point/orders/get-order/get) |
| **TEF Express** (Multiplus Card) | Software house como parceira, em marca própria; lojista usa suas adquirentes. [C](https://tefexpress.com.br/) | Tabela especial para software house, só por contato. [C](https://tefexpress.com.br/) | "100% das operadoras", com cinco TEF houses atrás de uma integração. Alegação do fornecedor. [C](https://tefexpress.com.br/) | Portal do Representante, grupo de homologação, receita recorrente possível. [C](https://tefexpress.com.br/) |
| **Cappta** | Descontinuar: Linx assumiu em 08/2023. [C](https://share.linx.com.br/display/DTEF/Comunicado+ao+cliente+Cappta) Novas instalações bloqueadas segundo um sistema parceiro. [S](http://evohelp.w12app.com.br/pt-BR/articles/2425396-integracao-evo-e-cappta) | | | Linx foi vendida à TOTVS, concluído em 27/02/2026. [S](https://www.barchart.com/story/news/478731/stoneco-announces-successful-closing-of-linx-sale) |
| **Elgin TEF Hub** | Página sem detalhe; dado estruturado de R$ 1.000 sem contexto. [S](https://www.elgin.com.br/tef-hub/p) Não usar esse número. | | | SAC 0800 70 35 446. [C](https://www.elgin.com.br/tef-hub/p) |
| **Scope** (Conexão Tech) | Só confirmado como TEF house aceita pela Stone e pela Alterdata. [C](https://ajuda.stone.com.br/tef/o-que-e-tef) [C](https://ajuda.alterdata.com.br/pdvbase/tef-como-funciona-239353876.html) Sem preço. | | | |
| **ConnectTEF** (Smart POS) | Para software houses e revendas; portal de parceiro. [C](https://connecttef.com.br/) Sem preço. | | Cielo, Stone, Rede, Getnet, Mercado Pago e outras. [C](https://connecttef.com.br/) | Mais de 270 software houses, alegação do fornecedor. [C](https://connecttef.com.br/) |

**Fidelidade PayGo:** cancelamento pode ser pedido a qualquer momento (anexo II, 5.5), mas a taxa de conectividade da credenciadora não cessa por isso (5.5.1). A instalação precisa terminar em 30 dias, ou os números lógicos caem; um novo número leva até 10 dias úteis. [C](https://openfiles.paygo.com.br/site/files/PayGo%20-%20Contrato%20Plataforma%20PayGo%20TEF_Pu%CC%81blico.pdf) Não li as 44 páginas por inteiro.

**Mercado:** faixa de R$ 100 a R$ 250 por PDV ao mês citada por fornecedor, não verificada. [S](https://blog.modopag.com.br/pagamentos/diferenca-entre-pos-e-tef-integrado/)

## 2. Passo a passo de homologação

**PayGo** [C](https://paygodev.readme.io/docs/passo-a-passo-homologa%C3%A7%C3%A3o)
1. Baixar o kit e o roteiro (v20261030). [C](https://paygodev.readme.io/docs/downloads)
2. Abrir chamado no [Jira da Setis](https://dev.proj.setis.com.br/servicedesk/customer/portal/16) escolhendo a solução; recebem-se as credenciais.
3. Testar no sandbox com C6 Pay, Rede e Pix C6 Bank simulados. O Windows entra em modo `demo` e pede CPF ou CNPJ, ID de instalação e senha. [C](https://paygodev.readme.io/docs/kit-de-integracao-troca-arquivos)
4. Rodar o roteiro e enviar evidências e logs no chamado.
5. Resposta em 5 dias úteis; aprovado, a PayGo emite o certificado no Jira. O campo `738-000` carrega o registro de certificação. [C](https://paygodev.readme.io/docs/detalhamento-dos-campos)
6. **Produção:** o lojista precisa de afiliação com cada adquirente e ponto de captura configurado. [C](https://paygodev.readme.io/docs/paygo-windows-1)

Pinpad: "qualquer PIN-Pad certificado", com PPC-930, 940, 950, Lane 3000 e 3600, IPP320, Q25 e P200 entre os principais; não há homologação por modelo descrita. [C](https://paygodev.readme.io/docs/principais-pin-pads-utilizados) **Versão nova do PDV:** a página atual não diz. Em 2014 o certificado valia um ano ou até alterar o código. [S antigo](https://regys.com.br/homologacao-tef-paygo/) Perguntar no chamado.

**SiTef:** documentação, conta no portal, certificação, produção. [C](https://www.softwareexpress.com.br/pt/ajuda/) Testes com SitDemo e DLLs simuladas; as de produção não funcionam em desenvolvimento. [S](https://www.projetoacbr.com.br/forum/topic/87428-sitef-como-iniciar-a-homologa%C3%A7%C3%A3o-por-dll-clisitef/) Relato de homologação em menos de um dia, de 2017. [S antigo](https://www.projetoacbr.com.br/forum/topic/40154-tef-com-software-express/)

**AutoTEF:** Partner Program, credencial, Stone Code de teste, SAK e pinpad debug, roteiro, validação pela Stone, credencial e DLLs de produção. [C](https://autotef.readme.io/reference/processo-de-integra%C3%A7%C3%A3o) Pinpads certificados: Lane 3000 e 3600, PPC930 e 950, P200, IPP320, Q25, PPP100, com ABECS 2.12. [C](https://autotef.readme.io/reference/dispositivos-suportados)

**TEF Express:** ambiente de testes, simulação e apoio; o guia de instalação fica no Portal do Representante. [C](https://tefexpress.com.br/) [C](https://docs.multipluscard.com.br/pt/documentacao/tef/webservice/configuracao/) Quem emite certificado e custo: não publicado.

**Mercado Pago:** sem homologação comercial encontrada. O dispositivo virtual consta só do conhecimento anterior; a visão geral lida não o cita. [C](https://www.mercadopago.com.br/developers/pt/docs/mp-point/overview)

## 3. Sistema operacional

| Provedor | Windows | Linux | Android | macOS |
|---|---|---|---|---|
| PayGo | Sim, arquivo, DLL, API. [C](https://paygodev.readme.io/) | Fora da lista oficial. Existe exemplo com `PGWebLib.so`, Ubuntu 20.04 e OpenSSL 1.1.1. [C](https://github.com/PGPagamentos/pdvLinuxPayGoLibC_C) Suporte atual não confirmado | Sim. [C](https://paygodev.readme.io/) | Não achei |
| SiTef | CliSiTef e PDVCliSiTef. [S](https://www.projetoacbr.com.br/forum/topic/92359-sitef-como-iniciar-a-homologa%C3%A7%C3%A3o-por-troca-de-arquivos-pdvclisitef) | CliSiTef, só biblioteca. [S](https://dev.softwareexpress.com.br/en/docs/clisitef/clisitef_documento_principal/) | m-SiTef. [S](https://www.projetoacbr.com.br/forum/topic/87613-sitef-guia-para-integra%C3%A7%C3%A3o-com-m-sitef-no-android/) | Não achei |
| AutoTEF Slim | Sim. [C](https://autotef.readme.io/reference/instalando-o-slim) | Ubuntu 20.04 e 22.04, "não pode ser diferente"; 8 GB de RAM. [C](https://autotef.readme.io/reference/introdu%C3%A7%C3%A3o) | Não achei | Não achei |
| TEF Express | Windows 7 SP1 ou superior. [C](https://docs.multipluscard.com.br/pt/documentacao/tef/webservice/introducao/) | Não informado | Smart POS. [C](https://tefexpress.com.br/) | Não achei |
| Mercado Pago, ConnectTEF | Por HTTP, indiferente ao SO do PDV (inferência) | Idem | Idem | Idem |

**Supermercado PME:** não achei dado de participação de mercado. O VR Super aceita Windows 10 x64 ou Linux x64. [S](https://docs.vrsoft.com.br/vrsuper/outros/pdv-checkout/pre-requisitos-do-pdv) **Decisão:** começar por Rust em Windows. Linux fica para SiTef, AutoTEF ou Smart POS. Atenção à máquina: a PayGo pede 8 GB no Windows 10 e 16 GB no Windows 11, o que pode barrar caixa antigo. [C](https://paygodev.readme.io/docs/paygo-windows-1)

## 4. Sem `unsafe` no workspace

- **PayGo:** arquivos em `C:\PAYGO\Req` e `Resp`, com a PayGo apagando o que lê em `Req` e a automação o que lê em `Resp`. [C](https://paygodev.readme.io/docs/diret%C3%B3rios-para-troca-de-arquivos) O `ravenrs-pagamento` já tem arquivos IntPos, testados só contra simulador. [C](AUTOMACAO-COMERCIAL.md) Alternativa ControlPay: HTTP, mas o kit também instala o PayGo Windows, ligado a um serviço de filas. [C](https://paygodev.readme.io/docs/kit-de-integracao-api)
- **SiTef:** `PDVCliSiTef` por arquivo em `C:\Client\Req` e `Resp`, só Windows. [S](https://www.projetoacbr.com.br/forum/topic/92359-sitef-como-iniciar-a-homologa%C3%A7%C3%A3o-por-troca-de-arquivos-pdvclisitef) No Linux, só processo auxiliar sobre a biblioteca.
- **TEF Stone:** `C:\TEF_DIAL`, layout publicado, com `Ativo.001`. [C](https://tefdoc.stone.com.br/reference/layout-de-mensagens)
- **AutoTEF Slim:** HTTP local em `localhost:8000`; serviço .NET. [C](https://autotef.readme.io/reference/instalando-o-slim) **Vivo?** Parcialmente: `autotef.readme.io` abre, a página de integração foi atualizada há cerca de um mês, mas changelog e requisitos têm mais de um ano; a última release lista Pix novo e .NET 8, enquanto os requisitos ainda pedem .NET 6. [C](https://autotef.readme.io/reference/changelog-autotef-slim) `microtefdocs.stone.com.br` deu erro 530 hoje. Em 2025 ainda havia adoção. [S](https://www.projetoacbr.com.br/forum/topic/85469-autotef-stone-pegar-codigo-autoriza%C3%A7%C3%A3o-pix/)
- **TEF Express:** `C:\TEF_DIAL` com `IntPos.001`, ou HTTP em `api.multipluscard.com.br` com gerenciador no caixa. [C](https://docs.multipluscard.com.br/pt/documentacao/tef/tefdial/exemplocomunicacao/) [C](https://docs.multipluscard.com.br/pt/documentacao/tef/webservice/comunicacao/)
- **Cappta:** não entrar. Produto descontinuado [C](https://share.linx.com.br/display/DTEF/Comunicado+ao+cliente+Cappta), e `docs.desktop.cappta.com.br` não resolveu hoje.

**Encapsular a DLL em processo auxiliar:** no contrato PayGo não achei proibição expressa. Há vedação de engenharia reversa, de instalar por iniciativa própria sem autorização (II, 3.1.1) e de comercializar o software sem autorização (II, 3.1); para software de equipamento, também de "qualquer conversão". [C](https://openfiles.paygo.com.br/site/files/PayGo%20-%20Contrato%20Plataforma%20PayGo%20TEF_Pu%CC%81blico.pdf) Isso não é parecer jurídico. A cláusula 3.1.1 pede atenção também ao nosso instalador. Contrato do SiTef não é público. Com arquivo, a pergunta deixa de existir.

## 5. Pix

| Caminho | Como funciona |
|---|---|
| PayGo | Carteira digital, campo `749-000` (ID 8 por arquivo), com bancos C6, Itaú, Cielo, Sicoob, Sipag, Sicredi. [S](https://www.projetoacbr.com.br/forum/topic/85945-ativa%C3%A7%C3%A3o-e-utiliza%C3%A7%C3%A3o-do-pix-e-carteiras-digitais-no-tef-paygo/) O endToEndId volta no `012-000`; `013-000` não vale para Pix. [C](https://paygodev.readme.io/docs/detalhamento-dos-campos) |
| SiTef | Lojista obtém credencial do PSP e registra na plataforma. [C](https://www.softwareexpress.com.br/pt/blog/integracao-pix-e-sitef---ampliando-a-oferta-de-pagamentos-no-var) |
| AutoTEF | Exige conta Stone e chave (`hasBankStone`, `hasPixKey`). [C](https://autotef.readme.io/reference/pix) |
| TEF Express | No PDV, depende da adquirente. [C](https://tefexpress.com.br/) |
| Mercado Pago | Exige chave Pix cadastrada. [C](https://www.mercadopago.com.br/developers/pt/docs/mp-point/overview) |

**Pix direto, para a POC:** API do Banco Central com `cob`, `pix` e `webhook`, OAuth, mTLS e `txid` de 26 a 35 caracteres. [C](https://bacen.github.io/pix-api/) Cada PSP varia em URLs e escopos. [S](https://www.voidr.co/en/resources/playbooks/pix-api-psp)

**O que a NFC-e pede no Pix dinâmico:** `tPag=17`, `vPag`, `tpIntegra=1`, CNPJ da instituição de pagamento, `cAut` com endToEndId, `CNPJReceb`, `idTermPag`. [S](https://inventti.com.br/sefaz-mt-nf-e-nfc-e-vinculo-do-comprovante-de-pagamento-eletronico-perguntas-e-respostas/) Pix estático é `tPag=20`, sem grupo `card`. [C](https://www.sefaz.pb.gov.br/announcements/16593-sefaz-pb-alerta-contribuintes-para-novas-validacoes-de-pagamento-integrado-nfc-e-e-nf-e-cartoes-e-pix) Falta confirmar na fonte se `idTermPag` é exigido nas UFs do plano.

## 6. Mercado Pago: `cAut` e `tBand`

- **Orders API:** o exemplo traz `id`, `reference_id`, `status`, `payment_method {type, installments, id}` e `card {first_digits, last_digits}`. Nenhum campo de autorização ou NSU, mesmo com a página expandida. [C](https://www.mercadopago.com.br/developers/pt/reference/in-person-payments/point/orders/get-order/get)
- **API Payments:** a referência expandida lista `authorization_code`. Não li a descrição, e um relato da comunidade o mostra nulo. [C](https://www.mercadopago.com.br/developers/en/reference/online-payments/checkout-pro-preferences/get-payment/get) [S](https://github.com/BlackBoxVision/react-native-mercadopago-px/issues/49) Os IDs da Order (`PAY01...`) têm outro formato; a ligação com `/v1/payments` não está documentada no que li.
- **Relatório de dinheiro em conta, seção Point:** tem `AUTHORIZATION_CODE`, `FRANCHISE` e `POI_ID`; NSU não aparece. [C](https://www.mercadopago.com.br/developers/pt/docs/mp-point/resources/reports/account-money/report-fields) Relatório é posterior à venda, e a NFC-e precisa do dado na hora (inferência).
- **`tBand`:** derivável de `payment_method.id` (`master`) por tabela nossa. [C](https://www.mercadopago.com.br/developers/pt/reference/in-person-payments/point/orders/get-order/get)
- **CNPJ:** 10.573.521/0001-91 no rodapé do portal; falta confirmar que é o CNPJ certo para o grupo `card`. [C](https://www.mercadopago.com.br/developers/pt/reference/in-person-payments/point/orders/get-order/get)
- **Comprovante e fórum oficial:** nada conclusivo. Teste que resolve: pagar uma venda real, ler a Order, `/v1/payments` e o comprovante, e comparar.
- **Alternativa:** a PayGo lista Mercado Pago como rede. [C](https://paygodev.readme.io/docs/redes-adquirentes-carteiras-digitais-e-pix-no-paygo)

## 7. Exigências fiscais

**Regras nacionais.** Não consegui abrir a NT 2025.001 oficial (redirecionamento em excesso e navegação negada), então isto é secundário:
- **392:** com cartão integrado, exige CNPJ da credenciadora e `cAut`, desde 08/09/2025; 437 valida o CNPJ e 443 a bandeira. [S](https://inventti.com.br/nf-e-nfc-e-nt-2025-001-versao-1-02-traz-prorrogacao-e-ajustes-nas-regras-de-validacao/) Outra fonte diz que a 392 é opcional por UF. [S](https://oobj.com.br/bc/rejeicao-392-como-resolver/)
- **391:** falta do grupo `card`; desativada para o modelo 55, sem informação para o 65. [S](https://inventti.com.br/nf-e-nfc-e-nt-2025-001-versao-1-02-traz-prorrogacao-e-ajustes-nas-regras-de-validacao/) A SEFAZ-PB a descreve como nacional para cartão e Pix dinâmico. [C](https://www.sefaz.pb.gov.br/announcements/16593-sefaz-pb-alerta-contribuintes-para-novas-validacoes-de-pagamento-integrado-nfc-e-e-nf-e-cartoes-e-pix) **Contradição não resolvida.**
- **737:** `tpIntegra=2` em UF que não aceita; facultativa, regra YA04a-20, sem lista de UFs. [S](https://oobj.com.br/bc/rejeicao-737-como-resolver)
- **NT 2026.006:** vinculação com `gPgtoVinc` e evento 110300, produção em 16/11/2026, sem obrigação em 2026; split previsto para 2027; não substitui YA. [S](https://blog.tecnospeed.com.br/nt-2026-006-vinculacao-de-pagamento-nf-e/) [S](https://mgcontecnica.com.br/2026/11/03/reforma-tributaria-nota-tecnica-que-implementa-a-vinculacao-da-transacao-de-pagamento-com-split-payment-na-nf-e-e-nfc-e-tem-novas-datas/)

**Risco de vender com maquininha solta, `tpIntegra=2`:**

| UF | O que achei | Risco |
|---|---|---|
| **RS** | Oficial: NFC-e integrada, comprovante e nota no mesmo equipamento, código nos dois; supermercados desde 04/2023, todos desde 2024. [C](https://estado.rs.gov.br/varejistas-devem-estar-atentos-a-obrigatoriedade-de-integracao-entre-nota-fiscal-e-meios-de-pagamento-eletronico) Multa de R$ 8.497,92 por equipamento ao mês, apreensão, autuações em lotes em 2026. [S](https://coad.com.br/home/noticias-detalhe/135698/rio-grande-do-sul-inicia-autuacoes-por-falta-de-integracao-entre-nfc-e-e-meios-de-pagamento) | **Alto. Não vender sem integração.** A notícia oficial não cita TEF; POS integrado parece admissível, sem confirmação no texto da norma |
| **SC** | Multiplus diz obrigatório; [S](https://multipluscard.com.br/blog/obrigatoriedade-tef-por-estado-2026/) norma de pagamento não achada. Homologação do PAF-NFC-e com laudo obrigatória, Ato DIAT 54/2026, cadastro com e-CNPJ. [S](https://www.coad.com.br/home/noticias-detalhe/139685/sefaz-sc-atualiza-regras-para-emissao-de-nfc-e-e-credenciamento-de-software-houses) | **Alto**, e bloqueia venda mesmo com TEF até o laudo |
| **SP** | Multiplus: malha fiscal. [S](https://multipluscard.com.br/blog/obrigatoriedade-tef-por-estado-2026/) Infotec: vigente para grandes. [S](https://infotecal.com.br/blog/obrigatoriedade-tef-vinculacao-pagamento-nfce-estados) Norma não achada | Médio, por cruzamento |
| **MG, PR, ES, DF** | Multiplus: malha. Infotec: exigência ativa. Nenhuma norma citada. PR em 2015: facultativa. [S antigo](https://sped.fazenda.pr.gov.br/sites/sped/arquivos_restritos/files/migrados/File/NFCE/Treinamento_NFCE_04_08_2015.pdf) | Médio; confirmar antes de vender |
| **RJ** | Multiplus: malha. Norma não achada. [S](https://multipluscard.com.br/blog/obrigatoriedade-tef-por-estado-2026/) | Médio ou baixo; confirmar |

Direção do mercado: CE, GO, MT, PB já exigem. [S](https://blog.tecnospeed.com.br/integracao-tef-com-a-nfc-e/) MT recusa intervenção manual. [S](https://inventti.com.br/sefaz-mt-nf-e-nfc-e-vinculo-do-comprovante-de-pagamento-eletronico-perguntas-e-respostas/) Um fornecedor prevê obrigação nacional em 2027, sem fonte normativa. [S](https://infotecal.com.br/blog/obrigatoriedade-tef-vinculacao-pagamento-nfce-estados) O TEF não tira do PDV a responsabilidade fiscal: a PayGo diz que NFC-e está fora do escopo do TEF. [C](https://paygodev.readme.io/docs/faq-geral)

## 8. Concorrentes

- **Omie:** sem integração direta; o lojista contrata intermediária (Auttar, PayGo, Cappta) e usa `TEF_DIAL`. [C](https://ajuda.omie.com.br/pt-BR/articles/8708507-omie-pdv-configurando-o-tef)
- **Alterdata:** SiTef e Scope, com servidor TEF. [C](https://ajuda.alterdata.com.br/pdvbase/tef-como-funciona-239353876.html)
- **Bling:** POS integrado, com código na NFC-e, nascido para RS e MT; modelos não informados. [C](https://blog.bling.com.br/?p=8020)
- **Conta Azul:** automático com Stone, sem Pix; digitação de NSU nas demais. [C](https://ajuda.contaazul.com/hc/pt-br/articles/360023352911-Frente-de-caixa-PDV-dados-de-pagamento-eletr%C3%B4nico-nas-emiss%C3%B5es-de-NFC-e)
- **Saipos:** "TEF Saipos" multiadquirente com Pix no pinpad, sem nomear o motor. [C](https://saipos.com/noticia/como-implantar-o-sistema-tef-em-seu-negocio)
- **TOTVS:** TEF Cloud próprio. [S](https://tdn.totvs.com/x/D55n) **Linx:** D-TEF, hoje no grupo TOTVS. [S](https://www.barchart.com/story/news/478731/stoneco-announces-successful-closing-of-linx-sale) **Tiny:** nada encontrado.

O que o mercado espera: pinpad multiadquirente com Pix, TEF contratado à parte ou embutido com marca própria, e POS integrado como opção para loja pequena.

## 9. Recomendação

**Contrate primeiro a PayGo por troca de arquivos no Windows, porque é o caminho sem FFI e multiadquirente com pinpad USB, documentação aberta e análise publicada em 5 dias úteis.**

Prazo documentado após o desenvolvimento: 5 dias úteis de análise mais até 10 dias úteis por número lógico, dentro de janela de 30 dias de instalação. É soma de prazos publicados, não promessa. O custo para a PME segue desconhecido; esse critério ainda não sustenta a escolha.

**Depois:**
1. **TEF Express**, em paralelo na proposta, pelo mesmo tipo de protocolo e pelo CNPJ da rede em `010-0X1`, código SAT em `010-0X2` e marca própria. [C](https://docs.multipluscard.com.br/pt/documentacao/tef/tefdial/descricaocampos/)
2. **SiTef Express**, para redes maiores e Linux, aceitando processo auxiliar.
3. **ConnectTEF ou Point**, para loja pequena e fora do Windows.
4. **Pix direto**, já na POC. AutoTEF só se surgir cliente exclusivo Stone.

**Três contatos:**
1. [Chamado PayGo no Jira](https://dev.proj.setis.com.br/servicedesk/customer/portal/16) mais [paygo.com.br/contato](https://paygo.com.br/contato/). Perguntar: preço por checkout, taxa de certificação, recertificação por versão, CNPJ da credenciadora e `tBand`, instalador nosso.
2. [TEF Express](https://tefexpress.com.br/), formulário "Peça o TEF Express", (18) 3221-8134, atendimento@multipluscard.com.br. Pedir tabela de software house, sistemas operacionais e quem certifica.
3. Fiserv, (11) 3170-5300, via [Fale Conosco](https://www.softwareexpress.com.br/pt/ajuda/). Pedir proposta do SiTef Express para uma loja e o custo da software house.

**O que mudaria a recomendação:**
- A PayGo não fornecer forma confiável de obter CNPJ e bandeira: TEF Express passa à frente.
- Preço da PayGo muito acima da faixa do TEF Express ou do SiTef Express.
- Primeiro cliente em Linux, ou caixas abaixo de 8 GB: SiTef com auxiliar, ou Smart POS.
- Exigência de TEF/DLL certificada que invalide arquivo no roteiro atual.
- Teste real provar que `authorization_code` do Mercado Pago bate com o comprovante: Point vira opção comercial para loja pequena.

## 10. O que não deu tempo de confirmar

- Texto oficial das NT 2025.001 e 2026.006 e do MOC, e a lista de UFs que aplicam 391, 392 e 737.
- Normas de SP, MG, PR, RJ, ES e DF. Antes de vender nessas UFs, confirmar com contador e SEFAZ.
- Texto da norma do RS sobre POS. Uma busca atribuiu ao governo a frase de que TEF não é obrigatório, mas a página aberta não a contém; não usar.
- Contrato PayGo inteiro; data da versão do contrato; cláusulas de fidelidade fora do anexo TEF.
- Se o roteiro PayGo exige pinpad físico desde o início, e se CPF basta para o sandbox.
- Prazos e preços atuais de SiTef, Stone, TEF Express, Scope, Elgin e ConnectTEF. Retorno de `cAut` no webhook da ConnectTEF.
- Linux da PayGo hoje; `DPOSDRV` da Stone não foi revisto; TEF Stone em relação ao piso no AutoTEF.
- Muitos [C] de páginas longas vieram de leitura por ferramenta que resume o conteúdo; confira a página antes de citar em contrato.
