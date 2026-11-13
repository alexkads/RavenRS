//! The contract of the helper, in the same process and over a real child process.
#![allow(clippy::expect_used)]

use ravenrs_pagamento::auxiliar::{CanalLinhas, CanalLocal, ProvedorAuxiliar};
use ravenrs_pagamento::simulador::{SimuladorProvedor, SituacaoRazao, cenarios, iniciar};
use ravenrs_pagamento::sitef::{AcaoSiTef, CicloSiTef, RETORNO_CONTINUA, RodadaSiTef};
use ravenrs_pagamento::{
    Cancelamento, Capacidades, Desfecho, DiarioMemoria, EntradaOperador, ErroProvedor, Estado, IdPagamento,
    Impressao, Pagamentos, Passo, PedidoPagamento, Pergunta, ProvedorId, ProvedorPagamento,
    ReferenciaTransacao, SituacaoProvedor, TipoPagamento,
};

fn id() -> IdPagamento {
    IdPagamento::new("venda-1").expect("id")
}

fn pedido() -> PedidoPagamento {
    PedidoPagamento::new(id(), 1990, TipoPagamento::Debito).expect("pedido")
}

#[test]
fn venda_inteira_e_recuperacao_atravessam_o_contrato() {
    let simulador = SimuladorProvedor::new();
    let diario = DiarioMemoria::new();
    {
        let provedor = ProvedorAuxiliar::conectar(CanalLocal::new(simulador.clone())).expect("conecta");
        let mut pagamentos = Pagamentos::new(provedor, diario.clone()).expect("novo");
        pagamentos.iniciar(pedido()).expect("inicia");
    }
    let provedor = ProvedorAuxiliar::conectar(CanalLocal::new(simulador.clone())).expect("reconecta");
    let mut pagamentos = Pagamentos::new(provedor, diario).expect("religa");
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Desfeito);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Desfeita));
}

#[test]
fn perguntas_cruzam_o_canal() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::pergunta_parcelas));
    let provedor = ProvedorAuxiliar::conectar(CanalLocal::new(simulador)).expect("conecta");
    let mut pagamentos = Pagamentos::new(provedor, DiarioMemoria::new()).expect("novo");
    assert!(matches!(
        pagamentos.iniciar(pedido()).expect("inicia"),
        Passo::Perguntar(Pergunta::Campo { .. })
    ));
    let passo = pagamentos.continuar(&id(), EntradaOperador::Texto("3".into())).expect("responde");
    assert!(matches!(passo, Passo::Aprovado(_)));
    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
}

/// A fake CliSiTef loop: the rounds a real one might give, through the pure translation, behind the
/// same trait a real helper would implement. There is no library here.
#[derive(Debug, Default)]
struct SiTefFalso {
    ciclo: Option<CicloSiTef>,
    roteiro: Vec<(i32, RodadaSiTef)>,
    respostas: Vec<String>,
}

fn rodada(comando: i32, campo: i32, buffer: &str) -> (i32, RodadaSiTef) {
    (
        RETORNO_CONTINUA,
        RodadaSiTef { comando, tipo_campo: campo, minimo: 0, maximo: 32, buffer: buffer.into() },
    )
}

impl SiTefFalso {
    fn novo() -> Self {
        let mut roteiro = vec![
            rodada(1, 0, "INSIRA OU PASSE O CARTAO"),
            rodada(30, 0, "ULTIMOS 4 DIGITOS"),
            rodada(0, 950, "16501555000157"),
            rodada(0, 951, "01"),
            rodada(0, 952, "AUT123"),
            rodada(0, 134, "HOST77"),
            rodada(0, 121, "CLIENTE"),
            rodada(0, 122, "LOJA"),
        ];
        roteiro.reverse();
        Self { ciclo: None, roteiro, respostas: Vec::new() }
    }

    fn avancar(&mut self) -> Result<Passo, ErroProvedor> {
        let ciclo = self.ciclo.as_mut().ok_or_else(|| ErroProvedor::Protocolo("sem ciclo".into()))?;
        while let Some((_, rodada)) = self.roteiro.pop() {
            if let AcaoSiTef::Passo(passo) = ciclo.rodada(&rodada) {
                return Ok(passo);
            }
        }
        ciclo.fim(0)
    }
}

impl ProvedorPagamento for SiTefFalso {
    fn id(&self) -> ProvedorId {
        ProvedorId::SiTef
    }
    fn capacidades(&self) -> Capacidades {
        Capacidades::SITEF
    }
    fn iniciar(&mut self, pedido: &PedidoPagamento) -> Result<Passo, ErroProvedor> {
        self.ciclo = Some(CicloSiTef::new(pedido.id.clone(), pedido.valor_centavos));
        self.avancar()
    }
    fn continuar(&mut self, _: &IdPagamento, entrada: EntradaOperador) -> Result<Passo, ErroProvedor> {
        let retorno = CicloSiTef::responder(&entrada);
        if retorno.continua == -1 {
            return self.ciclo.as_ref().ok_or(ErroProvedor::Tempo)?.fim(-2);
        }
        self.respostas.push(retorno.buffer);
        self.avancar()
    }
    fn confirmar(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Ok(())
    }
    fn desfazer(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Ok(())
    }
    fn cancelar(&mut self, _: &IdPagamento) -> Result<Cancelamento, ErroProvedor> {
        Ok(Cancelamento::Feito)
    }
    fn estornar(&mut self, _: &ReferenciaTransacao) -> Result<(), ErroProvedor> {
        Err(ErroProvedor::SemSuporte("o estorno interativo não está no esqueleto".into()))
    }
    fn consultar(&mut self, _: &IdPagamento) -> Result<SituacaoProvedor, ErroProvedor> {
        Ok(SituacaoProvedor::Indeterminada)
    }
    fn pendencias(&mut self) -> Result<Vec<ReferenciaTransacao>, ErroProvedor> {
        Ok(Vec::new())
    }
}

#[test]
fn ciclo_do_sitef_passa_pelo_contrato_ate_os_dados_fiscais() {
    let provedor = ProvedorAuxiliar::conectar(CanalLocal::new(SiTefFalso::novo())).expect("conecta");
    assert_eq!(provedor.id(), ProvedorId::SiTef);
    let mut pagamentos = Pagamentos::new(provedor, DiarioMemoria::new()).expect("novo");
    assert!(matches!(
        pagamentos.iniciar(pedido()).expect("inicia"),
        Passo::Perguntar(Pergunta::Mensagem { exige_ok: false, .. })
    ));
    assert!(matches!(
        pagamentos.continuar(&id(), EntradaOperador::Nada).expect("segue"),
        Passo::Perguntar(Pergunta::Campo { .. })
    ));
    let Passo::Aprovado(aprovacao) =
        pagamentos.continuar(&id(), EntradaOperador::Texto("1234".into())).expect("segue")
    else {
        panic!("devia aprovar");
    };
    assert_eq!(aprovacao.dados_fiscais.c_aut.as_deref(), Some("AUT123"));
    assert_eq!(aprovacao.dados_fiscais.t_band.as_deref(), Some("01"));
    assert!(aprovacao.dados_fiscais.atende_ya05_10());
    assert_eq!(aprovacao.vias.via_cliente(), ["CLIENTE"]);
    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
}

/// A channel whose other side vanished, as a helper that crashed.
#[derive(Debug)]
struct CanalMorto;

impl CanalLinhas for CanalMorto {
    fn enviar(&mut self, _: &str) -> Result<(), ErroProvedor> {
        Err(ErroProvedor::Indisponivel("o auxiliar caiu".into()))
    }
    fn receber(&mut self) -> Result<String, ErroProvedor> {
        Err(ErroProvedor::Indisponivel("o auxiliar caiu".into()))
    }
}

#[test]
fn auxiliar_que_nao_abre_e_erro_e_nao_panico() {
    assert!(ProvedorAuxiliar::conectar(CanalMorto).is_err_and(|erro| erro.incerto()));
}

#[cfg(all(feature = "auxiliar-bin", not(target_arch = "wasm32")))]
mod processo_real {
    use ravenrs_pagamento::auxiliar::CanalProcesso;

    use super::*;

    const AUXILIAR: &str = env!("CARGO_BIN_EXE_ravenrs-pagamento-auxiliar-simulado");

    #[test]
    fn venda_num_processo_filho_de_verdade() {
        let canal = CanalProcesso::iniciar(AUXILIAR, &[]).expect("inicia");
        let provedor = ProvedorAuxiliar::conectar(canal).expect("conecta");
        assert_eq!(provedor.id(), ProvedorId::Simulador);
        let mut pagamentos = Pagamentos::new(provedor, DiarioMemoria::new()).expect("novo");
        assert!(matches!(pagamentos.iniciar(pedido()).expect("inicia"), Passo::Aprovado(_)));
        assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
    }

    #[test]
    fn auxiliar_morre_na_confirmacao_e_o_pdv_continua_de_pe() {
        let canal = CanalProcesso::iniciar(AUXILIAR, &["morrer-ao-confirmar"]).expect("inicia");
        let provedor = ProvedorAuxiliar::conectar(canal).expect("conecta");
        let diario = DiarioMemoria::new();
        let mut pagamentos = Pagamentos::new(provedor, diario.clone()).expect("novo");
        pagamentos.iniciar(pedido()).expect("inicia");
        let erro = pagamentos.concluir(&id(), Impressao::Concluida).expect_err("o auxiliar morreu");
        assert!(matches!(erro, ravenrs_pagamento::ErroPagamento::Provedor(e) if e.incerto()));
        assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
        drop(pagamentos);

        // A new helper with a clean simulator: the journal still holds the decision, and carries it out.
        let canal = CanalProcesso::iniciar(AUXILIAR, &[]).expect("reinicia");
        let provedor = ProvedorAuxiliar::conectar(canal).expect("reconecta");
        let mut pagamentos = Pagamentos::new(provedor, diario).expect("religa");
        assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Confirmado);
    }
}
