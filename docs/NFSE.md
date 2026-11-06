# NFS-e Padrão Nacional: pesquisa para o PDV (hoje: 05/11/2026)

> **Situação em 06/11/2026.** O menor escopo da seção 6 virou código: `crates/ravenrs-nfse` (motor e `SefinSandbox`), `crates/ravenrs-dfe` (o crate comum) e a DANFSe A4 em `crates/ravenrs-app-nfe`. O que segue sem confirmação está no README do `ravenrs-nfse`. O caixa ainda não emite NFS-e.

**Legenda.** [C] li na fonte. [S] fonte secundária. [L] lembrança não confirmada.

Páginas web foram lidas por um resumidor. PDFs foram extraídos e lidos por mim. Os XSD 1.01 foram lidos numa cópia do `nfse-php`, não no ZIP oficial.

**Conclusões principais**
- **O órgão não é a SEFAZ.** A obrigação de ME/EPP vem da Resolução CGSN 191/2026 e já vale desde 01/11/2026.
- **O PDV pode emitir por API.** A resolução cita o Emissor Nacional por web ou por API.
- **O DANFSe é nosso.** A API foi suspensa e o tamanho mínimo é A4, então bobina térmica não serve como DANFSe.
- **O algoritmo de assinatura vigente não está confirmado.** Só o primeiro envio real decide.
- **Produção restrita exige certificado.** Segundo fonte secundária, é preciso certificado ICP-Brasil real.

## 1. O que é, quem manda e quem é obrigado

**Órgão e papel da SEFAZ**
- A SEFAZ não é o órgão. Quem publica o modelo é o CGNFS-e: [Res. CGNFS-e 3/2023](https://www.gov.br/nfse/pt-br/biblioteca/portarias-e-resolucoes-cgnfs-e) [C].
- As notas técnicas são assinadas pela SE/CGNFS-e, e o secretário-executivo é auditor municipal de Londrina: [NT 007](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf) [C].
- O único papel estadual que achei: a Res. 6/2025 compartilha dados com as fazendas de MG e RS até a instalação do CGIBS: [lista de resoluções](https://www.gov.br/nfse/pt-br/biblioteca/portarias-e-resolucoes-cgnfs-e) [C].
- Não achei papel de autorização da SEFAZ. "Sefin Nacional" é um módulo do Sistema Nacional, e o nome pode explicar a confusão (inferência minha).
- A NFS-e é gerada e guardada no Ambiente Nacional, operado pela Receita Federal ou por municípios: [portal](https://www.gov.br/nfse/pt-br/municipios/conheca/copy_of_o-que-e-a-nfs-e) [C].
- A composição do comitê (RFB e entidades municipais) ficou em [L]. O rodapé do [Emissor](https://www.nfse.gov.br/EmissorNacional) mostra RFB, Sebrae, CNM, ABRASF e SECEX [C].

**Componentes**
- ADN e Sefin Nacional são integrados, e o Portal dá acesso ao Emissor Web e aos painéis: [Manual Integrado, histórico](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/leiaute-e-esquemas-antigos/manualintegradosnnfse_v1-00-02-producao.pdf) [C].
- A DPS vira NFS-e de forma síncrona. Eventos são documentos XML vinculados à nota: [Manual API](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/manual-contribuintes-emissor-publico-api-sistema-nacional-nfs-e-v1-2-out2025.pdf) [C].
- O DANFSe é o documento auxiliar impresso: [NT 008](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-008-se-cgnfse-danfse-20260714-v1-02.pdf) [C].

**Obrigatoriedade**

| Quem | Regra | Fonte |
|---|---|---|
| Municípios e DF | Desde 01/01/2026: emissão no ambiente nacional, ou emissor próprio com compartilhamento. Vale até 31/12/2032. Sanção: suspensão de transferências voluntárias (§ 7º) | [LC 214, art. 62](https://www.planalto.gov.br/ccivil_03/leis/lcp/lcp214.htm) [C] |
| ME/EPP do Simples | Desde 01/11/2026: NFS-e nacional "emitida pelo Emissor Nacional", por web ou API. Inclui optante pendente. **Vedada em operação sujeita só a ICMS** (§ 1º-B) | [Res. CGSN 191/2026](https://economia.df.gov.br/documents/d/seec/resol-cgsn-n-191-2026-pdf) [C] |
| MEI | Obrigatória para serviço a PJ desde 01/09/2023; facultativa para PF | [Conta Azul](https://contaazul.com/blog/nfse-padrao-nacional/) [S] |
| Lucro Presumido e Real | Sem data federal única; depende do município | [Conta Azul](https://contaazul.com/blog/nfse-padrao-nacional/) [S] |

**Adesão e sistemas próprios**
- O portal informa adesão dos [5.571 entes](https://www.gov.br/nfse/pt-br/municipios/monitoramento-adesoes) [C]. A SE/CGNFS-e ainda cobrava [pendentes em maio](https://www.gov.br/nfse/pt-br/noticias/municipios-sem-adesao-ou-parametrizacao-da-nfs-e-ja-estao-sujeitos-a-bloqueio-de-transferencias) [C].
- ADN e parametrização são obrigatórios; o emissor público é opcional: [nota CTAT/CNM](https://sefa.serra.es.gov.br/arquivosnfse/20251010_CTAT092025.pdf) [S].
- Só São Paulo teria declarado publicamente manter emissor próprio: [Omie](https://ajuda.omie.com.br/pt-BR/articles/12270528-ambiente-nacional-da-nfs-e-o-que-muda-a-partir-de-2026) [S].

**PDV em município sem Emissor Nacional**
- ME/EPP: a CGSN 191 não abre exceção [C]. Porém a API pode rejeitar com [E0039](https://berga.app/base-de-conhecimento/erros-nfse) (município não parametrizado) [S]. Não confirmei como isso se resolve na prática.
- Regime normal: a emissão tende a ser no sistema municipal (inferência, a partir do [S] da tabela). Isso fica fora da POC.
- Recomendo o PDV consultar `/convenio` e bloquear com mensagem clara. O ACBr lê os campos "Aderente ao Ambiente Nacional" e "Aderente ao Emissor Nacional": [provider](https://github.com/joaoduarte19/ACBr/blob/main/Fontes/ACBrDFe/ACBrNFSeX/Provedores/PadraoNacional.Provider.pas) [C].

## 2. API técnica

**Documentação e URLs**
- Índice oficial: [documentação técnica](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica) [C]. A própria página se diz minuta até homologação pelo CGNFS.
- Swagger: [lista oficial](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/apis-prod-restrita-e-producao) [C].
  - Sefin: `sefin.producaorestrita.nfse.gov.br/API/SefinNacional/docs/index` e `sefin.nfse.gov.br/SefinNacional/docs/index`.
  - ADN, parâmetros e DANFSe ficam em `adn[.producaorestrita].nfse.gov.br/{contribuintes|parametrizacao|danfse}/docs/index.html`.
- **Não consegui abrir os Swagger.** As duas tentativas na Sefin deram 403 e a do ADN deu erro de TLS. Portanto schemas JSON e códigos HTTP não vêm da fonte oficial.
- Base de envio, com divergência entre projetos:
  - [ACBr ini](https://raw.githubusercontent.com/joaoduarte19/ACBr/main/Fontes/ACBrDFe/ACBrNFSeX/ACBrNFSeXServicos.ini) [S]: `https://sefin.producaorestrita.nfse.gov.br/SefinNacional` e `https://sefin.nfse.gov.br/sefinnacional`.
  - [nfse-php](https://github.com/pedrocasado/nfse-php/blob/HEAD/src/NFSeService.php) [S]: `.../SefinNacional/nfse` nos dois ambientes.
  - O `/API/` só aparece na URL oficial do Swagger de restrita.

**Formato de envio**
- Mensagens em JSON, documento fiscal em XML: [Manual API](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/manual-contribuintes-emissor-publico-api-sistema-nacional-nfs-e-v1-2-out2025.pdf) [C].
- XML em UTF-8, GZip + Base64, sem prefixo de namespace e sem caracteres de formatação: [Manual Integrado](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/leiaute-e-esquemas-antigos/manualintegradosnnfse_v1-00-02-producao.pdf) [C, histórico].
- Campos JSON, confirmados no código de dois projetos, não em fonte oficial:
  - envio: `dpsXmlGZipB64` e `pedidoRegistroEventoXmlGZipB64`;
  - resposta: `nfseXmlGZipB64` e `eventoXmlGZipB64`;
  - fontes: [ACBr](https://github.com/joaoduarte19/ACBr/blob/main/Fontes/ACBrDFe/ACBrNFSeX/Provedores/PadraoNacional.Provider.pas) e [nfse-php](https://github.com/pedrocasado/nfse-php/blob/HEAD/src/NFSeService.php) [S].
- Erros podem vir em `erro` ou `erros`, com `codigo`/`Codigo`: [nfse-php](https://github.com/pedrocasado/nfse-php) [S].

**mTLS**
- TLS com autenticação mútua; certificado ICP-Brasil A1 ou A3 com CNPJ/CPF no `otherName`; EKU "Autenticação Cliente": [Manual Integrado](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/leiaute-e-esquemas-antigos/manualintegradosnnfse_v1-00-02-producao.pdf) [C, histórico].

**Endpoints** ([Manual API](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/manual-contribuintes-emissor-publico-api-sistema-nacional-nfs-e-v1-2-out2025.pdf) [C])
- `POST /nfse` e `GET /nfse/{chaveAcesso}`.
- `GET /dps/{id}`: devolve a chave, só para atores da nota.
- `HEAD /dps/{id}`: diz se a nota foi gerada.
- `POST` e `GET /nfse/{chave}/eventos[/{tipo}[/{seq}]]`.
- `GET /parametros_municipais/{mun}/convenio`, além de alíquota, retenções e benefícios.
- O manual oficial lista caminhos de parâmetros ambíguos. O [ACBr](https://github.com/joaoduarte19/ACBr/blob/main/Fontes/ACBrDFe/ACBrNFSeX/Provedores/PadraoNacional.Provider.pas) usa `/{serv}/{competência}/aliquota`, `/{competência}/retencoes`, `/{benef}/{competência}/beneficio` e `/{serv}/{competência}/regimes_especiais`, no host do ADN [S].

**DANFSe**
- A API foi suspensa em 03/08/2026; o software emissor gera o documento: [NT 008](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-008-se-cgnfse-danfse-20260714-v1-02.pdf) [C].
- Regras da mesma NT [C]: uma página, retrato, mínimo A4, QR Code de pelo menos 1,52 cm, e "NFS-e SEM VALIDADE JURÍDICA" quando `tpAmb=2`.

**Eventos**
- Cancelamento `e101101`, com `cMotivo` 1 (erro na emissão), 2 (serviço não prestado) ou 9 (outros): [XSD](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia].
- Substituição: DPS com chave a substituir gera o cancelamento automaticamente: [Manual API](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/manual-contribuintes-emissor-publico-api-sistema-nacional-nfs-e-v1-2-out2025.pdf) [C].
- Id do pedido: `PRE[0-9]{56}`, embora a anotação do XSD cite também `nPedRegEvento`. O ACBr usa chave + tipo [S]. Testar antes de fixar.

**Versões** ([produção](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual), [restrita](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/producao-restrita), [RTC](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc) [C])
- Produção: XSD v1.01 de 09/02/2026.
- Restrita: XSD v1.01 de 20/10/2026, ligado à NT 009 v1.01.
- A [página de atualizações](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/atualizacoes-e-implantacoes) não separa os ambientes em 20/10 [C].
- Segundo a [Fenacon](https://fenacon.org.br/reforma-tributaria/nfs-e-nacional-confira-as-orientacoes-sobre-leiaute-em-producao-regras-de-preenchimento-de-ibs-cbs-e-status-da-nota-tecnica-009/), produção segue NT 004 + NT 007 e os releases 1 a 3 da NT 009 entram em 01/12/2026 [S].

**Assinatura: não está confirmada para a versão vigente**
- Único texto oficial encontrado, de manual histórico: enveloped, C14N inclusiva `REC-xml-c14n-20010315`, `rsa-sha1`, `sha1`, EndCertOnly, certificado com a raiz do CNPJ/CPF do emitente: [Manual Integrado V1.00.02](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/leiaute-e-esquemas-antigos/manualintegradosnnfse_v1-00-02-producao.pdf) [C].
- O XSD deixa o `Algorithm` aberto: [xmldsig](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia].
- Projetos divergem:
  - [nfse-php](https://github.com/pedrocasado/nfse-php/blob/HEAD/src/NFSeService.php) usa SHA-256 com C14N inclusiva [C no código];
  - [pynfse](https://github.com/roberto-mello/pynfse-nacional/blob/HEAD/src/pynfse_nacional/xml_signer.py) usa `rsa-sha256` com `exc-c14n#WithComments` [C no código];
  - [nfse-br](https://pypi.org/project/nfse-br/0.1.0/) recusa tratar o SHA-1 como vigente [S].
- Nenhum desses códigos prova aceitação pela Sefin. Não chequei o padrão do ACBr.
- **Consequência: o Signer precisa aceitar SHA-1 e SHA-256 até o primeiro envio real.**
- A referência é `infDPS`, com `Signature` dentro de `DPS`, sem prefixo: [fórum](https://forum.nfsebrasil.com.br/t/erro-e0714-arquivo-enviado-com-erro-na-assinatura/366) [S] e [Campinas](https://groups.google.com/g/wsnfsecampinas/c/uDQEMuO-Qwo) [S].
- Causas relatadas de E0714:
  - namespace herdado fora do digest: [PR](https://github.com/diegociatos/appcafeworking/pull/15) [S];
  - CR em `xDescServ` e quebras de linha: [fórum](https://forum.nfsebrasil.com.br/t/erro-e0714-arquivo-enviado-com-erro-na-assinatura/366) [S].

**Identificadores** ([XSD](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia])
- Id da DPS (45): `"DPS"` + município (7) + tipo de inscrição (1) + inscrição (14) + série (5) + nDPS (15).
- Id da NFS-e (53): `"NFS"` + 50 dígitos = município (7) + ambiente gerador (1) + tipo de inscrição (1) + inscrição (14) + nº NFS-e (13) + AnoMês (4) + código numérico (9) + DV (1).
- Não confirmei o algoritmo do DV. Como o `nNFSe` é atribuído pela Sefin ([NT 007](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf) [C]), a chave deve vir da resposta (inferência).
- O padrão numérico do Id conflita com o CNPJ alfanumérico da [NT 009, item 2.1](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nota-tecnica-009-se-cgnfs-e-v-1-01.pdf) [C]. Conferir no XSD de 20/10, que não li.

**Erros principais** ([catálogo Berga](https://berga.app/base-de-conhecimento/erros-nfse) [S]; a tabela oficial está no Anexo I, xlsx não lido)

| Tema | Códigos |
|---|---|
| Assinatura e certificado | E0714, E0715, E0716, E0718, E1636 |
| Certificado de transmissão | E1200, E1203, E1207 |
| Id | E0004 |
| Duplicidade | E0014, E0802 |
| Ambiente e data | E0006, E0008 |
| Convênio | E0037, E0038, E0039, E0016 |
| Simples | E0160, E0166 |
| Serviço | E0310, E0312, E0314 |
| Alíquota | E0595, E1300 |
| Retenção | E0031, E0580 |
| Tomador | E0190, E0207 |
| Cancelamento | E0822, E0827, E0840 |

## 3. Regras fiscais

**Código do serviço e cadastro**
- `cTribNac`: 6 dígitos (item, subitem e desdobro). `cNBS`: 9 dígitos. `tribISSQN`: 1 tributável, 2 imunidade, 3 exportação, 4 não incidência. `tpRetISSQN`: 1 não retido, 2 pelo tomador; o terceiro valor não confirmei. Tudo em [XSD](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia].
- A lista de códigos está no [portal](https://www.gov.br/nfse/pt-br/mei-e-demais-empresas/codigos-de-tributacao-nacional-nbs) [C]. A correlação oficial é o [Anexo B](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/anexo_b-nbs2-lista_servico_nacional-snnfse-v1-01-20260122.xlsx), não lido.
- A obrigatoriedade da NBS variou por município: [BH](https://notagateway.com.br/blog/nfs-e-nacional-em-belo-horizonte-mg-prefeitura-divulga-parametros-para-emissao-do-issqn/) e [Londrina](https://repositorio.londrina.pr.gov.br/index.php/menu-fazenda/ggf/nfse-nacional/71715-nt-2025-001-implantacao-da-nfse-padrao-nacional/file) [S].

**Simples Nacional**
- `opSimpNac`: 1 não optante, 2 MEI, 3 ME/EPP. `regApTribSN`: 1 tudo pelo SN, 2 ISS por fora, 3 tudo por fora: [XSD](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia].
- `regApTribSN` é usado com `opSimpNac=3`: [pynfse](https://pypi.org/project/pynfse-nacional/) [S].
- A NT 009 acrescenta `opSimpNac=4`, `regApIBSCBSSN` e `cAtvSN`: [item 2.4](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nota-tecnica-009-se-cgnfs-e-v-1-01.pdf) [C].

**Tomador, série e número**
- `toma` é opcional no XSD, e a regra de negócio completa não li (Anexo I). Se informado, o CPF/CNPJ é validado (E0190/E0207) [S].
- `nDPS` começa em 1-9 e tem até 15 dígitos; série tem até 5: [XSD](https://github.com/pedrocasado/nfse-php/tree/HEAD/xsd/1.01) [C na cópia].
- O padrão da série usa `^`/`$`, que o libxml toma como literais: [PR](https://github.com/wagnerra23/oimpresso.com/pull/9221) [S]. Nosso validador precisa tratar isso.
- O Emissor Web tem a faixa de série 80000–89999: [atualizações](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/atualizacoes-e-implantacoes) [C]. **Não confirmei as faixas válidas para API.**
- `nNFSe` é da Sefin, e buracos na sequência não são irregularidade: [NT 007](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-007-se-cgnfse-v1-0.pdf) [C].
- DPS rejeitada permite reaproveitar o número: [Berga, E0014](https://berga.app/base-de-conhecimento/erros-nfse) [S].

**IBS/CBS**
- NFS-e em 01/10/2026; exceções em 01/12/2026; Simples em 01/01/2027: [Conta Azul](https://contaazul.com/blog/nfse-padrao-nacional/) e [CRC-SP](https://online.crcsp.org.br/portal/noticias/noticia.asp?c=11276) [S].
- Grupo ausente não causa rejeição; se informado, os códigos ficam obrigatórios: [Fenacon](https://fenacon.org.br/reforma-tributaria/nfs-e-nacional-confira-as-orientacoes-sobre-leiaute-em-producao-regras-de-preenchimento-de-ibs-cbs-e-status-da-nota-tecnica-009/) [S].
- A NT 009 traz `finNFSe`, `vAjusteBC` e o reposicionamento de `CST`/`cClassTrib`: [NT 009](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nota-tecnica-009-se-cgnfs-e-v-1-01.pdf) [C].
- Não li o texto do [Ato Conjunto RFB/CGIBS 4/2026](https://www.bmalaw.com.br/conteudo/direito-tributario/reforma-tributaria/reforma-tributaria-cronograma-para-emissao-de-documentos-fiscais-eletronicos/pdf). Só secundárias, com divergência de data entre [Contmatic](https://simplifique.contmatic.com.br/blogs/obrigatoriedade-ibs-cbs-documentos-fiscais-4-ondas-2026) e [Ozai](https://www.ozai.com.br/principais-providencias-sobre-cbs-e-ibs-a-partir-de-2026/).

**Loja mista**
- NFS-e é vedada a ME/EPP em operação só de ICMS: [CGSN 191](https://economia.df.gov.br/documents/d/seec/resol-cgsn-n-191-2026-pdf) [C].
- O produto continua em NF-e: [Fique atento](https://amdjus.com.br/?p=24826) [S].
- Na prática, uma venda mista gera dois documentos (inferência): dois números, dois retornos e possibilidade de falha parcial.
- Não confirmei regra sobre nota conjugada, nem caso de serviço com fornecimento de mercadoria. Pedir validação de contador.

## 4. Referências

| Projeto | Licença | Uso |
|---|---|---|
| [ACBr PadraoNacional](https://github.com/joaoduarte19/ACBr/blob/main/Fontes/ACBrDFe/ACBrNFSeX/Provedores/PadraoNacional.Provider.pas) | LGPL segundo o pedido; a API do GitHub não informou | Só conferir comportamento. O ini aponta homologação em NT `009.1.01` e `Versao=1.01`; há também [GravarXml](https://github.com/joaoduarte19/ACBr/blob/main/Fontes/ACBrDFe/ACBrNFSeX/Provedores/PadraoNacional.GravarXml.pas) |
| [NFSeNacionalSdk](https://github.com/higordiasz/nfse_nacional_sdk_dotnet) (.NET) | MIT | Prévia; a melhor referência permissiva. Não li o código |
| [nfse-br](https://pypi.org/project/nfse-br/0.1.0/) (Python) | MIT | Validação local; diz não verificar aceitação pela SEFIN |
| [nfse-php](https://github.com/pedrocasado/nfse-php) | LGPL/GPL/MIT no README | Fluxo e XSD |
| [pynfse-nacional](https://pypi.org/project/pynfse-nacional/) | AGPL-3.0 | Só leitura; não copiar |
| [nfe-parser](https://docs.rs/nfe-parser) (Rust) | não verificada | NFS-e ABRASF municipal, não a nacional |

Não achei crate Rust para a NFS-e nacional, mas não fiz busca direta no crates.io. A busca foi [genérica na web](https://docs.rs/nfe-parser) [S].

## 5. Ambiente de teste

- **Certificado real:** provável sim. A [Nota Gateway](https://notagateway.com.br/blog/api-nfse-nacional/) diz que restrita usa ICP-Brasil do emitente [S]. Não achei AC de teste oficial, e nenhuma fonte oficial trata o caso de forma explícita.
- **CNPJ real:** provável sim. A geração integra CNC, CNPJ, CPF e Simples: [Manual API](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/documentacao-atual/manual-contribuintes-emissor-publico-api-sistema-nacional-nfs-e-v1-2-out2025.pdf) [C]. E0718 exige assinatura do próprio emitente [S]. Não sei se o cadastro de restrita espelha o de produção.
- **Município:** E0037/E0038/E0039 sugerem convênio ativo e parametrizado [S]. A aplicação em restrita não foi confirmada. Testar com o município do CNPJ de teste.
- **Efeito fiscal:** nenhum em restrita: [Fenacon](https://fenacon.org.br/reforma-tributaria/nfs-e-nacional-confira-as-orientacoes-sobre-leiaute-em-producao-regras-de-preenchimento-de-ibs-cbs-e-status-da-nota-tecnica-009/) [S]. Combina com o aviso da [NT 008](https://www.gov.br/nfse/pt-br/biblioteca/documentacao-tecnica/rtc/nt-008-se-cgnfse-danfse-20260714-v1-02.pdf) [C].
- **Simulador oficial:** não encontrei. Isso é ausência de resultado, não prova de que não exista.
- **Provável sem certificado:** Id, ordem e XSD, GZip/Base64, C14N e assinatura com `xmllint`/`openssl`, fluxos no sandbox, layout do DANFSe.
- **Só com envio real:** algoritmo aceito, mTLS, formato real do JSON e de erros, regras de negócio, faixa de série.

## 6. Recomendação

Li `transporte.rs`, `assinatura.rs`, a primeira parte de `sandbox/mod.rs` e o `README.md`. Em `c14n.rs` e `http.rs` conferi só trechos.

**Ir para um crate comum** (regra da casa: nada de cópia entre motores)
- `xml`, `c14n`, `HttpTransport` com PFX, `Transport` e o mecanismo de handlers do sandbox (`use_once`, `on_unhandled`, `requests`).
- `verify_enveloped` e `sign_enveloped`, depois de parametrizados.

**O que muda**
1. **Método HTTP.** `HttpRequest` diz "sempre POST" e `http.rs:207` chama `.post`. É preciso método GET e HEAD.
2. **Assinatura.** `Signer::sign_rsa_sha1` e as constantes passam a receber o algoritmo; o `rsa` ganha `sha2`. O `Signer` também não pode depender de `NfeError`.
3. **Sandbox.** `Handler` e `NaoTratado` são presos à ação SOAP. Passam a casar método + caminho, e a parte SOAP fica em `ravenrs-nfe`.
4. **Novos.** JSON e GZip compatíveis com `wasm32`. O crate continua sem rede em wasm.
5. **Regra de resposta.** HTTP e erros passam a ser lidos por código, não por `cStat`.

**Novo crate `ravenrs-nfse`, menor POC**
- Prestador ME/EPP (`opSimpNac=3`, `regApTribSN=1`), `tpAmb=2` apenas, com `BetaAck` e produção bloqueada, como na NF-e.
- Um serviço por DPS, ISS sem retenção e `tribISSQN=1`; tomador opcional.
- `POST /nfse`, `HEAD` e `GET /dps/{id}`, `GET /nfse/{chave}`, cancelamento `e101101`, convênio e alíquota.
- **Recuperação depois de timeout** por `/dps/{id}`, usando o mesmo Id. Essa é a parte que mais importa num caixa.
- DANFSe A4 por `ravenrs-report`, com QR e a tarja de homologação. O cupom térmico vira só comprovante não fiscal, a menos que o dono decida outra coisa.
- Layout: como a NT 009 está prevista para 01/12/2026, recomendo builder com versão selecionável e foco na NT 009. Esse cronograma é [S] e o oficial ainda não saiu.
- Fora da POC: substituição, retenção, deduções, intermediário, exterior, obra, ajustes.

**Cenários do sandbox**
- Sucesso com `nfseXmlGZipB64`.
- **Nota gerada com resposta perdida**, recuperada por `HEAD` + `GET`.
- E0014 e E0802, E0714, E0004, E0006, E0008, E0037/E0038/E0039, E0160/E0166, E0310/E0312, E0595.
- E0718, HTTP 403 de mTLS, certificado vencido, E0822/E0840, indisponibilidade 5xx, Base64 ou GZip inválido.
- Conferência padrão: descompactar, validar Id, ordem, `tpAmb=2` e assinatura.
- **Ressalva:** o formato do corpo de erro vem de projetos terceiros. É fixture provisória até haver captura real.

**Decisões do dono do produto**
1. Comprar ou usar um certificado A1 e um CNPJ reais para a produção restrita. Sem isso não há prova real.
2. Atender só Simples na primeira versão (recomendo) ou também Lucro Presumido/Real, com provedores municipais.
3. Definir o que o caixa imprime para serviço: A4, PDF enviado ao cliente ou cupom não fiscal.
4. Regra comercial para venda mista: dois documentos e o que fazer em falha parcial, validada por contador.
5. Lista de municípios atendidos no lançamento.

## O que não consegui confirmar

- Swagger, schemas JSON e códigos HTTP oficiais (403 e erro de TLS).
- Algoritmo de assinatura vigente e faixas de série por API.
- DV da chave.
- ZIPs XSD e planilhas `.xlsx` oficiais (Anexos I, B, C, VI); não abri por causa do pedido de não baixar.
- [NT MEI 2023-001](https://www.gov.br/nfse/pt-br/nt-2023-001-obrigatoriedade-da-nfs-e-nacional-para-o-mei.pdf): baixada pela ferramenta, texto não lido.
- FAQ oficial: pede login.
- Composição do CGNFS-e.
- Texto do Ato Conjunto 4/2026.
- Regra de nota conjugada.

## Aviso sobre arquivos

O pedido era não baixar nada, mas o `WebFetch` salvou sozinho cópias de PDFs (e o `.ini` do ACBr, como `.bin`) em `/Users/alexkads/.claude/projects/-Users-alexkads-Projects-RavenRS--claude-worktrees-nfe-beta-mini-pdv/7b939497-c1f1-4853-8d19-10fbf6be0534/tool-results/`. Li os PDFs de lá.

Nada além deste documento foi alterado no repositório. Nenhum certificado ou dado fiscal foi enviado.
