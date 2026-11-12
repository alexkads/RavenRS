//! The one catalog of rules. Both the sandbox and the diagnosis read from it.
//!
//! Every rule says where it came from and how sure that is. "Lida na fonte" means the text was
//! read in that document during this POC, not that a real server was seen applying it.

use std::fmt;

use crate::retorno::descricao_c_stat;

/// Where a rule came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fonte {
    /// A schema of the layout package, read and validated with `xmllint`.
    Xsd(&'static str),
    /// The Manual de Orientação do Contribuinte 7.0 (Anexo I or Visão Geral).
    Moc(&'static str),
    /// A technical note.
    Nt(&'static str),
    /// A norm of ICP-Brasil or a recommendation like XMLDSig.
    Norma(&'static str),
    /// Behavior read in the ACBr mirror, with no code copied.
    Acbr(&'static str),
    /// A third party's page, which does not replace the official text.
    Secundaria(&'static str),
    /// A decision of this sandbox, with no fiscal source behind it.
    Sandbox(&'static str),
    /// The author's memory, with no source checked.
    Memoria(&'static str),
}

impl fmt::Display for Fonte {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xsd(texto) => write!(f, "XSD: {texto}"),
            Self::Moc(texto) => write!(f, "MOC 7.0: {texto}"),
            Self::Nt(texto) => write!(f, "NT: {texto}"),
            Self::Norma(texto) => write!(f, "Norma: {texto}"),
            Self::Acbr(texto) => write!(f, "Comportamento do ACBr: {texto}"),
            Self::Secundaria(texto) => write!(f, "Fonte secundária: {texto}"),
            Self::Sandbox(texto) => write!(f, "Decisão do sandbox: {texto}"),
            Self::Memoria(texto) => write!(f, "Memória: {texto}"),
        }
    }
}

/// How much a rule can be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Confianca {
    /// The text of the rule was read in the source named.
    LidaNaFonte,
    /// The rule follows from a source, but the source does not say it in these terms.
    Inferida,
    /// From memory; to be checked before trusting.
    DeMemoria,
}

impl fmt::Display for Confianca {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LidaNaFonte => "regra lida na fonte",
            Self::Inferida => "regra inferida",
            Self::DeMemoria => "de memória",
        })
    }
}

/// Whether a rule stops the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Efeito {
    /// The server refuses.
    Rejeicao,
    /// Worth attention, but the document goes on.
    Alerta,
}

/// What it takes to tell whether a rule breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alcance {
    /// The message alone says it.
    Mensagem,
    /// Needs the moment of the receipt.
    Relogio,
    /// Needs what the server already holds, such as issued keys.
    EstadoDaSefaz,
    /// Needs the taxpayer registry or the trust list of the server.
    Cadastro,
    /// Depends on the service being up, not on the message.
    Servico,
}

/// One rule of the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Regra {
    /// The rule id in its source, or one of ours starting with `SBX-`.
    pub id: &'static str,
    /// The code answered; 0 when nothing reaches the server.
    pub c_stat: u32,
    /// Whether it stops the document.
    pub efeito: Efeito,
    /// What it takes to tell.
    pub alcance: Alcance,
    /// What goes wrong, in pt-BR.
    pub explicacao: &'static str,
    /// What to do, in pt-BR.
    pub como_corrigir: &'static str,
    /// Where it came from.
    pub fonte: Fonte,
    /// How sure that is.
    pub confianca: Confianca,
}

impl Regra {
    /// The name of the code, from the code table.
    #[must_use]
    pub fn titulo(&self) -> &'static str {
        descricao_c_stat(self.c_stat).unwrap_or("Sem código de retorno: não chega à SEFAZ")
    }
}

const fn lida(
    id: &'static str,
    c_stat: u32,
    alcance: Alcance,
    fonte: Fonte,
    explicacao: &'static str,
    como_corrigir: &'static str,
) -> Regra {
    Regra {
        id,
        c_stat,
        efeito: Efeito::Rejeicao,
        alcance,
        explicacao,
        como_corrigir,
        fonte,
        confianca: Confianca::LidaNaFonte,
    }
}

const fn inferida(
    id: &'static str,
    c_stat: u32,
    fonte: Fonte,
    explicacao: &'static str,
    como_corrigir: &'static str,
) -> Regra {
    Regra {
        id,
        c_stat,
        efeito: Efeito::Rejeicao,
        alcance: Alcance::Mensagem,
        explicacao,
        como_corrigir,
        fonte,
        confianca: Confianca::Inferida,
    }
}

use Alcance::{Cadastro, EstadoDaSefaz, Mensagem, Relogio, Servico};
use Fonte::{Moc, Nt, Xsd};

/// All the rules, in the order a server checks them inside each service.
pub const CATALOGO: &[Regra] = &[
    // Envelope and message.
    inferida(
        "SBX-SOAP",
        0,
        Fonte::Sandbox("o MOC descreve o SOAP, mas não o que um servidor responde a envelope errado"),
        "O envelope não chegou como SOAP 1.2 do serviço pedido, então a mensagem nem foi lida.",
        "Envie application/soap+xml com a ação do serviço no Content-Type, e nfeDadosMsg no namespace dele.",
    ),
    lida(
        "B02",
        243,
        Mensagem,
        Moc("Anexo I, 4.1, grupo B, B02"),
        "O texto recebido não é um XML bem formado.",
        "Confira se o XML chegou inteiro e se o que foi montado à mão escapa & e <.",
    ),
    lida(
        "B06",
        239,
        Mensagem,
        Moc("Anexo I, 4.1, grupo B, B06"),
        "A versão informada no atributo versao não é aceita por este serviço.",
        "Use versao=4.00 na NF-e e 1.00 no evento.",
    ),
    lida(
        "D01a-LOTE",
        565,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01a"),
        "A mensagem de autorização não tem a tag raiz enviNFe.",
        "Coloque a NFe dentro de enviNFe, com idLote e indSinc.",
    ),
    lida(
        "D01a",
        516,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01a"),
        "A mensagem não tem a tag raiz esperada para este serviço.",
        "Confira se a mensagem foi enviada ao serviço certo.",
    ),
    lida(
        "D01b",
        517,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01b"),
        "Falta o atributo versao na tag raiz da mensagem.",
        "Informe versao na tag raiz.",
    ),
    lida(
        "D01d",
        587,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01d"),
        "Há um namespace diferente do padrão da NF-e na mensagem.",
        "Declare só xmlns=\"http://www.portalfiscal.inf.br/nfe\"; xsi e xsd costumam vir do serializador.",
    ),
    lida(
        "D01e",
        588,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01e"),
        "Há espaços, quebras de linha ou BOM entre as tags ou nas pontas da mensagem.",
        "Gere o XML sem formatação e sem BOM, e não o reformate depois de assinar.",
    ),
    lida(
        "D01-LOTE",
        225,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01 (autorização)"),
        "O lote não passa no schema.",
        "Valide a NF-e contra nfe_v4.00.xsd para ver a tag exata.",
    ),
    lida(
        "D01",
        215,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D01"),
        "A mensagem não passa no schema do serviço.",
        "Valide a mensagem contra o XSD do serviço.",
    ),
    lida(
        "XSD-ORDEM",
        225,
        Mensagem,
        Xsd("leiauteNFe_v4.00.xsd, sequência das tags"),
        "Esta tag está fora da ordem do leiaute.",
        "Escreva as tags na ordem do leiaute; a ordem faz parte do schema.",
    ),
    lida(
        "XSD-DESCONHECIDA",
        225,
        Mensagem,
        Xsd("leiauteNFe_v4.00.xsd"),
        "Esta tag não faz parte deste grupo no leiaute.",
        "Remova a tag ou coloque-a no grupo certo.",
    ),
    lida(
        "XSD-OBRIGATORIA",
        225,
        Mensagem,
        Xsd("leiauteNFe_v4.00.xsd"),
        "Falta uma tag obrigatória neste grupo.",
        "Informe a tag.",
    ),
    lida(
        "XSD-VALOR",
        225,
        Mensagem,
        Xsd("leiauteNFe_v4.00.xsd"),
        "O valor não tem a forma que o schema pede.",
        "Corrija o valor conforme o tipo do campo.",
    ),
    // Certificate used to transmit. The sandbox has no TLS, so it reads these on the signature's.
    lida(
        "A01",
        280,
        Mensagem,
        Moc("Anexo I, 4.1, grupo A, A01; Visão Geral, 4.2.3"),
        "O certificado não permite Autenticação Cliente (Extended Key Usage), necessária para transmitir.",
        "Use um e-CNPJ com Autenticação Cliente, ou transmita com outro certificado que tenha.",
    ),
    lida(
        "A02",
        281,
        Relogio,
        Moc("Anexo I, 4.1, grupo A, A02"),
        "O certificado de transmissão está fora da validade.",
        "Renove o certificado A1 ou A3.",
    ),
    lida(
        "A03",
        283,
        Cadastro,
        Moc("Anexo I, 4.1, grupo A, A03"),
        "A cadeia do certificado de transmissão não fecha.",
        "Informe a cadeia completa da AC emissora.",
    ),
    lida(
        "A04",
        286,
        Cadastro,
        Moc("Anexo I, 4.1, grupo A, A04"),
        "Não foi possível consultar a lista de revogação do certificado de transmissão.",
        "Tente de novo; se persistir, confira o endereço da LCR no certificado.",
    ),
    lida(
        "A05",
        284,
        Cadastro,
        Moc("Anexo I, 4.1, grupo A, A05"),
        "O certificado de transmissão está revogado.",
        "Emita um certificado novo.",
    ),
    lida(
        "A06",
        285,
        Cadastro,
        Moc("Anexo I, 4.1, grupo A, A06"),
        "A raiz do certificado de transmissão não é a ICP-Brasil.",
        "Use um certificado ICP-Brasil.",
    ),
    lida(
        "A07",
        282,
        Mensagem,
        Moc("Anexo I, 4.1, grupo A, A07"),
        "Falta o CNPJ (OID 2.16.76.1.3.3) ou o CPF (OID 2.16.76.1.3.1) no certificado de transmissão.",
        "Use um e-CNPJ ou e-CPF de verdade.",
    ),
    // Certificate of the signature.
    lida(
        "E01",
        290,
        Mensagem,
        Moc("Anexo I, 4.1, grupo E, E01"),
        "O certificado da assinatura é inválido: versão diferente de 3, certificado de AC, ou uso da chave sem Assinatura Digital e Não Recusa.",
        "Use o certificado de pessoa jurídica, não o da AC, e confira o uso da chave.",
    ),
    inferida(
        "CERT-CHAVE-FRACA",
        290,
        Moc("Visão Geral, 4.2.6 cita 1024 bits; o limite de 2048 vem da ICP-Brasil, não conferido na norma"),
        "A chave do certificado tem menos de 2048 bits. O MOC 7.0 ainda cita 1024, mas certificados atuais não usam essas chaves.",
        "Emita um certificado com chave de 2048 bits ou mais.",
    ),
    inferida(
        "CERT-SHA1",
        290,
        Fonte::Sandbox("nenhuma regra do MOC cita o hash do certificado; só a assinatura do XML é SHA-1"),
        "O próprio certificado foi assinado pela AC com SHA-1. Não confundir com a assinatura do XML, que é SHA-1 de propósito.",
        "Emita um certificado assinado com SHA-256.",
    ),
    lida(
        "E02",
        291,
        Relogio,
        Moc("Anexo I, 4.1, grupo E, E02"),
        "O certificado da assinatura está vencido ou ainda não vale na data verificada.",
        "Renove o certificado; confira também o relógio da máquina.",
    ),
    lida(
        "E03",
        292,
        Mensagem,
        Moc("Anexo I, 4.1, grupo E, E03"),
        "Falta o CNPJ (OID 2.16.76.1.3.3) ou o CPF (OID 2.16.76.1.3.1) no certificado da assinatura.",
        "Use um e-CNPJ ou e-CPF de verdade.",
    ),
    lida(
        "E04",
        293,
        Cadastro,
        Moc("Anexo I, 4.1, grupo E, E04"),
        "A cadeia do certificado da assinatura não fecha: ele não foi assinado pela AC que diz.",
        "Reinstale o certificado; se ele veio de outro lugar, emita de novo.",
    ),
    lida(
        "E05",
        296,
        Cadastro,
        Moc("Anexo I, 4.1, grupo E, E05"),
        "Não foi possível consultar a lista de revogação do certificado da assinatura.",
        "Tente de novo; se persistir, confira o endereço da LCR no certificado.",
    ),
    lida(
        "E06",
        294,
        Cadastro,
        Moc("Anexo I, 4.1, grupo E, E06"),
        "O certificado da assinatura está revogado.",
        "Emita um certificado novo.",
    ),
    lida(
        "E07",
        295,
        Cadastro,
        Moc("Anexo I, 4.1, grupo E, E07"),
        "A raiz do certificado da assinatura não é a ICP-Brasil.",
        "Use um certificado ICP-Brasil.",
    ),
    inferida(
        "SBX-FORA-DO-LABORATORIO",
        293,
        Fonte::Sandbox("o sandbox só confia na própria AC de teste"),
        "O sandbox recebeu um certificado que não veio da AC de teste. Certificado real não entra no laboratório.",
        "Use SandboxSigner ou GeradorCertificado no sandbox, e deixe o certificado real para a homologação.",
    ),
    Regra {
        id: "LOCAL-PIN",
        c_stat: 0,
        efeito: Efeito::Rejeicao,
        alcance: Mensagem,
        explicacao: "O token recusou o PIN, ou está bloqueado. Nada foi assinado e nada chegou à SEFAZ.",
        como_corrigir: "Peça o PIN de novo sem tentar sozinho: tentativas erradas bloqueiam o token.",
        fonte: Fonte::Sandbox("comportamento do TokenSimulado; o limite real depende do fabricante"),
        confianca: Confianca::Inferida,
    },
    // Signature.
    lida(
        "F01",
        298,
        Mensagem,
        Moc("Anexo I, 4.1, grupo F, F01; Visão Geral, 4.2.4"),
        "A assinatura não segue o padrão: Reference URI, Transforms ou algoritmos diferentes dos previstos.",
        "Assine com Reference URI igual a #Id, Transforms enveloped e C14N, e os algoritmos da Visão Geral 4.2.4.",
    ),
    inferida(
        "SIG-SHA256",
        298,
        Moc("Visão Geral, 4.2.4 lista só rsa-sha1 e sha1; o código da recusa é inferido de F01"),
        "A assinatura usa SHA-256, mas o padrão da NF-e é RSA-SHA1 com digest SHA-1.",
        "Configure o assinador para SHA-1, mesmo que outros documentos usem SHA-256.",
    ),
    inferida(
        "F02-DIGEST",
        297,
        Moc("Anexo I, F02 fala do SignatureValue; o digest cair aqui é inferido"),
        "O DigestValue informado não confere com o conteúdo: o XML mudou depois de assinado, ou foi assinado de outra forma.",
        "Assine por último e não altere o XML depois. Veja a autópsia para a causa provável.",
    ),
    lida(
        "F02",
        297,
        Mensagem,
        Moc("Anexo I, 4.1, grupo F, F02"),
        "O SignatureValue não confere com o certificado informado.",
        "Assine o SignedInfo canonizado com a chave do certificado que vai no XML.",
    ),
    lida(
        "F03",
        213,
        Mensagem,
        Moc("Anexo I, 4.1, grupo F, F03"),
        "O CNPJ-base do certificado é diferente do CNPJ-base do emitente.",
        "Assine com o certificado da empresa emitente; filial e matriz têm a mesma base.",
    ),
    lida(
        "F03A",
        227,
        Mensagem,
        Moc("Anexo I, 4.1, grupo F, F03A"),
        "O CPF do certificado é diferente do CPF do emitente.",
        "Assine com o certificado do próprio emitente.",
    ),
    // Batch.
    lida(
        "GAP03a-3",
        452,
        Relogio,
        Nt("NT 2025.001 v1.02, GAP03a-3"),
        "Foi pedida resposta assíncrona para um lote com uma nota só.",
        "Envie com indSinc=1. Este crate só envia assim.",
    ),
    lida(
        "GAP03a-6",
        129,
        Relogio,
        Nt("NT 2023.002 v1.02, GAP03a-6"),
        "A autorização assíncrona foi extinta: toda autorização precisa ser síncrona.",
        "Envie com indSinc=1. Vigência na tabela de vigências.",
    ),
    lida(
        "GAP03a-4",
        126,
        Mensagem,
        Nt("NT 2023.002 v1.02, GAP03a-4"),
        "O lote traz mais de uma NFC-e.",
        "Envie uma NFC-e por lote.",
    ),
    // Document.
    lida(
        "A03-10",
        502,
        Mensagem,
        Moc("Anexo I, 4.2.1, A03-10"),
        "A chave no Id não é a concatenação dos campos da própria nota.",
        "Monte a chave a partir dos campos, sem editar nenhum depois.",
    ),
    lida(
        "B03-10",
        897,
        Mensagem,
        Moc("Anexo I, 4.2.1, B03-10"),
        "O cNF está na lista de valores proibidos, ou é igual ao nNF.",
        "Use um código numérico aleatório, fora da lista.",
    ),
    lida(
        "B09-10",
        703,
        Relogio,
        Moc("Anexo I, 4.2.1, B09-10"),
        "A emissão está mais de 5 minutos à frente do horário de recebimento.",
        "Sincronize o relógio da máquina (NTP) e confira o fuso.",
    ),
    lida(
        "B09-40",
        704,
        Relogio,
        Moc("Anexo I, 4.2.1, B09-40"),
        "A NFC-e normal chegou mais de 5 minutos depois da hora de emissão.",
        "Emita na hora da venda; na falta de rede, use a contingência da UF. A tolerância pode variar por UF.",
    ),
    lida(
        "B11a-10",
        707,
        Mensagem,
        Moc("Anexo I, 4.2.1, B11a-10"),
        "NFC-e só pode ser operação interna (idDest=1).",
        "Use NF-e para operação interestadual.",
    ),
    lida(
        "B12-20",
        271,
        Mensagem,
        Moc("Anexo I, 4.2.1, B12-20"),
        "O município do fato gerador não é da UF do emitente.",
        "Confira o código IBGE do município.",
    ),
    lida(
        "B21-10",
        709,
        Mensagem,
        Moc("Anexo I, 4.2.1, B21-10"),
        "A NFC-e só aceita tpImp 4 ou 5.",
        "Use tpImp=4.",
    ),
    lida(
        "B21-20",
        710,
        Mensagem,
        Moc("Anexo I, 4.2.1, B21-20"),
        "A NF-e não aceita tpImp 4 nem 5.",
        "Use tpImp=1.",
    ),
    lida(
        "B23-10",
        253,
        Mensagem,
        Moc("Anexo I, 4.2.1, B23-10"),
        "O dígito verificador da chave não confere com os outros 43 caracteres.",
        "Calcule o cDV em módulo 11; no CNPJ alfanumérico o valor é o ASCII menos 48.",
    ),
    lida(
        "B24-10",
        252,
        Mensagem,
        Moc("Anexo I, 4.2.1, B24-10"),
        "O tpAmb da nota é diferente do ambiente que recebeu a mensagem.",
        "Envie tpAmb=2 para homologação. Este crate nunca envia tpAmb=1.",
    ),
    lida(
        "B25-20",
        715,
        Mensagem,
        Moc("Anexo I, 4.2.1, B25-20"),
        "A NFC-e só aceita finalidade normal (finNFe=1).",
        "Use NF-e para complemento, ajuste e devolução.",
    ),
    lida(
        "B25a-10",
        716,
        Mensagem,
        Moc("Anexo I, 4.2.1, B25a-10"),
        "A NFC-e só serve para consumidor final (indFinal=1).",
        "Use NF-e para quem não é consumidor final.",
    ),
    lida(
        "B25b-20",
        717,
        Mensagem,
        Moc("Anexo I, 4.2.1, B25b-20"),
        "A NFC-e só aceita indPres 1 ou 4.",
        "Use NF-e para venda não presencial.",
    ),
    lida(
        "C02-10",
        207,
        Mensagem,
        Moc("Anexo I, 4.2.1, C02-10"),
        "O CNPJ do emitente tem dígito verificador inválido.",
        "Corrija o cadastro do emitente.",
    ),
    lida(
        "E01-10",
        719,
        Mensagem,
        Moc("Anexo I, 4.2.1, E01-10"),
        "A NF-e precisa identificar o destinatário.",
        "Informe o grupo dest.",
    ),
    lida(
        "E04-10",
        724,
        Mensagem,
        Moc("Anexo I, 4.2.1, E04-10"),
        "A NF-e precisa do nome do destinatário.",
        "Informe dest/xNome.",
    ),
    lida(
        "E04-20",
        598,
        Mensagem,
        Moc("Anexo I, 4.2.1, E04-20"),
        "Em homologação, o nome do destinatário tem de ser o texto fixo, em NF-e e em NFC-e.",
        "Use o texto da regra em dest/xNome quando tpAmb=2. Este crate já faz isso.",
    ),
    lida(
        "E05-10",
        726,
        Mensagem,
        Moc("Anexo I, 4.2.1, E05-10"),
        "A NF-e precisa do endereço do destinatário.",
        "Informe dest/enderDest.",
    ),
    lida(
        "E16a-10",
        789,
        Mensagem,
        Moc("Anexo I, 4.2.1, E16a-10"),
        "NFC-e só aceita destinatário não contribuinte (indIEDest=9).",
        "Use NF-e para contribuinte de ICMS.",
    ),
    lida(
        "I04-10",
        373,
        Mensagem,
        Moc("Anexo I, 4.2.1, I04-10"),
        "Em homologação, a NFC-e precisa do texto fixo na descrição do primeiro item.",
        "Use o texto da regra no primeiro xProd quando tpAmb=2. Este crate já faz isso.",
    ),
    lida(
        "I08-150",
        725,
        Mensagem,
        Moc("Anexo I, 4.2.1, I08-150; NT 2023.003 v1.40 acrescenta 5.949 em casos da UF"),
        "Este CFOP não é aceito em NFC-e.",
        "Use um CFOP da lista da regra, como 5.102 ou 5.405.",
    ),
    lida(
        "I11-10",
        629,
        Mensagem,
        Moc("Anexo I, 4.2.1, I11-10, tolerância (*4) de R$ 0,01"),
        "vProd difere de qCom vezes vUnCom em mais de um centavo.",
        "Calcule vProd com a mesma quantidade e o mesmo preço que vão no XML.",
    ),
    lida(
        "I11-20",
        630,
        Mensagem,
        Moc("Anexo I, 4.2.1, I11-20, tolerância (*4) de R$ 0,01"),
        "vProd difere de qTrib vezes vUnTrib em mais de um centavo.",
        "Confira a unidade tributável e o seu preço.",
    ),
    lida(
        "N23-10",
        806,
        Mensagem,
        Moc("Anexo I, 4.2.1, N23-10"),
        "Operação com substituição tributária (CSOSN 500, por exemplo) sem CEST.",
        "Informe o CEST do produto.",
    ),
    lida(
        "UB12-10",
        1115,
        Mensagem,
        Nt("NT 2025.002 v1.53, UB12-10"),
        "Emitente CRT 3 sem o grupo IBSCBS no item, em homologação, com emissão a partir de 01/07/2026.",
        "Informe o grupo IBSCBS. Esta POC só emite CRT 1.",
    ),
    lida(
        "W07-10",
        564,
        Mensagem,
        Moc("Anexo I, 4.2.1, W07-10"),
        "O total de produtos difere da soma dos itens com indTot=1.",
        "Recalcule o total pela soma dos itens.",
    ),
    lida(
        "W10-10",
        537,
        Mensagem,
        Moc("Anexo I, 4.2.1, W10-10"),
        "O total de descontos difere da soma dos itens.",
        "Recalcule o total pela soma dos itens.",
    ),
    lida(
        "W16-10",
        610,
        Mensagem,
        Moc("Anexo I, 4.2.1, W16-10, tolerância (*3) de R$ 0,50"),
        "vNF difere da soma dos valores que o compõem em mais de 50 centavos.",
        "Some vProd, menos vDesc e vICMSDeson, mais vST, vFCPST, vFrete, vSeg, vOutro, vII, vIPI, vIPIDevol e vServ.",
    ),
    lida(
        "YA02-40",
        899,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA02-40"),
        "A NFC-e não aceita tPag=90 (sem pagamento).",
        "Informe a forma de pagamento real.",
    ),
    lida(
        "YA03-10",
        865,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA03-10; NT 2025.001 deixa o modelo 55 para o futuro"),
        "A soma dos pagamentos é menor que o total da nota.",
        "Corrija os pagamentos.",
    ),
    lida(
        "YA03-20",
        866,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA03-20"),
        "Os pagamentos passam do total e não há vTroco.",
        "Informe vTroco.",
    ),
    lida(
        "YA03-30",
        904,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA03-30"),
        "Com tPag=90, o valor pago tem de ser zero.",
        "Informe vPag=0.00.",
    ),
    lida(
        "YA04-10",
        391,
        Mensagem,
        Nt("NT 2025.001 v1.02, YA04-10, que acrescenta Pix ao Anexo I; opcional por UF; futuro no modelo 55"),
        "Pagamento por cartão ou Pix sem o grupo card.",
        "Informe card com pelo menos tpIntegra.",
    ),
    lida(
        "YA04-20",
        963,
        Mensagem,
        Nt("NT 2023.004 v1.11, YA04-20; também em oobj.com.br e blog.tecnospeed.com.br/?p=24422"),
        "O grupo card só vale para tPag 03, 04, 10, 11, 12, 13, 15, 17 e 18.",
        "Tire o grupo card deste pagamento, ou corrija o tPag.",
    ),
    lida(
        "YA05-10",
        392,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA05-10; opcional por UF; também em oobj.com.br/bc/rejeicao-392-como-resolver"),
        "Com tpIntegra=1, faltam o CNPJ da credenciadora ou o cAut.",
        "Informe card/CNPJ e card/cAut, vindos do comprovante do TEF.",
    ),
    lida(
        "YA05-20",
        437,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA05-20"),
        "O CNPJ da instituição de pagamento é inválido.",
        "Confira card/CNPJ.",
    ),
    lida(
        "YA07a",
        796,
        Mensagem,
        Nt("NT 2023.004 v1.11, YA07a"),
        "O CNPJ do recebedor do pagamento é inválido.",
        "Confira card/CNPJReceb.",
    ),
    Regra {
        id: "SBX-RS-CAUT",
        c_stat: 0,
        efeito: Efeito::Alerta,
        alcance: Mensagem,
        explicacao: "No RS, segundo o pedido do produto, o cAut precisa ser igual ao do comprovante. O sandbox não tem o comprovante para comparar.",
        como_corrigir: "Copie o cAut exatamente como veio do TEF, sem cortar nem formatar.",
        fonte: Fonte::Secundaria(
            "atendimento.receita.rs.gov.br, citado no pedido; a página não foi localizada",
        ),
        confianca: Confianca::DeMemoria,
    },
    lida(
        "YA09-10",
        869,
        Mensagem,
        Moc("Anexo I, 4.2.1, YA09-10"),
        "vTroco é diferente de vPag menos vNF.",
        "Recalcule o troco.",
    ),
    lida(
        "ZX02-10",
        394,
        Mensagem,
        Moc("Anexo I, 4.2.1, ZX02-10"),
        "A NFC-e veio sem o QR Code.",
        "Informe infNFeSupl/qrCode.",
    ),
    lida(
        "ZX02-236",
        398,
        Mensagem,
        Nt("Anexo I, ZX02-236, atualizada pela NT 2025.001 para aceitar a versão 3"),
        "A versão do QR Code não é uma das previstas.",
        "Use a versão 2 ou 3 no segundo parâmetro.",
    ),
    lida(
        "ZX02-228",
        397,
        Mensagem,
        Moc("Anexo I, 4.2.1, ZX02-228"),
        "A chave no QR Code é diferente da chave da nota.",
        "Monte o QR Code depois da chave, sem editar nenhum dos dois.",
    ),
    lida(
        "ZX02-244",
        397,
        Mensagem,
        Moc("Anexo I, 4.2.1, ZX02-244"),
        "O ambiente no QR Code é diferente do tpAmb da nota.",
        "Use o mesmo tpAmb nos dois.",
    ),
    lida(
        "2B08-10",
        539,
        EstadoDaSefaz,
        Moc("Anexo I, 4.2.1, 2B08-10"),
        "Já existe nota com este número, série e emitente, mas com outra chave.",
        "Quase sempre o cNF mudou entre tentativas: consulte a chave do retorno antes de gerar outra nota.",
    ),
    lida(
        "2B08-20",
        204,
        EstadoDaSefaz,
        Moc("Anexo I, 4.2.1, 2B08-20"),
        "Esta nota já está na base da SEFAZ.",
        "Não reenvie: consulte a chave e guarde o protocolo que já existe.",
    ),
    // Service status and query.
    lida("STATUS-107", 107, Servico, Moc("Anexo I, 4.4.1"), "O serviço está em operação.", "Nada a fazer."),
    lida(
        "STATUS-108",
        108,
        Servico,
        Moc("Anexo I, 4.4.1"),
        "O serviço está parado por pouco tempo.",
        "Aguarde e tente de novo, ou use a contingência da UF.",
    ),
    lida(
        "STATUS-109",
        109,
        Servico,
        Moc("Anexo I, 4.4.1"),
        "O serviço está parado sem previsão.",
        "Use a contingência da UF.",
    ),
    lida(
        "J01",
        252,
        Mensagem,
        Moc("Visão Geral, regras da consulta"),
        "O tpAmb da consulta é diferente do ambiente que a recebeu.",
        "Consulte no mesmo ambiente da nota.",
    ),
    lida(
        "J02a",
        236,
        Mensagem,
        Moc("Visão Geral, J02a"),
        "A chave consultada tem dígito verificador inválido.",
        "Confira se a chave foi digitada ou colada inteira.",
    ),
    lida(
        "J03",
        217,
        EstadoDaSefaz,
        Moc("Visão Geral, J03"),
        "A SEFAZ não tem esta chave: a nota nunca foi autorizada, ou foi enviada a outro ambiente ou UF.",
        "Se o envio caiu sem retorno, esta resposta diz que ela não entrou; reenvie a mesma chave.",
    ),
    // Events.
    lida(
        "D04",
        491,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D04"),
        "O tpEvento informado é inválido.",
        "Use 110111 para cancelamento.",
    ),
    lida(
        "D05",
        492,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D05"),
        "O verEvento informado é inválido.",
        "Use verEvento=1.00.",
    ),
    lida(
        "D06",
        493,
        Mensagem,
        Moc("Anexo I, 4.1, grupo D, D06"),
        "O detEvento não atende ao schema do evento.",
        "Informe descEvento=Cancelamento, nProt e xJust de 15 a 255 caracteres.",
    ),
    lida(
        "P-DV",
        236,
        Mensagem,
        Moc("Visão Geral, regras gerais de evento"),
        "A chave do evento tem dígito verificador inválido.",
        "Confira a chave.",
    ),
    lida(
        "P12-44",
        574,
        Mensagem,
        Moc("Visão Geral, Tabela 5-38, P12-44"),
        "O CNPJ do autor do evento é diferente do CNPJ da chave.",
        "Só o emitente da nota pode cancelá-la.",
    ),
    lida(
        "P13-10",
        578,
        Relogio,
        Moc("Visão Geral, Tabela 5-38, P13-10"),
        "A data do evento está mais de 5 minutos à frente do processamento.",
        "Sincronize o relógio da máquina.",
    ),
    lida(
        "P15-10",
        594,
        Mensagem,
        Moc("Visão Geral, Tabela 5-38, P15-10"),
        "No cancelamento, o nSeqEvento tem de ser 1.",
        "Use nSeqEvento=1.",
    ),
    lida(
        "2P12-10",
        494,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P12-10"),
        "A chave não existe na SEFAZ: não se cancela uma nota que nunca foi autorizada.",
        "Veja a situação da chave. Nota rejeitada não precisa de cancelamento; basta corrigir e reenviar.",
    ),
    lida(
        "2P12-14",
        501,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P12-14; item 5.9.4"),
        "A nota foi autorizada há mais de 24 horas. A UF pode ter prazo próprio; a NF-e pode responder 155, a NFC-e é rejeitada.",
        "Consulte a legislação da UF. Fora do prazo, trate com a contabilidade.",
    ),
    lida(
        "2P12-22",
        580,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P12-22"),
        "O evento exige nota autorizada, e esta já está cancelada ou denegada.",
        "Nada a fazer: consulte a chave para ver o evento que já existe.",
    ),
    lida(
        "2P13-10",
        577,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P13-10"),
        "A data do evento é anterior à emissão da nota.",
        "Sincronize o relógio da máquina.",
    ),
    lida(
        "2P13-14",
        579,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P13-14"),
        "A data do evento é anterior à autorização, além da tolerância de 5 minutos.",
        "Sincronize o relógio da máquina.",
    ),
    lida(
        "2P23-10",
        222,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 2P23-10"),
        "O protocolo informado não é o da autorização desta nota.",
        "Use o nProt que veio no protNFe.",
    ),
    lida(
        "3P15-10",
        573,
        EstadoDaSefaz,
        Moc("Visão Geral, Tabela 5-38, 3P15-10"),
        "Este evento já foi registrado para esta chave.",
        "Consulte a chave; o MOC verifica antes se a nota já está cancelada (580).",
    ),
    // Recipient's manifestation.
    lida(
        "MANIF-595",
        595,
        Mensagem,
        Moc("Visão Geral, 5.11.3"),
        "A manifestação de operação não realizada (210240) exige justificativa.",
        "Informe xJust de 15 a 255 caracteres.",
    ),
    lida(
        "MANIF-494",
        494,
        EstadoDaSefaz,
        Moc("Visão Geral, regras gerais de evento"),
        "O Ambiente Nacional não tem esta chave.",
        "Confira a chave; ela pode ainda não ter chegado ao Ambiente Nacional.",
    ),
    lida(
        "MANIF-575",
        575,
        EstadoDaSefaz,
        Moc("Anexo I, 4.4.1; Visão Geral, 5.11.3"),
        "Só o destinatário da nota pode se manifestar sobre ela.",
        "Confira o CNPJ do autor: a base tem de ser a do destinatário.",
    ),
    // Distribution.
    lida(
        "DIST-138",
        138,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, item 3"),
        "Há documentos. Se o schema começar com resNFe, é só o resumo: o XML completo depende de manifestação.",
        "Guarde o ultNSU. Para um resumo, registre a ciência (210210) e busque de novo.",
    ),
    lida(
        "DIST-137",
        137,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, item 3.11.4.1"),
        "Não há mais documentos para este CNPJ.",
        "Aguarde 1 hora antes de consultar de novo; antes disso a resposta é 656.",
    ),
    lida(
        "DIST-656-ESPERA",
        656,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, item 3.11.4.1"),
        "Consulta repetida menos de 1 hora depois de um 137 ou de ultNSU igual a maxNSU. O CNPJ fica bloqueado por 1 hora.",
        "Respeite proxima_consulta_em. O limitador do emissor evita esta rejeição.",
    ),
    lida(
        "DIST-656-NSU",
        656,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, item 3.11.4.1"),
        "A consulta usou um NSU anterior ao último entregue, em vez de seguir pelo ultNSU retornado.",
        "Guarde o ultNSU de cada resposta e use-o na próxima; o 656 traz o último consultado.",
    ),
    lida(
        "DIST-640",
        640,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, H172"),
        "Este CNPJ não é interessado nesta nota.",
        "Consulte com o CNPJ do destinatário.",
    ),
    lida(
        "DIST-589",
        589,
        EstadoDaSefaz,
        Nt("NT 2014.002 v1.40, H061"),
        "O NSU informado é maior que o maior NSU disponível.",
        "Use o ultNSU retornado, sem somar nada.",
    ),
    lida(
        "DIST-489",
        489,
        Mensagem,
        Nt("NT 2014.002 v1.40, H02"),
        "O CNPJ do interessado é inválido.",
        "Corrija o CNPJ do interessado.",
    ),
    lida(
        "DIST-252",
        252,
        Mensagem,
        Nt("NT 2014.002 v1.40, H01"),
        "O tpAmb da consulta é diferente do ambiente que a recebeu.",
        "Consulte no mesmo ambiente.",
    ),
    lida(
        "DIST-236",
        236,
        Mensagem,
        Nt("NT 2014.002 v1.40, H072"),
        "A chave consultada tem dígito verificador inválido.",
        "Confira se a chave foi digitada ou colada inteira.",
    ),
];

/// A rule of the catalog by its id.
#[must_use]
pub fn regra(id: &str) -> Option<&'static Regra> {
    CATALOGO.iter().find(|regra| regra.id == id)
}

/// The rules that answer a code.
pub fn regras_do_c_stat(c_stat: u32) -> impl Iterator<Item = &'static Regra> {
    CATALOGO.iter().filter(move |regra| regra.c_stat == c_stat)
}

/// How many rules the catalog has, by source and by trust.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cobertura {
    /// All the rules.
    pub total: usize,
    /// Text read in the source.
    pub lidas: usize,
    /// Inferred from a source.
    pub inferidas: usize,
    /// From memory.
    pub de_memoria: usize,
    /// From the MOC.
    pub moc: usize,
    /// From a technical note.
    pub nt: usize,
    /// From a schema.
    pub xsd: usize,
    /// From a third party.
    pub secundaria: usize,
    /// Decisions of the sandbox.
    pub sandbox: usize,
}

/// Counts the catalog. `REGRAS.md` repeats these numbers and a test keeps them equal.
#[must_use]
pub fn cobertura() -> Cobertura {
    let mut contagem = Cobertura { total: CATALOGO.len(), ..Cobertura::default() };
    for regra in CATALOGO {
        match regra.confianca {
            Confianca::LidaNaFonte => contagem.lidas += 1,
            Confianca::Inferida => contagem.inferidas += 1,
            Confianca::DeMemoria => contagem.de_memoria += 1,
        }
        match regra.fonte {
            Fonte::Moc(_) => contagem.moc += 1,
            Fonte::Nt(_) => contagem.nt += 1,
            Fonte::Xsd(_) => contagem.xsd += 1,
            Fonte::Secundaria(_) => contagem.secundaria += 1,
            Fonte::Sandbox(_) => contagem.sandbox += 1,
            Fonte::Norma(_) | Fonte::Acbr(_) | Fonte::Memoria(_) => {}
        }
    }
    contagem
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_code_has_a_title() {
        for (posicao, regra) in CATALOGO.iter().enumerate() {
            assert!(CATALOGO[posicao + 1..].iter().all(|outra| outra.id != regra.id), "{} repete", regra.id);
            assert!(regra.c_stat == 0 || descricao_c_stat(regra.c_stat).is_some(), "{} sem título", regra.id);
            assert!(!regra.explicacao.is_empty() && !regra.como_corrigir.is_empty());
        }
    }

    #[test]
    #[ignore = "só imprime os números para copiar em REGRAS.md"]
    fn print_coverage() {
        println!("{:?}", cobertura());
    }

    #[test]
    fn regras_md_repeats_the_real_coverage() {
        let texto = include_str!("../../REGRAS.md");
        let contagem = cobertura();
        for linha in [
            format!("| Total | {} |", contagem.total),
            format!("| Lidas na fonte | {} |", contagem.lidas),
            format!("| Inferidas | {} |", contagem.inferidas),
            format!("| De memória | {} |", contagem.de_memoria),
            format!("| Do MOC | {} |", contagem.moc),
            format!("| De nota técnica | {} |", contagem.nt),
            format!("| Do XSD | {} |", contagem.xsd),
            format!("| Secundária | {} |", contagem.secundaria),
            format!("| Decisão do sandbox | {} |", contagem.sandbox),
        ] {
            assert!(texto.contains(&linha), "REGRAS.md desatualizado: esperava `{linha}`");
        }
    }
}
