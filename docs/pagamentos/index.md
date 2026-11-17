# Pagamentos: pinpads, TEF e os quatro provedores

> **BETA, PROVA DE CONCEITO.** `ravenrs-pagamento` nunca falou com um provedor real, um pinpad real ou um cartão real. Não tem homologação nem certificado de ninguém. Tudo que aparece como "passou" abaixo passou contra simuladores nossos.

**Marcação.** [C] li na fonte. [S] fonte secundária, trecho de busca ou fórum. [L] lembrança não confirmada.

Só li literalmente, sem intermediário, o XSD `PL_010`, o Anexo I do MOC 7.0, a NT 2025.001, o PDF da ABECS, a lista de terminais e trechos do ACBr. Os demais [C] passaram por um resumo automático da página: confira os identificadores críticos ao implementar contra o real. Pesquisa de novembro de 2026.

## Resumo

1. **Mercado Pago é o único dos quatro testável de ponta a ponta sem hardware**: dispositivo virtual e endpoint que simula status.
2. **Isso não prova o critério da POC.** Point Smart e os terminais do Stone Connect são maquininhas, não pinpads. Pinpad USB só entra por SiTef, PayGo, TEF Stone ou AutoTEF.
3. **A API do Mercado Pago não documenta `cAut` nem NSU.** É o maior risco fiscal encontrado (seção 6).

## 1. Pinpads

- **Participação de mercado: não confirmei.** Só há relatório sobre POS, não pinpad, dando cerca de metade para a PAX [S] https://www.marketresearch.com/Mordor-Intelligence-LLP-v4018/Brazil-POS-Terminal-Share-Trends-43841235
- **Lista oficial ABECS v02/2026.** O histórico de revisões cita Gertec (PPC930, PPC940, PPC940S, PPC950, PPC960, MP-145, MP15), Ingenico (Lane3000, Lane3600, Link2500, SELF5000), PAX (Q25), Tectoy (Q25, Q35, D140), Verifone (P200) e Newland (P180). A tabela vigente, na página 4, é imagem e não foi lida [C] https://abecs.org.br/certificacao-funcional-de-pinpad
- PAX D180 não aparece nesse histórico.
- **Homologados na TEF Stone:** PPC930 e Lane-3000 aceitam QR; PPC920 e IPP320 não [C] https://tefdoc.stone.com.br/docs/dispositivos-suportados.md
- **Conexão:** a norma prevê física serial sobre RS-232, USB ou Bluetooth, a 19.200 bps, 8N1 quando relevante ao meio [C, PDF da seção 2]. O ACBr diz que a ABECS exige cabo USB expondo porta serial e recomenda o driver do fabricante [C] https://www.projetoacbr.com.br/forum/topic/76119-novo-componente-acbrabecspinpad/
- **Na prática, quem define o pinpad aceito é a casa de TEF** [C] https://www.projetoacbr.com.br/forum/topic/81131-pinpad-para-utilizar-com-acbrtefd/
- **Para a POC:** PPC930 é a compra de menor risco: está na lista ABECS e na da Stone. Confirme com PayGo e SiTef antes de comprar.

## 2. Especificação ABECS

- **Vigente:** protocolo v2.20 (28/02/2025), suplementos SU001 e SU002 e casos de teste v2.20.01 (17/12/2025), públicos para download [C] https://abecs.org.br/certificacao-funcional-de-pinpad
- PDF em inglês lido [C] https://abecs.org.br/storage/pages/01JP0P3T0WA71S1TFDF2QYSKVM.pdf
- **Não li** os suplementos nem os casos de teste.
- A BibComp 1.08a é a geração antiga; há equipamentos de v1.08 a v2.12 em campo [C] https://docs.cielo.com.br/conecta/docs/conheca-a-biblioteca-do-pin-pad

### Ponto para revisão jurídica

O PDF traz o aviso de que o conteúdo só pode ser usado ou copiado com permissão escrita da ABECS [C]. O crate implementa o protocolo para interoperar e **não embute o documento nem suas tabelas textuais**: a tabela de mensagens do `GCD` existe só como índices numéricos dos poucos que um caixa usa, e os testes usam alguns exemplos curtos de pacotes em hexadecimal. Minha leitura de que isso é aceitável é opinião, não parecer. **Peça revisão jurídica antes de publicar ou vender**, inclusive sobre esses vetores curtos.

### Comandos no sumário da v2.20 [C]

| Grupo | Comandos | No crate |
|---|---|---|
| Controle | OPN (clássico e seguro), GIN, GIX, DWK, CLO, CLX | OPN clássico, GIN, GIX, CLO |
| Básicos | CEX, CHP, CKE, DEX, DSP, EBX, ENB, GCD, GDU, GKY, GPN, GTK, MNU, RMC | DEX, DSP, GCD, GKY, MNU |
| Multimídia | MLI, MLR, MLE, LMF, DMF, DSI | MLI, MLR, MLE, DSI |
| Tabelas EMV | GTS, TLI, TLR, TLE | nenhum |
| Cartão | GCR, CNG, GOC, FNC (obsoletos), GCX, GED, GOX, FCX | nenhum, de propósito |

### Quadro [C], implementado em `abecs::quadro` e `abecs::sessao`

- `SYN(16h) + dados + ETB(17h) + CRC16`; substituição 13h→13 33, 16h→13 36, 17h→13 37; até 2049 bytes originais.
- CRC-16-CCITT, polinômio 1021h, inicial 0, sobre dados + ETB antes da substituição. Vetor oficial `16 4F 50 4E 17 A8 A9`, conferido em teste.
- ACK 06, NAK 15, CAN 18, EOT 04. 2 s para ACK/NAK; 10 s para resposta não bloqueante; bloqueante espera indefinidamente, abortável com CAN (até 3 tentativas, 2 s pelo EOT).

### Decisões

- **OPN clássico.** A norma o chama de obsoleto e prefere o seguro, que exige RSA. O ACBr também não implementa o seguro [C] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrSerial/ACBrAbecsPinPad.pas. Um pinpad que recuse o clássico falha em `abrir`. **Só o teste com hardware responde isso.**
- **O ACBr (LGPL) foi usado para conferir comportamento**: identificadores e tempos. Nenhum código nem tabela foi copiado.
- Marcas de "bloqueante" se perderam na extração do PDF; tratei GCD, MNU e GKY como bloqueantes pela semântica. Confira.

## 3. O que a automação pode fazer no pinpad sem TEF

**Pode** (sem chave de adquirente):
- Mensagem, tecla, menu, informações, imagem ou QR Pix (MLI, MLR, MLE, DSI; formatos em `PP_MFSUP`) [C].
- `GCD`: dado em aberto digitado pelo cliente, como CPF ou telefone. Para cumprir o PCI, a mensagem vem de uma **tabela fixa da norma** [C, seção 3.3.8 do PDF].
- Códigos D e acima só saem em aberto a partir da 2.10; a chave DES antiga está em obsolescência [C] https://docs.apis-fiserv.com/latam/docs/descricao-dos-parametros
- Em equipamento antigo varia: um IPP350 com 1.08a falhou em GIX e DEX [S] https://www.projetoacbr.com.br/forum/topic/80433-acbrabecspinpad-igenico-ipp350-error-reading-ack/

**Não deve:**
- **Capturar senha.** O `GPN` devolve bloco cifrado e exige chave MK/WK ou DUKPT no pinpad [C, seção 3.3.11]. O crate não tem esse comando.
- **Pedir o número do cartão.** A tabela do GCD tem índice para isso. O crate recusa os índices 000Bh, 000Ch e 0032h, recomendação minha: PAN em claro traz o software para o escopo PCI DSS [L].

**Por que compra exige TEF ou adquirente.** É inferência do desenho da norma: caminho até o autorizador, chaves e tabelas pertencem à rede. Não localizei texto normativo que diga isso literalmente.

**Conflito de porta.** Com TEF ativo, o cliente do TEF é dono da serial; usar o pinpad fora dele foi descrito pelo consultor do ACBr como inviável em produção [S] https://www.projetoacbr.com.br/forum/topic/76259-tacbrabecspinpad/. Prefira as funções do TEF: PayGo `CDP`, `QRP`, `FQR`, `MNU` [C] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/ACBrTEFTXTPayGo.pas

## 4. Os quatro provedores

### Mercado Pago Point (Orders API): `mercado_pago::MercadoPagoPoint`

Docs [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/overview

- **Transporte:** HTTPS em nuvem. Terminais Point Smart 1 e 2, Pro 2 e 3.
- **Autenticação:** Bearer + `X-Idempotency-Key`. O token deve ficar **só no backend** [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/payment-processing
- **Preparar:** loja, caixa, `GET /terminals/v1/list`, `PATCH /terminals/v1/setup` com `operating_mode: PDV` [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/configure-terminal
- **Criar:** `POST /v1/orders`. **Aguardar:** webhook `order` com `x-signature`, resposta em 22 s [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/notifications. Consulta `GET /v1/orders/{id}` existe, mas o uso frequente é desaconselhado.
- **Status** [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/resources/status-order-transaction
- **Cancelar:** em `at_terminal` é assíncrono (202) e exige `x-allow-cancelable-status`. **Estornar:** só total, em até 90 dias. **Sem terceira perna.**
- **Teste, sem hardware** [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/integration-test: dispositivo virtual `NEWLAND_N950__SBX0000001`, `POST /v1/orders/{id}/events`. Exige conta e aplicação com credencial de teste, e webhook com URL HTTPS pública. A página é ambígua sobre o prefixo do token (`APP_USR` no texto, `TEST-` nos exemplos).
- **Comprovante:** `print_on_terminal`; `POST /terminals/v1/actions` [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/configure-printings
- **Contrato:** a página de produção não cita certificação [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/go-to-production; a medição manual de qualidade exige pagamento real em terminal físico [C] https://www.mercadopago.com.br/developers/pt/docs/mp-point/integration-quality. Taxas não levantadas.

**No crate:** criar, consultar, cancelar, estornar, simular eventos e ler notificação, atrás de `TransporteHttp`, com fixtures escritas por nós a partir dos exemplos documentados. **Não foi testado contra o real: não temos conta.** Não valida `x-signature`, porque o segredo pertence a um servidor. Não usei `reference_id` como `cAut`.

**Risco que sobra:** se a energia cair entre a resposta do `POST` e a gravação do id da ordem, o id se perde, e as páginas lidas não mostram consulta por `external_reference`. O crate responde `Indeterminada`, em vez de cancelar por conta própria, e alguém precisa olhar o terminal. O prazo padrão de 15 min da ordem limita a janela.

### Stone

Caminhos diferentes, e a escolha é comercial.

**a) Connect 2.0 (POS Android, via Pagar.me).** Docs [C] https://connect-stone.stone.com.br/docs/processo-de-integração

- HTTPS em nuvem; Basic com a SK + `ServiceRefererName`; `POST https://api.pagar.me/core/v5/orders/` com `poi_payment_settings` [C] https://connect-stone.stone.com.br/reference/criar-pedido.md
- Webhook obrigatório `charge.paid` e `charge.refunded` [C] https://connect-stone.stone.com.br/reference/webhook.md
- Fechar é obrigatório após pagar; limite de 30 pedidos abertos [C] https://connect-stone.stone.com.br/reference/fechamento-de-um-pedido.md
- Cancelar só antes do pagamento. **Estorno não tem rota de API** [C] https://connect-stone.stone.com.br/reference/cancelamento-de-pedido.md
- **Sem sandbox nem simulador**: valida em produção com R$ 1,00 e cartão real [C] https://connect-stone.stone.com.br/reference/autenticação-copy.md. O processo de integração cita conta de teste no dashboard: esclareça com a Stone.
- Parceria e Partner Hub obrigatórios, depois roteiro [C].

**No crate:** só `stone::dados_de_charge_paid`, o mapeamento fiscal do webhook, para o servidor que o receber, e `Capacidades::STONE_CONNECT`. Não há adaptador de pedido: pede parceria, POS físico e cartão real, e não prova pinpad. Com ele, se a NFC-e falhar depois de pago, o estorno é **manual no POS**, registrado por `registrar_estorno_externo`.

**b) TEF Stone (pinpad).** Windows nativo e Linux "multiplataforma" [C] https://tefdoc.stone.com.br/docs/getting-started; DLL `DPOSDRV.DLL` [C] https://tefdoc.stone.com.br/docs/client-tef-com-a-automação-comercial.md; troca de arquivos só Windows, com texto que cita modem e parece antigo [C] https://tefdoc.stone.com.br/docs/troca-de-arquivos.md; `ConfirmaCartao`, `DesfazCartao`, `FinalizaTransacao`, queda de energia pelo parâmetro 1021 [C] https://tefdoc.stone.com.br/reference/fluxo-de-confirmação-e-desfazimento.md. **Não confirmei:** ambiente de teste, homologação, custo. **No crate:** `Capacidades::STONE_TEF`, pelo mesmo contrato de auxiliar do SiTef.

**c) AutoTEF (pinpad, HTTP local): `stone::AutoTef`, NÃO CONFIRMADO.** `http://localhost:8000/` [C, no ACBr] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/ACBrTEFAPIStoneAutoTEF.pas. A doc oficial respondeu 403 em https://autotef.stone.com.br/docs, então não sei se o produto segue ativo. Sem terceira perna e sem consulta no que li. É o melhor candidato sem FFI: **pergunte à Stone**. Leva aviso em toda aprovação.

**d) Outras rotas.** Módulo Stone dentro do SiTef [S] https://www.projetoacbr.com.br/forum/topic/42483-integracao-tef-com-stone/; Cappta, do grupo Stone, com Intpos [S] https://www.projetoacbr.com.br/forum/profile/67674-cappta-homologa/content/

### SiTef (Fiserv): `auxiliar` e `sitef`

- Biblioteca nativa: `CliSiTef32I.dll`, `CliSiTef64I.dll`, `libclisitef.so` [C] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/ACBrTEFCliSiTefComum.pas. macOS e API HTTP local oficial: não achei.
- Fluxo: `ConfiguraIntSiTefInterativo`, `IniciaFuncaoSiTefInterativo`, `ContinuaFuncaoSiTefInterativo` enquanto 10000, imprimir, `FinalizaFuncaoSiTefInterativo` [C] https://docs.apis-fiserv.com/latam/docs/funcionamento-basico
- Configuração [C] https://docs.apis-fiserv.com/latam/docs/configuracao-biblioteca
- Teste: SitDemo (Windows) + DLLs simuladas [C] https://www.projetoacbr.com.br/forum/topic/87428-sitef-como-iniciar-a-homologa%C3%A7%C3%A3o-por-dll-clisitef/; SOs para Linux [S] https://www.projetoacbr.com.br/forum/files/file/530-sdk-clisitef-linux-homologa%C3%A7%C3%A3o/. O caminho encontrado é via parceria ACBr; não confirmei o direto com a Fiserv.
- `dev.softwareexpress.com.br` não resolveu aqui; use `docs.apis-fiserv.com/latam`.

### PayGo (Setis): `intpos`

- **PayGo Windows, troca de arquivos:** sem FFI. Comandos ATV, CRT, ADM, CNC, CNF, NCN, CDP [C] https://paygodev.readme.io/docs/campos-previstos-para-cada-comando.md
- **PGWebLib DLL:** sem Warsaw a doc pede medidas alternativas de segurança [C] https://paygodev.readme.io/docs/kit-de-integracao-pgweblib-dll-sem-protecao
- **ControlPay:** web service, mas o cliente é o próprio PayGo Windows no PDV [C] https://paygodev.readme.io/docs/integrando-com-o-controlpay.md
- Downloads só listam Windows e Android [C] https://paygodev.readme.io/docs/downloads.md. **Trate como só Windows até a PayGo responder.**
- Três adquirentes de teste: C6PAY, REDE e PIX C6 BANK [C] https://paygodev.readme.io/docs/passo-a-passo-homologação.md. Não cita pinpad; presumo necessário no cartão [L].

### Formato IntPos, implementado em `intpos`

- Caminhos padrão `C:\TEF_DIAL\req` e `resp`, `C:\PAYGO\REQ` e `RESP` [C] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/ACBrTEFDDial.pas. No crate o diretório é configurável.
- Linha `III-SSS = VALOR`; grava `IntPos.tmp`, renomeia para `IntPos.001`; STS em 7 s, testando a cada 250 ms; resposta sem prazo fixo [C] https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/ACBrTEFTXTComum.pas
- Campos [C, PayGo v2.25] https://paygodev.readme.io/docs/detalhamento-dos-campos.md: 000 comando, 001 identificação, 002 documento, 003 valor em centavos, 004 moeda, 009 status (0 é sucesso), 010 rede, 012 NSU (no Pix, EndToEndId), 013 autorização (não vale para Pix), 027 controle, 028/029 via única, 030 mensagem, 712/713 via cliente, 714/715 via loja, 706 capacidades, 716/733/735/736/738 identificação, 731 tipo, 748 cartão, 749 forma (8 carteira digital), 999 fim.
- Latin-1 é suposição; o texto lido não define a codificação.
- A página da PayGo mostra `029-001: "texto"`; o crate lê os dois separadores e escreve ` = `.

### Saída sem `unsafe`

1. **Arquivos** (PayGo Windows): só `std::fs`.
2. **HTTP** (Mercado Pago, Connect, AutoTEF): `ureq`, atrás de feature.
3. **Processo auxiliar para DLL** (SiTef, TEF Stone, PGWebLib).

### Como será o auxiliar com FFI (não existe)

- Binário fora deste workspace, para o `forbid(unsafe_code)` seguir intacto; é a única parte com `unsafe`. Compila em 32 ou 64 bits conforme a biblioteca.
- Carrega a biblioteca e implementa `ProvedorPagamento` sobre ela, usando `sitef::CicloSiTef` para traduzir o ciclo iterativo. Depende de `ravenrs-pagamento`.
- O `main` é o de `src/bin/auxiliar_simulado.rs`: lê uma linha de stdin, responde `auxiliar::atender`, uma linha em stdout, log em stderr.
- Queda do auxiliar não derruba o PDV: vira erro incerto, e o diário decide ao reconectar. Isso foi testado com um filho real que morre na confirmação.
- **Não pode registrar** a resposta do operador. Estorno interativo do CliSiTef não está coberto: o contrato hoje é uma chamada única.
- **Pergunte à Fiserv** se redistribuir a biblioteca com um auxiliar afeta a certificação. Não encontrei resposta.

### Demais soluções e ACBr

Unidades em `Fontes/ACBrTEFD` [C] https://api.github.com/repos/joaoduarte19/ACBr/contents/Fontes/ACBrTEFD: `ACBrTEFAPI` traz PayGoWeb, CliSiTef, Elgin, StoneAutoTEF, Aditum, Scope, Destaxa, TPag, Equals, DirectPin, TXT, Auttar. Aditum é HTTP local em `https://localhost:4090/v2/`; Destaxa, socket; SmartTEF, REST em nuvem; Elgin, arquivos em `C:\Cliente`; Cappta, DLL no ACBr e TEF_DIAL em relato [S]; TPag `libDM_SDK`; Scope e Auttar, biblioteca + TCP [C, unidades em `https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrTEFD/`]. **Linx/GetNet, Rede e Tef Express: não confirmei nada.**

## 5. Homologação de cada provedor

**Regras que todo adaptador cumpre:** a transação só termina após aprovação, impressão e confirmação ou desfazimento; sem isso fica pendente e pode bloquear novas [C] https://paygodev.readme.io/docs/8-confirmação-de-transação.md. Queda de energia: gravar em memória não volátil logo após o retorno [C, mesma URL].

### Roteiro: PayGo [C] https://paygodev.readme.io/docs/passo-a-passo-homologação.md

1. Baixar o kit do PayGo Windows em uma máquina Windows.
2. Abrir chamado em https://dev.proj.setis.com.br/servicedesk/customer/portal/16; receber credenciais.
3. Preencher `Perfil::PayGo` com o registro de certificação do campo 738.
4. Executar o roteiro de testes (v20261030 nos downloads) contra `ProvedorArquivos`.
5. Enviar resultado e logs. Análise em até 5 dias úteis, com certificado. Custo não informado.

### Roteiro: SiTef

1. Falar com a Fiserv ou com um distribuidor; relato antigo de homologação sem taxa e 30 a 50% da mensalidade [S, datado] https://www.projetoacbr.com.br/forum/topic/40154-tef-com-software-express/
2. Construir o auxiliar com FFI (seção 4).
3. Rodar contra SitDemo + bibliotecas simuladas, com pinpad.
4. Preencher o pré-roteiro; enviar logs e comprovantes para o certificado [S] https://www.projetoacbr.com.br/forum/topic/90279-sitef-configura%C3%A7%C3%A3o-clisitefini-para-homologa%C3%A7%C3%A3o/
5. Em produção, terminal não aprovado em 24 h bloqueia [S].

### Roteiro: Mercado Pago

1. Criar conta e aplicação em https://www.mercadopago.com.br/developers/panel/app.
2. Subir o servidor que guarda o token e recebe o webhook.
3. Rodar `simular_evento` contra o dispositivo virtual: é o primeiro contato real, e confere nossas fixtures.
4. **Perguntar ao suporte** de onde vem o `cAut`.
5. Comprar um Point, associar, rodar pagamento real pequeno, medir qualidade, ativar produção. Prazo e custo não encontrados.

### Roteiro: Stone

1. Decidir o caminho (seção 4) e entrar no programa de parcerias [C] https://connect-stone.stone.com.br/docs/processo-de-integração
2. **AutoTEF:** perguntar se está ativo, onde está a doc e o instalador; conferir `stone::AutoTef` campo a campo.
3. **TEF Stone:** obter SDK e roteiro; construir o auxiliar.
4. **Connect:** implementar pedido e fechamento; testar em produção com cartão real; preencher o roteiro. Prazo e custo não encontrados.

A certificação ABECS é do terminal, não do software [S] https://api.abecs.org.br/wp-content/uploads/2020/05/Normativo-023-certificac%CC%A7a%CC%83o-de-pinpads.pdf

## 6. NFC-e

**XSD `leiauteNFe_v4.00.xsd`** (`PL_010`) [C, lido]. `card` opcional: `tpIntegra` obrigatório dentro do grupo (1 integrado: "TEF, Comércio Eletrônico, POS Integrado"; 2 não integrado); opcionais `CNPJ` (`[0-9A-Z]{12}[0-9]{2}`), `tBand` (`[0-9]{2}`), `cAut` (1 a 128), `CNPJReceb`, `idTermPag` (1 a 40).

**MOC 7.0 Anexo I e NT 2025.001** [C, lidos em cópias locais]:
- YA04-10: cartão (tPag 03, 04) exige o grupo; opcional por UF (391).
- YA04a-20: `tpIntegra=2` em UF que não aceita (737).
- YA05-10: `tpIntegra=1` exige `CNPJ` e `cAut`; opcional por UF (392).
- YA05-20: CNPJ com zeros ou dígito inválido (437), obrigatório.
- YA06-10: `tBand` deve existir na tabela publicada no Portal (443), obrigatório desde 01/09/2021.

**NT 2023.004 não lida diretamente.** 963 restringe `card` a tPag 03, 04, 10, 11, 12, 13, 15, 17, 18, e a 391 passa a cobrir Pix [S] https://blog.tecnospeed.com.br/?p=24422. RS exige `cAut` **igual ao impresso** [C] https://atendimento.receita.rs.gov.br/4-quais-dados-especificos-que-devem-ser-informados-na-nfc-e-e-se-a-empresa-ja-estiver-integrada-e-mesmo-assim-receber-alerta

| Provedor | `cAut` | `tBand` | CNPJ |
|---|---|---|---|
| SiTef | campo 952 | campo 951 | campo 950 [C, ACBr] |
| PayGo | 013-000; no Pix, 012-000 com alerta | 748 ou 040 | 010-000 + nossa tabela |
| Stone Connect | `metadata.authorization_code` [C] https://connect-stone.stone.com.br/reference/recebendo-um-pagamento.md | `metadata.scheme_name` | nossa tabela |
| AutoTEF | `receipt.authorisationCode` | `receipt.brandName` | nossa tabela |
| Mercado Pago | **ausente** | `payment_method.id` | nossa tabela |

**Mercado Pago: risco aberto.** `transactions.payments[]` não tem autorização nem NSU; `reference_id` é candidato sem confirmação [C] https://www.mercadopago.com.br/developers/pt/reference/in-person-payments/point/orders/get-order/get.md. O crate entrega `c_aut: None` com `AlertaFiscal::CAutAusente`.

**Tabelas em `fiscal.rs`, com a fonte em cada linha:**
- `BANDEIRAS`: códigos 01 a 09 do MOC 7.0 [C]. **Não conferi a tabela vigente do Portal**, que a NT 2020.006 manda consultar e pode ter mais linhas [S: 28 operadoras] https://buscadorncm.com.br/nfe/meios-de-pagamento. Apelidos do Mercado Pago além de `master` são [L]. Bandeira desconhecida vira alerta, nunca 99 sozinha.
- `REDES`: Stone, Cielo, Getnet, Mercado Pago [S]. **Nenhuma conferida na Receita**; um teste confere só o dígito verificador. Linha vinda da tabela sempre gera `CnpjDeTabelaNaoConferida`. O campo 951 do SiTef pode ser código ou nome: não confirmado.
- **Pix com `cAut` igual ao EndToEndId** é leitura minha do texto do XSD; gera `CAutPixPeloEndToEnd`. Peça revisão fiscal.

**Ordem da venda.** TEF: autorizar, emitir, imprimir, CNF; se a nota falhar, NCN. Mercado Pago e Connect: o dinheiro já foi capturado; se a NFC-e falhar, contingência ou estorno.

## 7. Rust e referências

- `serialport` 4.10.1, MPL-2.0 [C] https://crates.io/api/v1/crates/serialport, sem `libudev`. CI cobre Linux, macOS, Windows, FreeBSD [C] https://raw.githubusercontent.com/serialport/serialport-rs/main/README.md; wasm não listado.
- USB-CDC: Linux `/dev/ttyACM0`, e o `AUTO_USB` do CliSiTef não lista o PPC930 no Linux [S] https://dev.softwareexpress.com.br/en/docs/sitef-interface-simplificada/configuracao_do_pinpad; Windows porta COM com driver [C]; macOS `/dev/cu.usbmodem*` [L], **sem confirmação de suporte**.
- Crate Rust de ABECS ou TEF: não encontrei. `bc108` (Dart, MIT, BibComp 1.08) [C] https://pub.dev/packages/bc108

## 8. Arquitetura

| Camada | Onde | Observação |
|---|---|---|
| Domínio sans-IO | `dominio`, `fiscal` | compila em wasm |
| Diário e recuperação | `diario`, `pagamentos` | escreve antes de cada efeito |
| Contrato | `provedor` | `Passo` serve aos quatro |
| Simulador | `simulador` | estilo MSW, determinístico |
| Pinpad direto | `abecs` | sem GPN, sem cartão |
| Arquivos | `intpos` | só nativo |
| HTTP | `http`, `mercado_pago`, `stone` | `ureq` atrás de `http` |
| Auxiliar | `auxiliar`, `sitef` | FFI fora do workspace |

**Regra da recuperação.** Decisão gravada é cumprida. Sem decisão, aprovação de venda inacabada é desfeita. Sem terceira perna nada se desfaz: vira `CobradoSemConclusao`, para a aplicação concluir ou estornar. Provedor que não sabe dizer vira `Indeterminado`, nunca um palpite.

**Servidor.** Mercado Pago e Connect exigem um servidor para webhook e guarda de token. **O crate não o inclui.** Um PDV desktop não recebe chamada de entrada.

## Decisões do dono do produto

1. Contratar PayGo, SiTef e Stone: quem assina, como software house ou via distribuidor. Sem isso não há certificado nem produção.
2. Qual caminho da Stone: pinpad (TEF/AutoTEF) ou Connect (POS).
3. Aceitar ou não o Mercado Pago com `cAut` em aberto.
4. Windows como obrigatório nos caixas com TEF. PayGo só aparece para Windows e Android, e nada indica macOS em nenhum.
5. Operar o servidor de webhook e token.
6. Revisão jurídica sobre a especificação ABECS; revisão fiscal sobre as tabelas e o Pix.

## O que a POC prova e não prova

**Prova, contra simuladores nossos:**
- Queda em cada linha do diário, inclusive entre aprovar e confirmar, e queda durante a própria recuperação, sem deixar nada aberto nem cobrar duas vezes.
- Sem diário, o provedor não é chamado.
- Fluxo com perguntas ao operador, cenários do simulador e dados fiscais com alertas.
- Codec ABECS contra exemplos de pacote da norma; enlace com NAK, CRC ruim e pinpad mudo.
- Troca de arquivos IntPos em arquivos reais, incluindo CNF, NCN, CNC e pendência.
- Contrato do auxiliar por um processo filho real, inclusive morrendo.
- Compatibilidade do núcleo com wasm32, por compilação.

**Não prova:**
- Nenhuma aceitação por adquirente, transação real, pinpad real ou porta serial real.
- Que PayGo, TEF_DIAL e um gerenciador real se comportam como `GerenciadorSimulado`, que escrevi lendo documentação. Windows não foi exercitado.
- Que a API real do Mercado Pago responde como nossas fixtures.
- Que CliSiTef entrega os comandos e campos como `CicloSiTef` supõe: menu, códigos de retorno e significado de "continua" vieram de leitura e de lembrança, sem SitDemo.
- Que o AutoTEF existe hoje.
- Que uma SEFAZ aceita esses dados, ou o desempenho e a usabilidade da tela.

## O que não consegui confirmar

Participação de mercado; tabela vigente da lista ABECS; suplementos e casos de teste ABECS; NT 2023.004 e tabela de `tBand` do Portal diretamente; doc oficial do AutoTEF (403) e da Software Express (domínio não resolve); `cAut` do Mercado Pago; PayGo em Linux; exigência de pinpad na homologação PayGo; custos e prazos, salvo os 5 dias úteis da PayGo e relatos antigos sobre SiTef; custo e homologação de TEF Stone, Scope, Cappta, Auttar; Linx/GetNet e Rede; suporte a macOS de qualquer pinpad; codificação dos arquivos IntPos.
