# Automação comercial: o que existe e o que falta para vender

**Finalidade.** Isto começou como POC, mas reúne o que é preciso para lançar um produto profissional de automação comercial para pequenas e médias empresas. Este documento é o mapa desse caminho.

**Situação hoje.** Tudo o que é fiscal ou de pagamento é **beta, prova de conceito**. Nenhuma nota foi enviada a um servidor real da SEFAZ. Nenhuma etiqueta ou cupom saiu em impressora física. Nenhum pagamento passou por pinpad ou provedor real. Produção está bloqueada no código.

Este documento não traz prazos, porque nenhum foi estimado. Também não é parecer jurídico, fiscal ou contábil.

Documentos ligados:
- [`crates/ravenrs-nfe/REGRAS.md`](../crates/ravenrs-nfe/REGRAS.md): de onde veio cada regra fiscal e o que ficou sem fonte.
- [`docs/PAGAMENTOS.md`](PAGAMENTOS.md): pagamentos, origem das regras e homologação de cada provedor.
- [`docs/TEF-COMPARATIVO.md`](TEF-COMPARATIVO.md): comparativo de TEF.
- [`docs/PERIFERICOS.md`](PERIFERICOS.md): periféricos como dados.
- [`docs/NFSE.md`](NFSE.md): pesquisa da NFS-e no Padrão Nacional.
- [`crates/ravenrs-report-label/FONTES.md`](../crates/ravenrs-report-label/FONTES.md): o que cada manual de impressora confirmou.
- [`examples/mini-pdv/README.md`](../examples/mini-pdv/README.md): como rodar o caixa e o que testar por tecla.

## 1. O que existe hoje

A coluna "Como foi verificado" diz o que de fato aconteceu. "Sem janela" quer dizer teste automático que dirige a tela pela árvore de acessibilidade, sem abrir janela.

### Caixa e cadastro

| O quê | Crate | Como foi verificado |
|---|---|---|
| Carrinho, `quantidade*código`, totais em decimal exato, formas de pagamento e troco | `ravenrs-pos` | Testes do crate |
| Tela do caixa só pelo teclado, com leitor de código, venda pendente, cliente e observação | `ravenrs-app-pos` | `examples/mini-pdv/tests/caixa.rs`, sem janela; telas olhadas em PNG. Sem leitor físico |
| Produtos com GTIN validado, NCM, CFOP, CSOSN, unidade e preço | `ravenrs-app-pos`, editor `Gtin` em `ravenrs-app-ui` | Testes sem janela |
| Clientes com endereço, para a nota grande | `ravenrs-app-pos` | Testes sem janela |
| Bloqueio de tela com PIN | `Session::lock` em `ravenrs-app-ui` | Testes sem janela. Não é fronteira de segurança sozinho |
| Registros em SQLite | `ravenrs-app-sqlite` | Testes do crate e do framework |
| Entrada de nota de fornecedor, estoque, custo médio, códigos próprios e formação de preço | `ravenrs-pos`, `ravenrs-app-pos` | **Em desenvolvimento enquanto este texto foi escrito.** Confira o estado no código antes de contar com isso |

### Nota fiscal

| O quê | Crate | Como foi verificado |
|---|---|---|
| NFC-e (65) e NF-e (55), síncrono, Simples Nacional (CRT 1, CSOSN 102 e 500) | `ravenrs-nfe` | XML validado com `xmllint` contra o XSD `PL_010`, em cópias do sped-nfe. **Nunca foi a um servidor real** |
| Chave de acesso, DV, CNPJ alfanumérico | `ravenrs-nfe` | Testes do crate; DV do CNPJ de memória, ver `REGRAS.md` |
| C14N inclusiva e XMLDSig, SHA-1 ou SHA-256 | `ravenrs-dfe`, comum à NF-e e à NFS-e | C14N comparada com `xmllint --c14n`; assinatura SHA-1 verificada com `openssl`; SHA-256 só nos testes do crate |
| NFS-e do Padrão Nacional: DPS, emissão, recuperação pelo Id, consulta, cancelamento e convênio | `ravenrs-nfse` | Só contra o `SefinSandbox`. Ordem lida no XSD v1.01, sem `xmllint`. **Nunca foi à Sefin** |
| DANFSe A4 no editor de relatórios | `ravenrs-app-nfe` | Teste do render: uma página, aviso, chave e QR acima de 1,52 cm. Endereço do QR não confirmado |
| SOAP 1.2, status, autorização, consulta e cancelamento | `ravenrs-nfe` | Só contra o sandbox |
| Rede real com A1, TLS verificado, mTLS | `ravenrs-nfe`, feature `http` | Servidor mTLS local, com certificado de teste. **Nunca falou com a SEFAZ** |
| Sandbox da SEFAZ no estilo MSW, que confere assinatura, ordem, totais e duplicidade | `ravenrs-nfe::sandbox` | Testes do crate. Passar nele não garante autorização real |
| Diagnóstico de rejeição, autópsia de assinatura, laboratório de certificados, máscara de dados pessoais | `ravenrs-nfe::sandbox` | Testes do crate. Em desenvolvimento |
| Emitente, notas emitidas, consulta, cancelamento e avisos que não fecham | `ravenrs-app-nfe` | Testes sem janela |
| Troca de NFC-e autorizada por NF-e (cancela e emite outra) | `ravenrs-app-nfe`, `ravenrs-app-pos` | Teste sem janela. **Regra fiscal sem validação de contador** |

### Impressão

| O quê | Crate | Como foi verificado |
|---|---|---|
| Relatório em ZPL II, EPL2/PPLB, PPLA, TSPL e ESC/POS | `ravenrs-report-label` | Bytes lidos de volta por um parser de teste e códigos lidos por um decodificador independente (`rxing`). **Nunca impresso** |
| Saída por TCP 9100, CUPS, arquivo `.prn`, compartilhamento do Windows | `ravenrs-report-label` | Testes do crate. O caminho do Windows não foi executado |
| Cupom em bobina de 80 e 58 mm, com QR Code, personalizável | `ravenrs-app-nfe`, editor padrão | Testes sem janela. Campos obrigatórios protegidos e "SEM VALOR FISCAL" obrigatório |
| DANFE A4 como relatório guardado | `ravenrs-app-nfe`, editor padrão | Teste sem janela. **Sem conferência com o leiaute oficial** |
| Etiqueta de gôndola, com prévia em pontos | `ravenrs-app-pos`, `ravenrs-report` (`labels`) | Testes sem janela e telas em PNG |
| Corte, gaveta, bipe e avanço como controles do layout | `ravenrs-report`, `ravenrs-report-designer` | Testes de bytes. Sem gaveta nem impressora de verdade |
| Editor de relatórios como único editor de layouts, com "Impressora direta" | `ravenrs-report-designer` | Testes do editor |

### Periféricos

| O quê | Crate | Como foi verificado |
|---|---|---|
| Impressoras, etiquetadoras, pinpads e leitores descritos como dados, em manifestos versionados com catálogo assinado | `ravenrs-perifericos` | **Em desenvolvimento enquanto este texto foi escrito.** Confira o estado no código |

### Pagamento

| O quê | Crate | Como foi verificado |
|---|---|---|
| Interface comum, diário que grava antes de agir e recuperação depois de queda de energia | `ravenrs-pagamento` | Testes do crate. Em desenvolvimento |
| Simulador de provedor, protocolo ABECS, arquivos IntPos, adaptadores | `ravenrs-pagamento` | Só contra o simulador. **Nunca testado com pinpad, cartão ou provedor real.** Ver [`PAGAMENTOS.md`](PAGAMENTOS.md) |
| Ligação com a tela do caixa | | **Ainda não existe** |

### O que o framework ganhou com isso

Avisos de módulo, trabalho em segundo plano, registro único, numeração sem buraco, rascunhos, janela de balcão, teclas F e Alt, `KeyBar`, `ScanField` e tema por aplicação entraram em `ravenrs-app`, `ravenrs-app-ui` e `ravenrs-widgets`. `examples/feature-center` demonstra cada um, e `examples/feature-center/tests/tour.rs` os testa sem janela.

## 2. O que falta para vender

A ordem dos blocos é a recomendada. Cada bloco destrava o seguinte. Quando a coluna "De quem depende" diz **dono** ou **contrato**, o desenvolvimento não avança sozinho.

### Bloco A: provar que funciona de verdade

Sem isto, nada do resto importa. Aqui se descobre quanto do que está pronto sobrevive ao mundo real.

| Item | O que é | De quem depende |
|---|---|---|
| Certificado A1 e contribuinte credenciado | Compra do A1 e um CNPJ credenciado em homologação. A SEFAZ pode rejeitar com 203 sem credenciamento | **Dono** |
| Hardware para testar | Certificado A1, pinpad PPC930 ou equivalente, etiquetadora, impressora térmica de cupom, leitor, gaveta. Ver seção 3 | **Dono** |
| Homologação real da NF-e e NFC-e | Status, autorização, consulta, cancelamento, QR Code. Prova endereços, TLS, ação SOAP, `urlChave` e regras por UF | Depende do A1 |
| Impressão física | Cada marca e linguagem, densidade, corte, gaveta, leitura por leitor real. Há um relato de falha ao carregar imagem na L42 Pro; se acontecer, é preciso um modo com fontes e barras nativas | Depende do hardware |
| Windows e Linux como alvo real | Caixa de loja costuma ser Windows. Teclas, impressão, SQLite, instalador, em máquina real | **Dono**: só ele testa nesses sistemas |
| Pagamento real | Contrato ou credenciamento com pelo menos um provedor, e homologação dele. Ligar o crate à tela do caixa | **Contrato** e **dono**: qual provedor primeiro |

### Bloco B: o fiscal que um cliente precisa no primeiro dia

| Item | O que é | De quem depende |
|---|---|---|
| UFs | Hoje a tabela só tem SP, RS, PR, MG, SC, RJ, ES e DF, em homologação. Escolher por onde começar, conferir endereços e regras de cada uma | **Dono**: quais UFs no lançamento |
| Liberar produção | Abrir `tpAmb=1`, com bloqueios próprios, depois de homologar. Hoje não existe caminho no código | **Dono**, depois de homologar |
| Contingência da NFC-e (offline) | Emitir sem internet, guardar e transmitir depois, com regras de prazo. Sem isso, a loja para quando a internet cai | Desenvolvimento. Conferir regra por UF |
| EPEC | Contingência da NF-e modelo 55 | Desenvolvimento |
| Inutilização de numeração | Fora da POC porque está sendo extinta em algumas UFs. Conferir onde ainda vale | Desenvolvimento |
| Carta de correção | Evento da NF-e | Desenvolvimento |
| Manifestação do destinatário | Para quem recebe NF-e, e para a entrada de mercadoria | Desenvolvimento. Decidir se entra na primeira versão: **dono** |
| CRT 3 e IBS/CBS | Regime Normal e reforma tributária. CRT 3 exige `IBSCBS` em homologação desde 01/07/2026, segundo fonte secundária. A obrigatoriedade para CRT 1 a partir de 2027 não foi conferida | **Dono**: atender só Simples no início ou não. Contador para a regra |
| Lei 12.741 (`vTotTrib`) | Lacuna declarada em `REGRAS.md` | Desenvolvimento |
| NFS-e | Ainda não tem código. A pesquisa e cinco decisões do dono estão em [`NFSE.md`](NFSE.md) | **Dono** |
| Fim do autorizador assíncrono | Já só usamos o síncrono. Acompanhar notas técnicas, porque o leiaute continua mudando | Contínuo |

### Bloco C: a loja funcionando no dia a dia

| Item | O que é | De quem depende |
|---|---|---|
| Abertura e fechamento de caixa, sangria e suprimento | Não existem. Sem isso o caixa não fecha o dia | Desenvolvimento |
| Perfis e permissões do operador | O framework já tem papéis e auditoria (`SecuredApp`); o `mini-pdv` não usa. Definir o que pede senha de gerente: cancelar item, desconto, sangria | Desenvolvimento. Regras: **dono** |
| Estoque, compras e entradas | Parte em desenvolvimento. Falta inventário, ajuste, baixa na venda e relatórios | Desenvolvimento |
| Balança e etiqueta de peso | Código de balança (prefixo 20), etiqueta de peso e leitura de balança no caixa | Desenvolvimento e hardware |
| Relatórios gerenciais | Vendas por dia, por produto, por forma de pagamento. O motor de relatórios já existe | Desenvolvimento |
| SPED e XMLs para o contador | Guardar os XMLs pelo prazo legal, exportar por período. SPED Fiscal e o que mais o contador pedir | Contador define o que é exigido |

### Bloco D: o produto que se instala e se mantém

| Item | O que é | De quem depende |
|---|---|---|
| Instalação | Instalador por sistema, configuração guiada de emitente, A1 e impressora | Desenvolvimento |
| Atualização automática | `family-finance` tem um caminho. O caixa não pode atualizar no meio da venda, e leiaute fiscal muda por data | Desenvolvimento |
| Backup | Do SQLite e dos XMLs, com restauração testada. XML fiscal perdido é problema legal | Desenvolvimento. Onde guardar: **dono** |
| Mais de um caixa na mesma loja | Hoje é um SQLite por máquina. Numeração e estoque compartilhados pedem um servidor local | **Dono**: o que entra na primeira versão |
| Guarda segura do A1 e do CSC | Hoje a senha fica em memória. Falta onde guardar o PFX na máquina do cliente | Desenvolvimento e revisão de segurança |
| Suporte | Canal, horário, acesso remoto, diagnóstico. O relato de bug em `ravenrs-movie` e `sandbox::analisar` podem ajudar | **Dono** |
| LGPD | CPF e endereço de cliente nas notas e no banco. Base legal, prazo de guarda, resposta ao titular, máscara em logs (já há `sandbox::mascara`). Contrato com o cliente: quem é controlador e quem é operador | **Dono** e jurídico |

### Bloco E: o negócio

| Item | O que é | De quem depende |
|---|---|---|
| Licenciamento comercial versus MIT/Apache | O RavenRS é MIT ou Apache-2.0, e continua. O produto precisa de modelo próprio: repositório separado, licença comercial, chave de ativação. Decidir o que fica aberto. Os crates de `crates/ravenrs-nfe` e afins hoje saem sob MIT/Apache | **Dono** e jurídico |
| Preço, contrato, responsabilidade | Modelo de cobrança e limite de responsabilidade por nota errada ou caixa parado | **Dono** e jurídico |
| Marca | O plano usa o PDV Alterdata só como inspiração de fluxo. O produto precisa de nome e identidade próprios, e o desenho final precisa de revisão para não copiar telas de terceiros | **Dono** |
| Certificações por exigência local | Algumas UFs ou segmentos podem exigir credenciamento de software. Conferir antes de prometer | Jurídico e contador |

## 3. Hardware a comprar para testar

Sem modelos definitivos: a lista diz o que testar, não o que recomendar a clientes. Os modelos abaixo saíram do plano e de anúncios, e a compatibilidade está em [`FONTES.md`](../crates/ravenrs-report-label/FONTES.md) com o grau de confiança.

| Item | Para quê | Observação |
|---|---|---|
| Certificado A1 de CNPJ real | Homologação de NF-e e NFC-e, produção restrita da NFS-e | Depende de contribuinte credenciado |
| Pinpad PPC930 (Gertec) ou equivalente | ABECS e TEF | Confirmar o modelo com o provedor escolhido |
| Etiquetadora | ZPL, EPL/PPLB, PPLA, TSPL | Elgin L42 Pro cobre várias linguagens, segundo anúncios. Testar também uma Zebra ou TSC, se possível |
| Térmica de cupom, 80 mm | ESC/POS, corte, gaveta, QR Code | Conferir largura real em pontos |
| Gaveta de dinheiro | Comando de abrir | Liga na impressora |
| Leitor de código de barras | `ScanField`, ritmo, GTIN | Com fio, em modo teclado |
| Balança, quando chegar o bloco C | Etiqueta de peso | Depois |

## 4. Revisão jurídica e contábil a contratar

Nada disto foi revisado por profissional. As decisões abaixo foram tomadas com cautela, mas não são pareceres.

- **Uso do ACBr (LGPL-2.1+) como referência.** O RavenRS é MIT/Apache, e portar unidade por unidade pode gerar obra derivada. A regra adotada: as regras saem do MOC, dos XSD e das notas técnicas, o ACBr só confere comportamento, e nada foi copiado. `REGRAS.md` e `FONTES.md` registram a origem. Pedir revisão disso antes de vender.
- **Protocolo ABECS.** Conferir licença, uso permitido e se há exigência de certificação para implementar e distribuir. Ver [`PAGAMENTOS.md`](PAGAMENTOS.md).
- **Certificação e PCI.** O crate não captura cartão nem pede senha, e isso deve continuar assim. Confirmar o que cada provedor exige.
- **Troca de NFC-e autorizada por NF-e.** O caixa cancela uma e emite a outra, com aviso de prazo. Precisa de validação de contador antes de ir a qualquer cliente: prazo, motivo, e se a operação é aceita em cada UF.
- **Certificados e dados pessoais.** Guarda do A1 do cliente, LGPD e retenção de XML.
- **Contratos do produto:** licença de uso, suporte, responsabilidade fiscal do cliente e do fornecedor.

## 5. Decisões que só o dono pode tomar

Em ordem de quanto cada uma destrava:

1. Comprar o A1 e o hardware da seção 3, e qual CNPJ usar em homologação.
2. Qual provedor de pagamento primeiro, e o contrato com ele.
3. Quais UFs e quais regimes (só Simples ou também Normal) no lançamento.
4. Se NFS-e e venda mista entram na primeira versão.
5. Um caixa por loja ou vários na primeira versão.
6. Modelo comercial e o que fica aberto sob MIT/Apache.
7. Contratar a revisão jurídica e o contador da seção 4.

## 6. Como manter este documento

Cada linha da seção 1 precisa dizer a verdade sobre como foi verificada. Quando um item for provado de verdade, por exemplo a primeira autorização real, diga o que rodou e quando. Item novo da seção 2 entra com quem decide.
