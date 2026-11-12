//! The adapter against fixtures written by us from the public documentation. No network is touched.
#![allow(clippy::expect_used)]

use std::cell::RefCell;
use std::collections::VecDeque;

use ravenrs_pagamento::http::{ErroTransporte, Metodo, RequisicaoHttp, RespostaHttp, TransporteHttp};
use ravenrs_pagamento::mercado_pago::{
    EventoSimulado, MercadoPagoPoint, TERMINAL_VIRTUAL, TokenAcesso, ler_notificacao,
};
use ravenrs_pagamento::{
    AlertaFiscal, Cancelamento, Desfecho, DiarioMemoria, EntradaOperador, ErroPagamento, ErroProvedor,
    Estado, IdPagamento, Impressao, Pagamentos, Passo, PedidoPagamento, ProvedorPagamento, SituacaoProvedor,
    TipoPagamento,
};

const CRIADA: &str = include_str!("fixtures/mercado_pago/ordem_criada.json");
const PROCESSADA: &str = include_str!("fixtures/mercado_pago/ordem_processada.json");
const FALHOU: &str = include_str!("fixtures/mercado_pago/ordem_falhou.json");
const CANCELADA: &str = include_str!("fixtures/mercado_pago/ordem_cancelada.json");
const EM_CANCELAMENTO: &str = include_str!("fixtures/mercado_pago/ordem_em_cancelamento.json");
const ESTORNADA: &str = include_str!("fixtures/mercado_pago/ordem_estornada.json");
const NOTIFICACAO: &str = include_str!("fixtures/mercado_pago/notificacao.json");

#[derive(Default)]
struct Roteiro {
    respostas: RefCell<VecDeque<Result<RespostaHttp, ErroTransporte>>>,
    pedidos: RefCell<Vec<RequisicaoHttp>>,
}

impl Roteiro {
    fn com(self, status: u16, corpo: &str) -> Self {
        self.respostas.borrow_mut().push_back(Ok(RespostaHttp { status, corpo: corpo.as_bytes().to_vec() }));
        self
    }

    fn com_erro(self, erro: ErroTransporte) -> Self {
        self.respostas.borrow_mut().push_back(Err(erro));
        self
    }
}

impl TransporteHttp for Roteiro {
    fn enviar(&self, requisicao: &RequisicaoHttp) -> Result<RespostaHttp, ErroTransporte> {
        self.pedidos.borrow_mut().push(requisicao.clone());
        self.respostas
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(ErroTransporte::Rede("sem roteiro".into())))
    }
}

fn adaptador(roteiro: &Roteiro) -> MercadoPagoPoint<&Roteiro> {
    MercadoPagoPoint::new(roteiro, TokenAcesso::new("TEST-token"), TERMINAL_VIRTUAL)
}

fn id() -> IdPagamento {
    IdPagamento::new("venda-1").expect("id")
}

fn pedido(tipo: TipoPagamento) -> PedidoPagamento {
    PedidoPagamento::new(id(), 1990, tipo).expect("pedido")
}

#[test]
fn criar_manda_o_que_a_documentacao_pede() {
    let roteiro = Roteiro::default().com(201, CRIADA);
    let mut mp = adaptador(&roteiro);
    assert!(matches!(mp.iniciar(&pedido(TipoPagamento::Debito)), Ok(Passo::Aguardar { .. })));
    let pedidos = roteiro.pedidos.borrow();
    let enviado = &pedidos[0];
    assert_eq!(enviado.metodo, Metodo::Post);
    assert_eq!(enviado.url, "https://api.mercadopago.com/v1/orders");
    assert_eq!(enviado.cabecalho("Authorization"), Some("Bearer TEST-token"));
    assert_eq!(enviado.cabecalho("X-Idempotency-Key"), Some("ravenrs-venda-1"));
    let corpo = enviado.json().expect("json");
    assert_eq!(corpo["type"], "point");
    assert_eq!(corpo["external_reference"], "venda-1");
    assert_eq!(corpo["transactions"]["payments"][0]["amount"], "19.90");
    assert_eq!(corpo["config"]["point"]["terminal_id"], TERMINAL_VIRTUAL);
    assert_eq!(corpo["config"]["payment_method"]["default_type"], "debit_card");
}

#[test]
fn parcelas_levam_quem_paga_os_juros() {
    let roteiro = Roteiro::default().com(201, CRIADA);
    let mut mp = adaptador(&roteiro);
    mp.iniciar(&pedido(TipoPagamento::Credito { parcelas: 3 })).expect("cria");
    let corpo = roteiro.pedidos.borrow()[0].json().expect("json");
    assert_eq!(corpo["config"]["payment_method"]["default_type"], "credit_card");
    assert_eq!(corpo["config"]["payment_method"]["default_installments"], 3);
    assert_eq!(corpo["config"]["payment_method"]["installments_cost"], "seller");
}

#[test]
fn venda_inteira_com_c_aut_ausente_explicito() {
    let roteiro = Roteiro::default().com(201, CRIADA).com(200, PROCESSADA);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    assert!(matches!(
        pagamentos.iniciar(pedido(TipoPagamento::Escolher)).expect("cria"),
        Passo::Aguardar { .. }
    ));
    let Passo::Aprovado(aprovacao) = pagamentos.continuar(&id(), EntradaOperador::Nada).expect("consulta")
    else {
        panic!("devia aprovar");
    };
    let fiscais = &aprovacao.dados_fiscais;
    assert_eq!(fiscais.c_aut, None, "reference_id não pode virar cAut");
    assert!(matches!(fiscais.alertas[0], AlertaFiscal::CAutAusente { .. }));
    assert!(!fiscais.atende_ya05_10());
    assert_eq!(fiscais.t_band.as_deref(), Some("02"));
    assert_eq!(fiscais.cnpj.as_deref(), Some("10573521000191"));
    assert!(fiscais.alertas.iter().any(|a| matches!(a, AlertaFiscal::CnpjDeTabelaNaoConferida { .. })));
    assert_eq!(fiscais.id_term_pag.as_deref(), Some(TERMINAL_VIRTUAL));
    assert_eq!(aprovacao.referencia.controle.as_deref(), Some("ORD01"));
    assert_eq!(aprovacao.referencia.valor_centavos, 1990);
    assert!(aprovacao.vias.impressas_no_terminal);
    assert!(aprovacao.vias.vazias());
    assert_eq!(aprovacao.tipo_efetivo, Some(TipoPagamento::Credito { parcelas: 6 }));
    // There is no third leg: finishing the sale calls nothing.
    let antes = roteiro.pedidos.borrow().len();
    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
    assert_eq!(roteiro.pedidos.borrow().len(), antes);
}

#[test]
fn recusa_vira_negado_com_o_detalhe() {
    let roteiro = Roteiro::default().com(201, CRIADA).com(200, FALHOU);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("cria");
    let Passo::Negado { codigo, .. } = pagamentos.continuar(&id(), EntradaOperador::Nada).expect("consulta")
    else {
        panic!("devia negar");
    };
    assert_eq!(codigo, "insufficient_amount");
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Negado);
}

#[test]
fn cancelar_em_created_e_sincrono_e_at_terminal_nao() {
    let roteiro = Roteiro::default().com(201, CRIADA).com(200, CANCELADA);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("cria");
    assert_eq!(pagamentos.cancelar(&id()).expect("cancela"), Cancelamento::Feito);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Cancelado);
    let pedidos = roteiro.pedidos.borrow();
    assert_eq!(pedidos[1].url, "https://api.mercadopago.com/v1/orders/ORD01/cancel");
    assert_eq!(pedidos[1].cabecalho("x-allow-cancelable-status"), Some("at_terminal"));
    assert!(pedidos[1].cabecalho("X-Idempotency-Key").is_some());

    let roteiro = Roteiro::default().com(201, CRIADA).com(202, EM_CANCELAMENTO).com(200, CANCELADA);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("cria");
    assert_eq!(pagamentos.cancelar(&id()).expect("pede"), Cancelamento::Solicitado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aguardando);
    assert!(matches!(pagamentos.continuar(&id(), EntradaOperador::Nada), Ok(Passo::Cancelado { .. })));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Cancelado);
}

#[test]
fn estorno_sem_corpo_e_com_a_ordem_do_diario() {
    let roteiro = Roteiro::default().com(201, CRIADA).com(200, PROCESSADA).com(200, ESTORNADA);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("cria");
    pagamentos.continuar(&id(), EntradaOperador::Nada).expect("consulta");
    pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
    assert_eq!(pagamentos.estornar(&id()).expect("estorna"), Estado::Estornado);
    let pedidos = roteiro.pedidos.borrow();
    assert_eq!(pedidos[2].url, "https://api.mercadopago.com/v1/orders/ORD01/refund");
    assert_eq!(pedidos[2].corpo, None);
}

#[test]
fn simular_eventos_usa_o_corpo_documentado() {
    let roteiro = Roteiro::default().com(204, "").com(204, "").com(204, "");
    let mp = adaptador(&roteiro);
    mp.simular_evento(
        "ORD01",
        &EventoSimulado::Processado { tipo: "credit_card".into(), metodo: "master".into(), parcelas: 1 },
    )
    .expect("processado");
    mp.simular_evento(
        "ORD01",
        &EventoSimulado::Falhou {
            tipo: "credit_card".into(),
            metodo: "master".into(),
            parcelas: 1,
            detalhe: "high_risk".into(),
        },
    )
    .expect("falhou");
    mp.simular_evento("ORD01", &EventoSimulado::AcaoNecessaria).expect("ação");
    let pedidos = roteiro.pedidos.borrow();
    assert_eq!(pedidos[0].url, "https://api.mercadopago.com/v1/orders/ORD01/events");
    assert_eq!(pedidos[0].json().expect("json")["status_detail"], "accredited");
    assert_eq!(pedidos[1].json().expect("json")["status_detail"], "high_risk");
    assert_eq!(pedidos[2].json().expect("json")["status"], "action_required");
}

#[test]
fn queda_sem_resposta_ao_criar_fica_indeterminada_e_nao_cancela_sozinha() {
    let roteiro = Roteiro::default().com_erro(ErroTransporte::Tempo);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), DiarioMemoria::new()).expect("novo");
    let erro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect_err("tempo");
    assert_eq!(erro, ErroPagamento::Provedor(ErroProvedor::Tempo));
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Indeterminado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aguardando);
}

#[test]
fn id_da_ordem_vai_ao_diario_e_volta_depois_da_queda() {
    let diario = DiarioMemoria::new();
    let roteiro = Roteiro::default().com(201, CRIADA);
    {
        let mut pagamentos = Pagamentos::new(adaptador(&roteiro), diario.clone()).expect("novo");
        pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("cria");
    }
    // After the restart it finds the order and learns that money was taken with no sale finished.
    let roteiro = Roteiro::default().com(200, PROCESSADA).com(200, ESTORNADA);
    let mut pagamentos = Pagamentos::new(adaptador(&roteiro), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::CobradoSemConclusao);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
    assert_eq!(roteiro.pedidos.borrow()[0].url, "https://api.mercadopago.com/v1/orders/ORD01");
    assert_eq!(pagamentos.estornar(&id()).expect("estorna"), Estado::Estornado);
}

#[test]
fn desfazer_nao_existe_e_nao_finge() {
    let roteiro = Roteiro::default();
    let mut mp = adaptador(&roteiro);
    assert!(!mp.capacidades().terceira_perna);
    assert!(matches!(
        mp.desfazer(&ravenrs_pagamento::ReferenciaTransacao::default()),
        Err(ErroProvedor::SemSuporte(_))
    ));
    assert_eq!(mp.pendencias(), Ok(Vec::new()));
    assert!(roteiro.pedidos.borrow().is_empty());
}

#[test]
fn erros_http_se_separam_em_recusa_certa_e_incerteza() {
    let roteiro =
        Roteiro::default().com(400, r#"{"errors":[{"code":"missing_installments_cost","message":"falta"}]}"#);
    let mut mp = adaptador(&roteiro);
    let erro = mp.iniciar(&pedido(TipoPagamento::Debito)).expect_err("recusa");
    assert_eq!(
        erro,
        ErroProvedor::Recusado { codigo: "missing_installments_cost".into(), motivo: "falta".into() }
    );

    let roteiro = Roteiro::default().com(503, "");
    let mut mp = adaptador(&roteiro);
    assert!(mp.iniciar(&pedido(TipoPagamento::Debito)).is_err_and(|erro| erro.incerto()));
}

#[test]
fn consulta_mapeia_todos_os_status() {
    let casos = [
        (PROCESSADA, "aprovada"),
        (FALHOU, "negada"),
        (CANCELADA, "cancelada"),
        (ESTORNADA, "estornada"),
        (CRIADA, "andamento"),
    ];
    for (corpo, esperado) in casos {
        let roteiro = Roteiro::default().com(200, corpo);
        let mut mp = adaptador(&roteiro);
        mp.lembrar_ordem(id(), "ORD01");
        let situacao = mp.consultar(&id()).expect("consulta");
        let nome = match situacao {
            SituacaoProvedor::Aprovada(_) => "aprovada",
            SituacaoProvedor::Negada { .. } => "negada",
            SituacaoProvedor::Cancelada => "cancelada",
            SituacaoProvedor::Estornada => "estornada",
            SituacaoProvedor::EmAndamento => "andamento",
            _ => "outra",
        };
        assert_eq!(nome, esperado);
    }
}

#[test]
fn notificacao_e_lida_sem_prometer_assinatura() {
    let notificacao = ler_notificacao(NOTIFICACAO.as_bytes()).expect("lê");
    assert_eq!(notificacao.acao, "order.processed");
    assert_eq!(notificacao.ordem, "ORD01");
    assert_eq!(notificacao.status.as_deref(), Some("processed"));
    assert_eq!(notificacao.referencia_externa.as_deref(), Some("venda-1"));
    assert_eq!(ler_notificacao(b"{}"), None);
}

#[test]
fn servidor_proprio_no_lugar_da_api() {
    let roteiro = Roteiro::default().com(201, CRIADA);
    let mut mp = adaptador(&roteiro).com_base("https://pagamentos.exemplo.invalid");
    mp.iniciar(&pedido(TipoPagamento::Pix)).expect("cria");
    let pedidos = roteiro.pedidos.borrow();
    assert_eq!(pedidos[0].url, "https://pagamentos.exemplo.invalid/v1/orders");
    assert_eq!(pedidos[0].json().expect("json")["config"]["payment_method"]["default_type"], "qr");
}
