//! Card and Pix at the checkout, through `ravenrs-pagamento`: whatever provider is behind it, the
//! checkout asks the same questions, and an approval only becomes final once the note is authorized.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app_nfe::engine::{Cartao, TipoIntegracao};
use ravenrs_pagamento::simulador::SimuladorProvedor;
use ravenrs_pagamento::{
    DadosFiscaisPagamento, Diario, DiarioMemoria, EntradaOperador, Estado, IdPagamento, Impressao,
    Pagamentos, Passo, PedidoPagamento, ProvedorPagamento, Recuperacao, TipoPagamento, TpIntegra,
};
use ravenrs_pos::Forma;

/// What the checkout needs from a payment provider, whichever it is.
pub trait Tef {
    /// Starts charging.
    fn iniciar(&self, pedido: PedidoPagamento) -> Result<Passo, String>;
    /// Goes on with what the operator answered.
    fn continuar(&self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, String>;
    /// Says whether the sale finished: confirms the approval, or undoes it.
    fn concluir(&self, id: &IdPagamento, impressao: Impressao) -> Result<Estado, String>;
    /// Settles what an earlier session left open.
    fn recuperar(&self) -> Result<Vec<Recuperacao>, String>;
}

impl<P: ProvedorPagamento, D: Diario> Tef for RefCell<Pagamentos<P, D>> {
    fn iniciar(&self, pedido: PedidoPagamento) -> Result<Passo, String> {
        self.borrow_mut().iniciar(pedido).map_err(|error| error.to_string())
    }

    fn continuar(&self, id: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, String> {
        self.borrow_mut().continuar(id, entrada).map_err(|error| error.to_string())
    }

    fn concluir(&self, id: &IdPagamento, impressao: Impressao) -> Result<Estado, String> {
        self.borrow_mut().concluir(id, impressao).map_err(|error| error.to_string())
    }

    fn recuperar(&self) -> Result<Vec<Recuperacao>, String> {
        self.borrow_mut().recuperar().map_err(|error| error.to_string())
    }
}

/// The provider and its simulator, for a checkout that has no contract yet: nothing is charged.
pub fn tef_simulado() -> (Rc<dyn Tef>, SimuladorProvedor) {
    let simulador = SimuladorProvedor::new();
    let pagamentos = match Pagamentos::new(simulador.clone(), DiarioMemoria::new()) {
        Ok(pagamentos) => pagamentos,
        // A journal in memory cannot fail to open; should it, the checkout still sells in cash.
        Err(_) => return (Rc::new(SemTef), simulador),
    };
    (Rc::new(RefCell::new(pagamentos)), simulador)
}

/// A checkout with no provider: cards and Pix say so.
struct SemTef;

impl Tef for SemTef {
    fn iniciar(&self, _pedido: PedidoPagamento) -> Result<Passo, String> {
        Err("Não há provedor de cartão configurado.".to_owned())
    }
    fn continuar(&self, _id: &IdPagamento, _entrada: EntradaOperador) -> Result<Passo, String> {
        Err("Não há provedor de cartão configurado.".to_owned())
    }
    fn concluir(&self, _id: &IdPagamento, _impressao: Impressao) -> Result<Estado, String> {
        Err("Não há provedor de cartão configurado.".to_owned())
    }
    fn recuperar(&self) -> Result<Vec<Recuperacao>, String> {
        Ok(Vec::new())
    }
}

/// How a way of paying goes to the provider, or `None` for cash, which never does.
pub fn tipo_do_provedor(forma: Forma) -> Option<TipoPagamento> {
    match forma {
        Forma::Dinheiro => None,
        Forma::Debito => Some(TipoPagamento::Debito),
        Forma::Credito => Some(TipoPagamento::Credito { parcelas: 1 }),
        Forma::Pix => Some(TipoPagamento::Pix),
    }
}

/// The card group of the note, from what the provider approved.
pub fn cartao_da_aprovacao(dados: &DadosFiscaisPagamento) -> Cartao {
    let mut cartao = Cartao::nao_integrado();
    if dados.tp_integra == TpIntegra::Integrado {
        cartao.integracao = TipoIntegracao::Integrado;
    }
    cartao.cnpj = dados.cnpj.clone();
    cartao.bandeira = dados.t_band.clone();
    cartao.autorizacao = dados.c_aut.clone();
    cartao.cnpj_recebedor = dados.cnpj_receb.clone();
    cartao.terminal = dados.id_term_pag.clone();
    cartao
}

/// A new id for a payment, with nothing personal in it.
pub fn novo_id(numero_venda: u64) -> Result<IdPagamento, String> {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    IdPagamento::new(format!("pdv-{numero_venda}-{agora}")).map_err(|error| format!("{error:?}"))
}
