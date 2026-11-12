//! The iterative loop of CliSiTef as a pure translation, with no library in sight.
//!
//! The real helper process calls `IniciaFuncaoSiTefInterativo` and then
//! `ContinuaFuncaoSiTefInterativo` while it returns 10000. Each round gives a command, a field type,
//! limits and a buffer. [`CicloSiTef`] turns one round into what the screen must do, and collects
//! the fields of the answer. The helper only has to move bytes between this and the library.
//!
//! Field and command numbers were checked against the behavior of the LGPL unit `ACBrTEFCliSiTefComum`
//! and the public Fiserv pages; see `docs/PAGAMENTOS.md`. **Never run against a real CliSiTef or
//! against SitDemo.** What field 951 holds in practice is not confirmed.

use crate::dominio::{
    Aprovacao, EntradaOperador, IdPagamento, OpcaoMenu, Passo, Pergunta, ReferenciaTransacao, TipoCampo, Vias,
};
use crate::fiscal::{DadosFiscaisPagamento, cnpj_valido};
use crate::provedor::ErroProvedor;

/// What `IniciaFuncaoSiTefInterativo` and `ContinuaFuncaoSiTefInterativo` return to go on.
pub const RETORNO_CONTINUA: i32 = 10000;
/// Field: the fiscal CNPJ of the acquirer.
pub const CAMPO_CNPJ_CREDENCIADORA: i32 = 950;
/// Field: the fiscal brand.
pub const CAMPO_BANDEIRA_FISCAL: i32 = 951;
/// Field: the fiscal authorization number.
pub const CAMPO_AUTORIZACAO_FISCAL: i32 = 952;
/// Field: the transaction number at SiTef.
pub const CAMPO_NSU_SITEF: i32 = 133;
/// Field: the transaction number at the host.
pub const CAMPO_NSU_HOST: i32 = 134;
/// Field: the first copy of the receipt.
pub const CAMPO_VIA_1: i32 = 121;
/// Field: the second copy of the receipt.
pub const CAMPO_VIA_2: i32 = 122;

/// One round of the loop, as the library gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RodadaSiTef {
    /// `ProximoComando`.
    pub comando: i32,
    /// `TipoCampo`.
    pub tipo_campo: i32,
    /// `TamanhoMinimo`.
    pub minimo: u16,
    /// `TamanhoMaximo`.
    pub maximo: u16,
    /// The text in the buffer.
    pub buffer: String,
}

/// What to do with a round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcaoSiTef {
    /// Call the library again at once, with an empty buffer.
    Prosseguir,
    /// Show this on the screen and bring the answer to [`CicloSiTef::responder`].
    Passo(Passo),
}

/// How an answer goes back to the library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetornoSiTef {
    /// `Continua`: 0 goes on, 1 goes back, 2 interrupts, -1 ends the collection.
    pub continua: i32,
    /// The buffer.
    pub buffer: String,
}

/// Translates rounds and collects the answer of one transaction.
#[derive(Clone, Debug)]
pub struct CicloSiTef {
    id: IdPagamento,
    valor_centavos: u64,
    cnpj: Option<String>,
    bandeira: Option<String>,
    autorizacao: Option<String>,
    nsu_sitef: Option<String>,
    nsu_host: Option<String>,
    via1: Vec<String>,
    via2: Vec<String>,
}

impl CicloSiTef {
    /// A new transaction.
    #[must_use]
    pub fn new(id: IdPagamento, valor_centavos: u64) -> Self {
        Self {
            id,
            valor_centavos,
            cnpj: None,
            bandeira: None,
            autorizacao: None,
            nsu_sitef: None,
            nsu_host: None,
            via1: Vec::new(),
            via2: Vec::new(),
        }
    }

    /// Reads a round. Command 0 only delivers a field, and the messages need no answer.
    pub fn rodada(&mut self, rodada: &RodadaSiTef) -> AcaoSiTef {
        let texto = rodada.buffer.trim().to_owned();
        match rodada.comando {
            0 => {
                self.guardar(rodada.tipo_campo, &rodada.buffer);
                AcaoSiTef::Prosseguir
            }
            1..=3 | 5 | 13..=15 => {
                AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Mensagem { texto, exige_ok: false }))
            }
            4 | 11 | 12 | 16 => AcaoSiTef::Prosseguir,
            20 => AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Confirmacao { texto })),
            21 => AcaoSiTef::Passo(Passo::Perguntar(ler_menu(&rodada.buffer))),
            22 => AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Mensagem { texto, exige_ok: true })),
            23 => AcaoSiTef::Passo(Passo::Aguardar { mensagem: None, tentar_em_ms: 200 }),
            30 | 31 | 34 | 35 | 41 => AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Campo {
                rotulo: texto,
                tipo: match rodada.comando {
                    34 => TipoCampo::Valor,
                    35 => TipoCampo::CodigoBarras,
                    41 => TipoCampo::Mascarado,
                    _ => TipoCampo::Texto,
                },
                minimo: rodada.minimo,
                maximo: rodada.maximo,
            })),
            50 => AcaoSiTef::Passo(Passo::Perguntar(Pergunta::QrCode { conteudo: texto, mensagem: None })),
            51 | 52 => AcaoSiTef::Prosseguir,
            // A command with no known meaning goes on rather than freezing the checkout on a guess.
            _ => AcaoSiTef::Prosseguir,
        }
    }

    /// Turns what the operator did into the next call of the loop.
    #[must_use]
    pub fn responder(entrada: &EntradaOperador) -> RetornoSiTef {
        let (continua, buffer) = match entrada {
            EntradaOperador::Nada | EntradaOperador::Ok => (0, String::new()),
            EntradaOperador::Sim => (0, "0".to_owned()),
            EntradaOperador::Nao => (0, "1".to_owned()),
            EntradaOperador::Opcao(indice) => (0, (indice + 1).to_string()),
            EntradaOperador::Texto(texto) => (0, texto.clone()),
            EntradaOperador::Voltar => (1, String::new()),
            EntradaOperador::Cancelar => (-1, String::new()),
        };
        RetornoSiTef { continua, buffer }
    }

    /// The end of the loop, by the code the library returned.
    pub fn fim(&self, retorno: i32) -> Result<Passo, ErroProvedor> {
        match retorno {
            0 => Ok(Passo::Aprovado(Box::new(self.aprovacao()))),
            -2 => Ok(Passo::Cancelado { motivo: "Cancelado pelo operador".to_owned() }),
            // Positive codes come from the network or the issuer, negative ones from the library.
            n if n > 0 => {
                Ok(Passo::Negado { codigo: n.to_string(), motivo: format!("Não autorizada ({n})") })
            }
            n => Err(ErroProvedor::Indisponivel(format!("CliSiTef retornou {n}"))),
        }
    }

    fn guardar(&mut self, campo: i32, valor: &str) {
        match campo {
            CAMPO_CNPJ_CREDENCIADORA => self.cnpj = Some(valor.trim().to_owned()),
            CAMPO_BANDEIRA_FISCAL => self.bandeira = Some(valor.trim().to_owned()),
            CAMPO_AUTORIZACAO_FISCAL => self.autorizacao = Some(valor.trim().to_owned()),
            CAMPO_NSU_SITEF => self.nsu_sitef = Some(valor.trim().to_owned()),
            CAMPO_NSU_HOST => self.nsu_host = Some(valor.trim().to_owned()),
            CAMPO_VIA_1 => self.via1 = linhas(valor),
            CAMPO_VIA_2 => self.via2 = linhas(valor),
            _ => {}
        }
    }

    fn aprovacao(&self) -> Aprovacao {
        let mut fiscais = DadosFiscaisPagamento::integrado();
        fiscais.com_c_aut(self.autorizacao.as_deref(), "o CliSiTef não entregou o campo 952");
        match self.cnpj.as_deref() {
            Some(cnpj) if cnpj_valido(cnpj) => fiscais.cnpj = Some(cnpj.to_owned()),
            Some(cnpj) => fiscais.com_cnpj_do_provedor(cnpj),
            None => fiscais.com_cnpj_da_rede(None),
        }
        // Field 951 may be a code or a name; which one it is in practice was not confirmed.
        match self.bandeira.as_deref() {
            Some(codigo) if codigo.len() == 2 && codigo.bytes().all(|b| b.is_ascii_digit()) => {
                fiscais.t_band = Some(codigo.to_owned());
            }
            outra => fiscais.com_bandeira(outra),
        }
        Aprovacao {
            referencia: ReferenciaTransacao {
                id: Some(self.id.clone()),
                rede: None,
                nsu: self.nsu_host.clone().or_else(|| self.nsu_sitef.clone()),
                controle: self.nsu_sitef.clone(),
                valor_centavos: self.valor_centavos,
                extras: std::collections::BTreeMap::new(),
            },
            dados_fiscais: fiscais,
            // The source only says first and second copy; which one is the customer's is not confirmed.
            vias: Vias {
                cliente: self.via1.clone(),
                estabelecimento: self.via2.clone(),
                unica: Vec::new(),
                impressas_no_terminal: false,
            },
            mensagem_operador: None,
            tipo_efetivo: None,
            avisos: Vec::new(),
        }
    }
}

/// The options come as `1:Debito;2:Credito;` after an optional title line.
fn ler_menu(buffer: &str) -> Pergunta {
    let opcoes = buffer
        .split(';')
        .filter_map(|parte| {
            let (valor, texto) = parte.split_once(':')?;
            Some(OpcaoMenu { valor: valor.trim().to_owned(), texto: texto.trim().to_owned() })
        })
        .collect();
    Pergunta::Menu { titulo: String::new(), opcoes }
}

fn linhas(texto: &str) -> Vec<String> {
    texto.lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fiscal::AlertaFiscal;

    fn rodada(comando: i32, tipo_campo: i32, buffer: &str) -> RodadaSiTef {
        RodadaSiTef { comando, tipo_campo, minimo: 0, maximo: 0, buffer: buffer.to_owned() }
    }

    fn novo() -> CicloSiTef {
        CicloSiTef::new(IdPagamento::new("s1").expect("id"), 1990)
    }

    #[test]
    fn cada_familia_de_comando_vira_o_passo_certo() {
        let mut ciclo = novo();
        assert_eq!(ciclo.rodada(&rodada(0, 133, "123")), AcaoSiTef::Prosseguir);
        assert!(matches!(
            ciclo.rodada(&rodada(1, 0, "INSIRA O CARTAO")),
            AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Mensagem { exige_ok: false, .. }))
        ));
        assert!(matches!(
            ciclo.rodada(&rodada(20, 0, "CONFIRMA?")),
            AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Confirmacao { .. }))
        ));
        assert!(matches!(
            ciclo.rodada(&rodada(22, 0, "TECLE ENTER")),
            AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Mensagem { exige_ok: true, .. }))
        ));
        assert!(matches!(ciclo.rodada(&rodada(23, 0, "")), AcaoSiTef::Passo(Passo::Aguardar { .. })));
        assert!(matches!(
            ciclo.rodada(&rodada(34, 0, "VALOR")),
            AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Campo { tipo: TipoCampo::Valor, .. }))
        ));
        assert!(matches!(
            ciclo.rodada(&rodada(50, 0, "QR")),
            AcaoSiTef::Passo(Passo::Perguntar(Pergunta::QrCode { .. }))
        ));
        assert_eq!(ciclo.rodada(&rodada(9999, 0, "")), AcaoSiTef::Prosseguir);
    }

    #[test]
    fn menu_le_as_opcoes() {
        let mut ciclo = novo();
        let AcaoSiTef::Passo(Passo::Perguntar(Pergunta::Menu { opcoes, .. })) =
            ciclo.rodada(&rodada(21, 0, "1:Debito;2:Credito;"))
        else {
            panic!("devia ser menu");
        };
        assert_eq!(opcoes[1], OpcaoMenu { valor: "2".into(), texto: "Credito".into() });
    }

    #[test]
    fn respostas_do_operador_viram_chamadas_da_biblioteca() {
        assert_eq!(CicloSiTef::responder(&EntradaOperador::Cancelar).continua, -1);
        assert_eq!(CicloSiTef::responder(&EntradaOperador::Voltar).continua, 1);
        assert_eq!(CicloSiTef::responder(&EntradaOperador::Opcao(1)).buffer, "2");
        assert_eq!(CicloSiTef::responder(&EntradaOperador::Texto("x".into())).buffer, "x");
    }

    #[test]
    fn campos_viram_aprovacao_com_dados_fiscais() {
        let mut ciclo = novo();
        for (campo, valor) in [
            (950, "16501555000157"),
            (951, "02"),
            (952, "AUT9"),
            (133, "000123"),
            (134, "HOST9"),
            (121, "VIA1\nL2"),
            (122, "VIA2"),
        ] {
            ciclo.rodada(&rodada(0, campo, valor));
        }
        let Ok(Passo::Aprovado(aprovacao)) = ciclo.fim(0) else { panic!("devia aprovar") };
        assert_eq!(aprovacao.dados_fiscais.c_aut.as_deref(), Some("AUT9"));
        assert_eq!(aprovacao.dados_fiscais.t_band.as_deref(), Some("02"));
        assert_eq!(aprovacao.dados_fiscais.cnpj.as_deref(), Some("16501555000157"));
        assert!(aprovacao.dados_fiscais.alertas.is_empty());
        assert_eq!(aprovacao.referencia.nsu.as_deref(), Some("HOST9"));
        assert_eq!(aprovacao.vias.cliente, ["VIA1", "L2"]);
    }

    #[test]
    fn sem_os_campos_fiscais_nada_e_inventado() {
        let Ok(Passo::Aprovado(aprovacao)) = novo().fim(0) else { panic!("devia aprovar") };
        let dados = aprovacao.dados_fiscais;
        assert_eq!((dados.c_aut, dados.cnpj, dados.t_band), (None, None, None));
        assert!(dados.alertas.iter().any(|a| matches!(a, AlertaFiscal::CAutAusente { .. })));
    }

    #[test]
    fn fim_distingue_recusa_cancelamento_e_falha() {
        let ciclo = novo();
        assert!(matches!(ciclo.fim(-2), Ok(Passo::Cancelado { .. })));
        assert!(matches!(ciclo.fim(5), Ok(Passo::Negado { .. })));
        assert!(ciclo.fim(-5).is_err_and(|erro| erro.incerto()));
    }
}
