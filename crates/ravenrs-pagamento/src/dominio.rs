//! The payment state machine and the data that travels through it. No input or output here.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::fiscal::DadosFiscaisPagamento;

/// The longest payment id. It is the limit of `external_reference` in Mercado Pago.
pub const TAMANHO_MAXIMO_ID: usize = 64;

/// The identity of one payment, chosen by the application and unique for the life of the journal.
///
/// Only letters, digits, `-` and `_`, up to [`TAMANHO_MAXIMO_ID`] characters. Do not put personal
/// data in it: it goes to the provider and to the journal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdPagamento(String);

impl IdPagamento {
    /// Checks and takes an id.
    pub fn new(id: impl Into<String>) -> Result<Self, ErroDominio> {
        let id = id.into();
        let valido = !id.is_empty()
            && id.len() <= TAMANHO_MAXIMO_ID
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if valido { Ok(Self(id)) } else { Err(ErroDominio::IdInvalido(id)) }
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IdPagamento {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How the customer wants to pay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoPagamento {
    /// The operator or the customer chooses on the provider's side.
    Escolher,
    /// Debit card.
    Debito,
    /// Credit card, in this many installments.
    Credito {
        /// The number of installments, from 1.
        parcelas: u8,
    },
    /// Meal, food or similar voucher.
    Voucher,
    /// Pix.
    Pix,
}

/// What the application asks to charge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PedidoPagamento {
    /// The identity of this payment.
    pub id: IdPagamento,
    /// The amount in cents. It is never zero.
    pub valor_centavos: u64,
    /// How to pay.
    pub tipo: TipoPagamento,
    /// The number of the fiscal document tied to this payment, if there is one already.
    pub documento: Option<String>,
}

impl PedidoPagamento {
    /// A new request with no fiscal document yet. A zero amount is refused.
    pub fn new(id: IdPagamento, valor_centavos: u64, tipo: TipoPagamento) -> Result<Self, ErroDominio> {
        if valor_centavos == 0 {
            return Err(ErroDominio::ValorZero);
        }
        Ok(Self { id, valor_centavos, tipo, documento: None })
    }
}

/// The life of a payment.
///
/// ```text
/// Criado -> Aguardando -> Aprovado -> Confirmado -> Estornado
///    |          |  |         |  |
///    v          v  v         v  v
/// Cancelado  Negado Cancelado Desfeito  Estornado (only with no third leg)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Written in the journal; nothing was sent yet.
    Criado,
    /// Sent to the provider; the outcome is not known.
    Aguardando,
    /// Approved by the provider; the sale has not finished on our side.
    Aprovado,
    /// The sale finished and the provider was told so.
    Confirmado,
    /// Reversed right after approval, because the sale did not finish.
    Desfeito,
    /// Refused by the provider.
    Negado,
    /// Abandoned before approval.
    Cancelado,
    /// Paid back after the sale.
    Estornado,
}

impl Estado {
    /// True while something must still happen before this payment can be forgotten.
    #[must_use]
    pub const fn pendente(self) -> bool {
        matches!(self, Self::Criado | Self::Aguardando | Self::Aprovado)
    }
}

/// What moves a payment from one state to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evento {
    /// The request is about to go to the provider.
    Enviar,
    /// The provider approved.
    Aprovar,
    /// The provider refused.
    Negar,
    /// Abandoned before approval.
    Cancelar,
    /// The sale finished.
    Confirmar,
    /// The approval was reversed.
    Desfazer,
    /// The money went back.
    Estornar,
}

/// The only legal moves. Anything else is a defect in the caller or in a provider.
pub fn transitar(estado: Estado, evento: Evento) -> Result<Estado, ErroDominio> {
    use Estado as E;
    use Evento as V;
    let novo = match (estado, evento) {
        (E::Criado, V::Enviar) => E::Aguardando,
        (E::Criado | E::Aguardando, V::Cancelar) => E::Cancelado,
        (E::Aguardando, V::Aprovar) => E::Aprovado,
        (E::Aguardando, V::Negar) => E::Negado,
        (E::Aprovado, V::Confirmar) => E::Confirmado,
        (E::Aprovado, V::Desfazer) => E::Desfeito,
        (E::Aprovado | E::Confirmado, V::Estornar) => E::Estornado,
        _ => return Err(ErroDominio::TransicaoInvalida { estado, evento }),
    };
    Ok(novo)
}

/// What each provider can really do. The application reads this instead of asking who it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capacidades {
    /// The provider waits for a confirmation after approval, and undoes without one.
    pub terceira_perna: bool,
    /// A refund can be asked for through the integration.
    pub estorna_por_api: bool,
    /// A partial refund can be asked for through the integration.
    pub estorno_parcial: bool,
    /// A payment in progress can be cancelled from the point of sale.
    pub cancela_em_andamento: bool,
    /// The operator answers questions on our screen during the payment.
    pub pergunta_ao_operador: bool,
    /// The provider's own terminal prints the receipt.
    pub imprime_no_terminal: bool,
    /// It needs a server of ours to receive notifications or to hold the secret.
    pub exige_servidor: bool,
    /// It can be exercised with no contract and no hardware.
    pub testavel_sem_hardware: bool,
}

impl Capacidades {
    /// PayGo Windows and the standard manager, through files.
    pub const ARQUIVOS: Self = Self {
        terceira_perna: true,
        estorna_por_api: true,
        estorno_parcial: false,
        cancela_em_andamento: false,
        pergunta_ao_operador: false,
        imprime_no_terminal: false,
        exige_servidor: false,
        testavel_sem_hardware: false,
    };
    /// CliSiTef, through the helper process.
    pub const SITEF: Self = Self {
        terceira_perna: true,
        estorna_por_api: true,
        estorno_parcial: false,
        cancela_em_andamento: true,
        pergunta_ao_operador: true,
        imprime_no_terminal: false,
        exige_servidor: false,
        testavel_sem_hardware: false,
    };
    /// Mercado Pago Point through the Orders API.
    pub const MERCADO_PAGO_POINT: Self = Self {
        terceira_perna: false,
        estorna_por_api: true,
        estorno_parcial: false,
        cancela_em_andamento: true,
        pergunta_ao_operador: false,
        imprime_no_terminal: true,
        exige_servidor: true,
        testavel_sem_hardware: true,
    };
    /// Stone Connect 2.0: a machine, not a pinpad. There is no refund route in its API.
    pub const STONE_CONNECT: Self = Self {
        terceira_perna: false,
        estorna_por_api: false,
        estorno_parcial: false,
        cancela_em_andamento: false,
        pergunta_ao_operador: false,
        imprime_no_terminal: true,
        exige_servidor: true,
        testavel_sem_hardware: false,
    };
    /// Stone AutoTEF over local HTTP, as read in third-party code. Not confirmed with Stone.
    pub const STONE_AUTOTEF: Self = Self {
        terceira_perna: false,
        estorna_por_api: true,
        estorno_parcial: false,
        cancela_em_andamento: false,
        pergunta_ao_operador: false,
        imprime_no_terminal: false,
        exige_servidor: false,
        testavel_sem_hardware: false,
    };
    /// Stone TEF through its native library, in the helper process.
    pub const STONE_TEF: Self = Self { ..Self::SITEF };
}

/// Who processes the payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvedorId {
    /// The simulator of this crate. It has no fiscal value.
    Simulador,
    /// The standard manager through IntPos files.
    GerenciadorPadrao,
    /// PayGo Windows through IntPos files.
    PayGoArquivos,
    /// Mercado Pago Point.
    MercadoPagoPoint,
    /// SiTef through the helper process.
    SiTef,
    /// Stone TEF through the helper process.
    StoneTef,
    /// Stone AutoTEF over local HTTP.
    StoneAutoTef,
}

/// How the provider's answer identifies an approved transaction. It goes to the journal, so it
/// never holds a card number, a password or what the operator typed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenciaTransacao {
    /// The payment this belongs to, when the provider knows it.
    pub id: Option<IdPagamento>,
    /// The acquirer network.
    pub rede: Option<String>,
    /// The transaction number at the acquirer.
    pub nsu: Option<String>,
    /// The provider's own control code.
    pub controle: Option<String>,
    /// The amount in cents.
    pub valor_centavos: u64,
    /// What else this provider needs to find the transaction again.
    pub extras: BTreeMap<String, String>,
}

/// The receipts of a transaction, as text lines.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vias {
    /// The customer's copy, when it differs.
    pub cliente: Vec<String>,
    /// The merchant's copy, when it differs.
    pub estabelecimento: Vec<String>,
    /// The single copy, when the provider has only one.
    pub unica: Vec<String>,
    /// The provider's terminal prints the receipt, so none of this may exist.
    pub impressas_no_terminal: bool,
}

impl Vias {
    /// What to print for the customer: their copy, or else the single one.
    #[must_use]
    pub fn via_cliente(&self) -> &[String] {
        if self.cliente.is_empty() { &self.unica } else { &self.cliente }
    }

    /// What to keep for the merchant: their copy, or else the single one.
    #[must_use]
    pub fn via_estabelecimento(&self) -> &[String] {
        if self.estabelecimento.is_empty() { &self.unica } else { &self.estabelecimento }
    }

    /// True when there is nothing for the application to print.
    #[must_use]
    pub fn vazias(&self) -> bool {
        self.cliente.is_empty() && self.estabelecimento.is_empty() && self.unica.is_empty()
    }
}

/// Something the application must see on an approval that is not a fiscal matter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvisoAprovacao {
    /// The provider's terminal says it could not print.
    SemPapel,
    /// Any other notice, as the provider wrote it.
    Outro(String),
}

/// An approval, with all that the application needs from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Aprovacao {
    /// How to find this transaction again.
    pub referencia: ReferenciaTransacao,
    /// What the fiscal document needs.
    pub dados_fiscais: DadosFiscaisPagamento,
    /// The receipts.
    pub vias: Vias,
    /// The text for the operator, if the provider sent one.
    pub mensagem_operador: Option<String>,
    /// How it was really paid, when the provider says.
    pub tipo_efetivo: Option<TipoPagamento>,
    /// What else the application must see.
    pub avisos: Vec<AvisoAprovacao>,
}

/// What kind of value a field asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoCampo {
    /// Free text.
    Texto,
    /// Digits only.
    Numero,
    /// An amount of money.
    Valor,
    /// A value read by a barcode scanner.
    CodigoBarras,
    /// Text that the screen must hide while it is typed.
    Mascarado,
}

/// One option of a menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcaoMenu {
    /// The value that goes back to the provider.
    pub valor: String,
    /// The text the operator reads.
    pub texto: String,
}

/// What the provider needs from the operator's screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pergunta {
    /// A message; with `exige_ok` the flow waits for the operator.
    Mensagem {
        /// The text.
        texto: String,
        /// Whether the operator must acknowledge it.
        exige_ok: bool,
    },
    /// A yes or no.
    Confirmacao {
        /// The question.
        texto: String,
    },
    /// A choice.
    Menu {
        /// The title.
        titulo: String,
        /// The options, in the order given.
        opcoes: Vec<OpcaoMenu>,
    },
    /// A value to type.
    Campo {
        /// The label.
        rotulo: String,
        /// The kind of value.
        tipo: TipoCampo,
        /// The least length accepted.
        minimo: u16,
        /// The greatest length accepted.
        maximo: u16,
    },
    /// A QR code to show while the provider waits. Answer with [`EntradaOperador::Nada`].
    QrCode {
        /// The content of the QR code.
        conteudo: String,
        /// The text beside it.
        mensagem: Option<String>,
    },
}

/// What came from the operator. Its `Debug` hides typed text, since it may be a tax id.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntradaOperador {
    /// Nothing to say; just ask again.
    Nada,
    /// Acknowledged.
    Ok,
    /// Yes.
    Sim,
    /// No.
    Nao,
    /// The chosen option of a menu, from zero.
    Opcao(usize),
    /// The typed value.
    Texto(String),
    /// Back to the previous question.
    Voltar,
    /// The operator gave up.
    Cancelar,
}

impl fmt::Debug for EntradaOperador {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nada => f.write_str("Nada"),
            Self::Ok => f.write_str("Ok"),
            Self::Sim => f.write_str("Sim"),
            Self::Nao => f.write_str("Nao"),
            Self::Opcao(indice) => write!(f, "Opcao({indice})"),
            Self::Texto(_) => f.write_str("Texto(<oculto>)"),
            Self::Voltar => f.write_str("Voltar"),
            Self::Cancelar => f.write_str("Cancelar"),
        }
    }
}

/// What the flow asks for next.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Passo {
    /// Show this and send the answer to `continuar`.
    Perguntar(Pergunta),
    /// Nothing for now. Call `continuar` with [`EntradaOperador::Nada`] after this long.
    Aguardar {
        /// The text for the screen, if there is one.
        mensagem: Option<String>,
        /// How long to wait, in milliseconds.
        tentar_em_ms: u32,
    },
    /// Approved. Issue the fiscal document, print and call `concluir`.
    Aprovado(Box<Aprovacao>),
    /// Refused.
    Negado {
        /// The provider's code.
        codigo: String,
        /// The reason, for the operator.
        motivo: String,
    },
    /// Abandoned before approval.
    Cancelado {
        /// The reason, for the operator.
        motivo: String,
    },
}

/// Why something in the domain was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErroDominio {
    /// The id has a forbidden character or the wrong length.
    IdInvalido(String),
    /// There is no such thing as charging nothing.
    ValorZero,
    /// This move does not exist.
    TransicaoInvalida {
        /// The state it was in.
        estado: Estado,
        /// What was attempted.
        evento: Evento,
    },
}

impl fmt::Display for ErroDominio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdInvalido(id) => write!(f, "id de pagamento inválido: {id:?}"),
            Self::ValorZero => f.write_str("valor zero não é um pagamento"),
            Self::TransicaoInvalida { estado, evento } => {
                write!(f, "transição inválida: {evento:?} em {estado:?}")
            }
        }
    }
}

impl std::error::Error for ErroDominio {}

#[cfg(test)]
mod tests {
    use super::*;

    const ESTADOS: [Estado; 8] = [
        Estado::Criado,
        Estado::Aguardando,
        Estado::Aprovado,
        Estado::Confirmado,
        Estado::Desfeito,
        Estado::Negado,
        Estado::Cancelado,
        Estado::Estornado,
    ];
    const EVENTOS: [Evento; 7] = [
        Evento::Enviar,
        Evento::Aprovar,
        Evento::Negar,
        Evento::Cancelar,
        Evento::Confirmar,
        Evento::Desfazer,
        Evento::Estornar,
    ];

    #[test]
    fn a_tabela_inteira_de_transicoes_e_a_esperada() {
        let mut legais = Vec::new();
        for estado in ESTADOS {
            for evento in EVENTOS {
                if let Ok(novo) = transitar(estado, evento) {
                    legais.push((estado, evento, novo));
                }
            }
        }
        assert_eq!(
            legais,
            vec![
                (Estado::Criado, Evento::Enviar, Estado::Aguardando),
                (Estado::Criado, Evento::Cancelar, Estado::Cancelado),
                (Estado::Aguardando, Evento::Aprovar, Estado::Aprovado),
                (Estado::Aguardando, Evento::Negar, Estado::Negado),
                (Estado::Aguardando, Evento::Cancelar, Estado::Cancelado),
                (Estado::Aprovado, Evento::Confirmar, Estado::Confirmado),
                (Estado::Aprovado, Evento::Desfazer, Estado::Desfeito),
                (Estado::Aprovado, Evento::Estornar, Estado::Estornado),
                (Estado::Confirmado, Evento::Estornar, Estado::Estornado),
            ]
        );
    }

    #[test]
    fn estados_finais_nao_saem_do_lugar() {
        for estado in [Estado::Desfeito, Estado::Negado, Estado::Cancelado, Estado::Estornado] {
            for evento in EVENTOS {
                assert!(transitar(estado, evento).is_err(), "{estado:?} {evento:?}");
            }
            assert!(!estado.pendente());
        }
    }

    #[test]
    fn id_recusa_o_que_nao_passa_no_provedor() {
        assert!(IdPagamento::new("venda-1_a").is_ok());
        assert!(IdPagamento::new("").is_err());
        assert!(IdPagamento::new("com espaco").is_err());
        assert!(IdPagamento::new("x".repeat(65)).is_err());
    }

    #[test]
    fn debug_da_entrada_nao_mostra_o_que_foi_digitado() {
        let texto = format!("{:?}", EntradaOperador::Texto("11122233344".into()));
        assert!(!texto.contains("111"));
    }

    #[test]
    fn valor_zero_nao_vira_pedido() {
        let id = IdPagamento::new("a").expect("id");
        assert_eq!(PedidoPagamento::new(id, 0, TipoPagamento::Pix), Err(ErroDominio::ValorZero));
    }
}
