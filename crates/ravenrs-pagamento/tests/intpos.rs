//! The exchange of files, on real files, against the simulated manager.
#![allow(clippy::expect_used)]
#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::path::PathBuf;

use ravenrs_pagamento::intpos::{
    ArquivoIntPos, Caminhos, GerenciadorSimulado, Perfil, ProvedorArquivos, RelogioComGerenciador,
    RespostaGerenciador,
};
use ravenrs_pagamento::{
    AlertaFiscal, Desfecho, DiarioMemoria, EntradaOperador, ErroPagamento, ErroProvedor, Estado, IdPagamento,
    Impressao, Pagamentos, Passo, PedidoPagamento, ProvedorId, ProvedorPagamento, RelogioManual,
    SituacaoProvedor, TipoPagamento,
};

struct Ambiente {
    pasta: PathBuf,
    caminhos: Caminhos,
    gerenciador: GerenciadorSimulado,
    relogio: RelogioManual,
}

impl Ambiente {
    fn novo(nome: &str) -> Self {
        let pasta =
            std::env::temp_dir().join(format!("ravenrs-pagamento-intpos-{}-{nome}", std::process::id()));
        let _ = fs::remove_dir_all(&pasta);
        let caminhos = Caminhos::em(&pasta);
        Self {
            pasta,
            gerenciador: GerenciadorSimulado::new(caminhos.clone()),
            caminhos,
            relogio: RelogioManual::new(),
        }
    }

    fn provedor(&self, perfil: Perfil) -> ProvedorArquivos<RelogioComGerenciador> {
        let relogio =
            RelogioComGerenciador { relogio: self.relogio.clone(), gerenciador: self.gerenciador.clone() };
        ProvedorArquivos::new(self.caminhos.clone(), perfil, relogio).expect("provedor")
    }
}

impl Drop for Ambiente {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.pasta);
    }
}

fn paygo() -> Perfil {
    Perfil::PayGo {
        empresa: "RAVENRS".into(),
        aplicacao: "POC".into(),
        versao: "0.1".into(),
        registro_certificacao: "SEM-REGISTRO".into(),
    }
}

fn id() -> IdPagamento {
    IdPagamento::new("venda-1").expect("id")
}

fn pedido(tipo: TipoPagamento) -> PedidoPagamento {
    PedidoPagamento::new(id(), 1990, tipo).expect("pedido")
}

/// Runs the manager and the flow in turns until the step is no longer a wait.
fn conduzir<D: ravenrs_pagamento::Diario>(
    ambiente: &Ambiente,
    pagamentos: &mut Pagamentos<ProvedorArquivos<RelogioComGerenciador>, D>,
    mut passo: Passo,
) -> Result<Passo, ErroPagamento> {
    for _ in 0..50 {
        if !matches!(passo, Passo::Aguardar { .. }) {
            return Ok(passo);
        }
        ambiente.gerenciador.passo().expect("passo do gerenciador");
        ambiente.relogio.avancar(250);
        passo = pagamentos.continuar(&id(), EntradaOperador::Nada)?;
    }
    panic!("não terminou: {passo:?}");
}

#[test]
fn venda_inteira_com_tmp_renomeado_sts_resposta_e_cnf() {
    let ambiente = Ambiente::novo("venda");
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");

    // The rename happened: the manager sees the whole file, and the temporary one is gone.
    assert!(ambiente.caminhos.req.exists());
    assert!(!ambiente.caminhos.req_tmp.exists());
    let escrito = ArquivoIntPos::ler(&fs::read(&ambiente.caminhos.req).expect("req"));
    assert_eq!(escrito.comando(), Some("CRT"));
    assert_eq!(escrito.valor(3), Some("1990"));
    assert_eq!(escrito.valor(4), Some("0"));
    assert_eq!(escrito.valor(731), Some("2"));
    for campo in [706, 716, 733, 735, 736, 738] {
        assert!(escrito.valor(campo).is_some(), "falta o campo {campo}");
    }
    assert!(escrito.completo());

    let Passo::Aprovado(aprovacao) = conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz") else {
        panic!("devia aprovar");
    };
    assert_eq!(aprovacao.dados_fiscais.c_aut.as_deref(), Some("A00001"));
    assert_eq!(aprovacao.dados_fiscais.t_band.as_deref(), Some("02"));
    assert_eq!(aprovacao.dados_fiscais.cnpj.as_deref(), Some("16501555000157"));
    assert!(
        aprovacao
            .dados_fiscais
            .alertas
            .iter()
            .any(|a| matches!(a, AlertaFiscal::CnpjDeTabelaNaoConferida { .. }))
    );
    assert_eq!(aprovacao.vias.via_cliente()[0], "VIA DO CLIENTE (SIMULADOR)");
    assert_eq!(aprovacao.vias.via_estabelecimento()[0], "VIA DA LOJA (SIMULADOR)");
    assert_eq!(aprovacao.tipo_efetivo, Some(TipoPagamento::Debito));
    assert_eq!(ambiente.gerenciador.pendentes(), ["CTRL0001"]);

    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
    assert_eq!(ambiente.gerenciador.confirmados(), ["CTRL0001"]);
    assert!(ambiente.gerenciador.pendentes().is_empty());
    assert_eq!(ambiente.gerenciador.comandos(), ["CRT", "CNF"]);
    let cnf = &ambiente.gerenciador.pedidos()[1];
    for campo in [0, 1, 27, 733, 738] {
        assert!(cnf.valor(campo).is_some(), "o CNF não tem o campo {campo}");
    }
    assert!(!ambiente.caminhos.pendente.exists(), "a nota sai junto com a pendência");
}

#[test]
fn gerenciador_padrao_nao_manda_os_campos_da_paygo() {
    let ambiente = Ambiente::novo("padrao");
    let mut provedor = ambiente.provedor(Perfil::GerenciadorPadrao);
    assert_eq!(provedor.id(), ProvedorId::GerenciadorPadrao);
    provedor.iniciar(&pedido(TipoPagamento::Escolher)).expect("inicia");
    let escrito = ArquivoIntPos::ler(&fs::read(&ambiente.caminhos.req).expect("req"));
    assert_eq!(escrito.valor(733), None);
    assert_eq!(escrito.valor(731), None);
}

#[test]
fn papel_acabou_vira_ncn() {
    let ambiente = Ambiente::novo("ncn");
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz");
    assert_eq!(pagamentos.concluir(&id(), Impressao::Falhou).expect("conclui"), Estado::Desfeito);
    assert_eq!(ambiente.gerenciador.desfeitos(), ["CTRL0001"]);
    assert_eq!(ambiente.gerenciador.comandos(), ["CRT", "NCN"]);
}

#[test]
fn negado_nao_deixa_pendencia() {
    let ambiente = Ambiente::novo("negado");
    ambiente.gerenciador.programar(RespostaGerenciador::Negar("5".into(), "NAO AUTORIZADA".into()));
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    let Passo::Negado { codigo, motivo } = conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz")
    else {
        panic!("devia negar");
    };
    assert_eq!((codigo.as_str(), motivo.as_str()), ("5", "NAO AUTORIZADA"));
    assert!(!ambiente.caminhos.pendente.exists());
    assert!(!ambiente.caminhos.resp.exists());
}

#[test]
fn sem_autorizacao_vira_alerta() {
    let ambiente = Ambiente::novo("semaut");
    ambiente.gerenciador.programar(RespostaGerenciador::AprovarSemAutorizacao);
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    let Passo::Aprovado(aprovacao) = conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz") else {
        panic!("devia aprovar");
    };
    assert_eq!(aprovacao.dados_fiscais.c_aut, None);
    assert!(aprovacao.dados_fiscais.alertas.iter().any(|a| matches!(a, AlertaFiscal::CAutAusente { .. })));
}

#[test]
fn pix_usa_o_campo_certo_e_avisa() {
    let ambiente = Ambiente::novo("pix");
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Pix)).expect("inicia");
    let escrito = ArquivoIntPos::ler(&fs::read(&ambiente.caminhos.req).expect("req"));
    assert_eq!(escrito.valor(749), Some("8"));
    let Passo::Aprovado(aprovacao) = conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz") else {
        panic!("devia aprovar");
    };
    assert_eq!(aprovacao.dados_fiscais.c_aut, aprovacao.referencia.nsu);
    assert_eq!(aprovacao.dados_fiscais.t_band, None);
    assert!(aprovacao.dados_fiscais.alertas.contains(&AlertaFiscal::CAutPixPeloEndToEnd));
    assert_eq!(aprovacao.tipo_efetivo, Some(TipoPagamento::Pix));
}

#[test]
fn gerenciador_desligado_e_recusa_certa_pois_o_pedido_e_retirado() {
    let ambiente = Ambiente::novo("desligado");
    ambiente.gerenciador.desligar(true);
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let mut passo = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    for _ in 0..40 {
        if !matches!(passo, Passo::Aguardar { .. }) {
            break;
        }
        ambiente.relogio.avancar(250);
        passo = pagamentos.continuar(&id(), EntradaOperador::Nada).expect("continua");
    }
    assert!(matches!(&passo, Passo::Negado { codigo, .. } if codigo == "GP-INATIVO"), "{passo:?}");
    assert!(!ambiente.caminhos.req.exists());
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Negado);
}

#[test]
fn queda_entre_aprovar_e_confirmar_vira_ncn_ao_religar() {
    let ambiente = Ambiente::novo("queda");
    let diario = DiarioMemoria::new();
    {
        let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario.clone()).expect("novo");
        let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
        conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz");
    }
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Desfeito);
    assert_eq!(ambiente.gerenciador.desfeitos(), ["CTRL0001"]);
    assert!(ambiente.gerenciador.pendentes().is_empty());
}

#[test]
fn queda_com_a_resposta_ja_no_disco_nao_perde_a_aprovacao() {
    let ambiente = Ambiente::novo("resposta");
    let diario = DiarioMemoria::new();
    {
        let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario.clone()).expect("novo");
        pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
        ambiente.gerenciador.passo().expect("responde");
        // The power falls before our side reads the response.
    }
    assert!(ambiente.caminhos.resp.exists());
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Desfeito);
    assert_eq!(ambiente.gerenciador.desfeitos(), ["CTRL0001"]);
    assert!(!ambiente.caminhos.pendente.exists());
}

#[test]
fn queda_com_o_pedido_nao_lido_e_retirado_ao_religar() {
    let ambiente = Ambiente::novo("naolido");
    let diario = DiarioMemoria::new();
    {
        let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario.clone()).expect("novo");
        pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    }
    assert!(ambiente.caminhos.req.exists());
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario).expect("religa");
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::CanceladoSemEnvio);
    assert!(!ambiente.caminhos.req.exists());
    assert!(ambiente.gerenciador.comandos().is_empty());
}

#[test]
fn queda_com_o_gerenciador_trabalhando_fica_indeterminada_e_nao_chuta() {
    let ambiente = Ambiente::novo("trabalhando");
    ambiente.gerenciador.programar(RespostaGerenciador::Calar);
    let diario = DiarioMemoria::new();
    {
        let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario.clone()).expect("novo");
        pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
        ambiente.gerenciador.passo().expect("pega");
    }
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Indeterminado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aguardando);
}

#[test]
fn pendencia_de_sessao_anterior_sem_diario_vira_ncn() {
    let ambiente = Ambiente::novo("anterior");
    {
        let mut provedor = ambiente.provedor(paygo());
        provedor.iniciar(&pedido(TipoPagamento::Debito)).expect("inicia");
        ambiente.gerenciador.passo().expect("responde");
    }
    // A journal that knows nothing about it, as after reinstalling.
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::PendenciaDesfeita);
    assert_eq!(relatorio[0].id, None);
    assert_eq!(ambiente.gerenciador.desfeitos(), ["CTRL0001"]);
    assert!(pagamentos.recuperar().expect("de novo").is_empty());
}

#[test]
fn nao_comeca_com_aprovacao_no_ar() {
    let ambiente = Ambiente::novo("noar");
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz");
    let outro = PedidoPagamento::new(IdPagamento::new("venda-2").expect("id"), 100, TipoPagamento::Debito)
        .expect("p");
    let Passo::Negado { codigo, .. } = pagamentos.iniciar(outro).expect("responde") else {
        panic!("devia recusar")
    };
    assert_eq!(codigo, "PENDENCIA");
    assert_eq!(ambiente.gerenciador.comandos(), ["CRT"]);
}

#[test]
fn estorno_e_cnc_seguido_de_cnf() {
    let ambiente = Ambiente::novo("estorno");
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), DiarioMemoria::new()).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    let Passo::Aprovado(aprovacao) = conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz") else {
        panic!("devia aprovar");
    };
    pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
    assert_eq!(pagamentos.estornar(&id()).expect("estorna"), Estado::Estornado);
    assert_eq!(ambiente.gerenciador.comandos(), ["CRT", "CNF", "CNC", "CNF"]);
    assert_eq!(ambiente.gerenciador.cancelados(), [aprovacao.referencia.nsu.expect("nsu")]);
    let cnc = &ambiente.gerenciador.pedidos()[2];
    assert_eq!(cnc.valor(3), Some("1990"));
    assert!(cnc.valor(22).is_some());
    assert!(ambiente.gerenciador.pendentes().is_empty());
}

#[test]
fn atv_diz_se_ha_gerenciador() {
    let ambiente = Ambiente::novo("atv");
    let mut provedor = ambiente.provedor(paygo());
    assert!(provedor.ativo().expect("ativo"));
    ambiente.gerenciador.desligar(true);
    assert!(!provedor.ativo().expect("inativo"));
    assert!(!ambiente.caminhos.req.exists());
}

#[test]
fn cnf_sem_sts_e_incerto_e_mantem_a_pendencia() {
    let ambiente = Ambiente::novo("cnfsemsts");
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(ambiente.provedor(paygo()), diario).expect("novo");
    let primeiro = pagamentos.iniciar(pedido(TipoPagamento::Debito)).expect("inicia");
    conduzir(&ambiente, &mut pagamentos, primeiro).expect("conduz");
    ambiente.gerenciador.desligar(true);
    let erro = pagamentos.concluir(&id(), Impressao::Concluida).expect_err("sem sts");
    assert_eq!(erro, ErroPagamento::Provedor(ErroProvedor::Tempo));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
    assert!(ambiente.caminhos.pendente.exists());
    ambiente.gerenciador.desligar(false);
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Confirmado);
    assert_eq!(ambiente.gerenciador.confirmados(), ["CTRL0001"]);
}

#[test]
fn resposta_atrasada_fica_aguardando_e_resposta_de_outro_e_ignorada() {
    let ambiente = Ambiente::novo("atrasada");
    ambiente.gerenciador.atrasar_resposta();
    let mut provedor = ambiente.provedor(paygo());
    provedor.iniciar(&pedido(TipoPagamento::Debito)).expect("inicia");
    ambiente.gerenciador.passo().expect("sts");
    assert!(matches!(provedor.continuar(&id(), EntradaOperador::Nada), Ok(Passo::Aguardar { .. })));
    assert!(matches!(provedor.continuar(&id(), EntradaOperador::Cancelar), Ok(Passo::Aguardar { .. })));

    // Someone else's response in the folder is no answer to this request.
    let mut outra = ArquivoIntPos::novo("CRT", 1);
    outra.definir(9, 0, "0");
    fs::write(&ambiente.caminhos.resp, outra.codificar()).expect("escreve");
    assert!(matches!(provedor.continuar(&id(), EntradaOperador::Nada), Ok(Passo::Aguardar { .. })));
    assert!(matches!(provedor.consultar(&id()), Ok(SituacaoProvedor::EmAndamento)));
}

#[test]
fn resposta_pela_metade_nao_e_lida() {
    let ambiente = Ambiente::novo("metade");
    let mut provedor = ambiente.provedor(paygo());
    provedor.iniciar(&pedido(TipoPagamento::Debito)).expect("inicia");
    let numero = ArquivoIntPos::ler(&fs::read(&ambiente.caminhos.req).expect("req"))
        .identificacao()
        .expect("id")
        .to_owned();
    ambiente.gerenciador.programar(RespostaGerenciador::Calar);
    ambiente.gerenciador.passo().expect("sts");
    assert!(matches!(provedor.continuar(&id(), EntradaOperador::Nada), Ok(Passo::Aguardar { .. })));
    fs::write(&ambiente.caminhos.resp, format!("000-000 = CRT\r\n001-000 = {numero}\r\n009-000 = 0\r\n"))
        .expect("escreve");
    assert!(matches!(provedor.continuar(&id(), EntradaOperador::Nada), Ok(Passo::Aguardar { .. })));
}
