//! The promise of the journal: wherever the power falls, nothing is lost and nothing charges twice.
#![allow(clippy::expect_used)]

use ravenrs_pagamento::simulador::{
    CAPACIDADES_PADRAO, Operacao, SimuladorProvedor, SituacaoRazao, cenarios, confirmar, consultar,
    continuar, desfazer, estornar, iniciar, pendencias,
};
use ravenrs_pagamento::{
    AlertaFiscal, AvisoAprovacao, Cancelamento, Capacidades, Conteudo, Desfecho, Diario, DiarioMemoria,
    EntradaOperador, ErroPagamento, ErroProvedor, Estado, IdPagamento, Impressao, Pagamentos, Passo,
    PedidoPagamento, Pergunta, SituacaoProvedor, TipoPagamento,
};

fn id() -> IdPagamento {
    IdPagamento::new("venda-1").expect("id")
}

fn pedido() -> PedidoPagamento {
    PedidoPagamento::new(id(), 1990, TipoPagamento::Debito).expect("pedido")
}

fn sem_terceira_perna() -> Capacidades {
    Capacidades { terceira_perna: false, ..CAPACIDADES_PADRAO }
}

/// The whole sale, stopping at the first failure of the journal.
fn venda(pagamentos: &mut Pagamentos<SimuladorProvedor, DiarioMemoria>) -> Result<(), ErroPagamento> {
    pagamentos.iniciar(pedido())?;
    pagamentos.concluir(&id(), Impressao::Concluida)?;
    Ok(())
}

/// How many lines the whole sale writes.
fn linhas_da_venda(capacidades: Capacidades) -> usize {
    let diario = DiarioMemoria::new();
    let simulador = SimuladorProvedor::new().com_capacidades(capacidades);
    let mut pagamentos = Pagamentos::new(simulador, diario.clone()).expect("novo");
    venda(&mut pagamentos).expect("venda");
    diario.tamanho()
}

#[test]
fn caminho_feliz_escreve_tudo_antes() {
    let diario = DiarioMemoria::new();
    let simulador = SimuladorProvedor::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
    let Passo::Aprovado(aprovacao) = pagamentos.iniciar(pedido()).expect("inicia") else { panic!("aprova") };
    assert!(aprovacao.dados_fiscais.atende_ya05_10());
    assert_eq!(pagamentos.registro(&id()).expect("registro").estado, Estado::Aprovado);
    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Confirmada));
    let ordem: Vec<&'static str> = diario
        .ler()
        .expect("lê")
        .iter()
        .map(|l| match &l.conteudo {
            Conteudo::Aberto { .. } => "aberto",
            Conteudo::Intencao { .. } => "intencao",
            Conteudo::Rastro { .. } => "rastro",
            Conteudo::Transicao { .. } => "transicao",
        })
        .collect();
    assert_eq!(ordem, ["aberto", "intencao", "transicao", "transicao", "intencao", "transicao"]);
}

/// Cuts the power after each line of the journal, in turn, comes back, recovers and looks.
#[test]
fn queda_em_cada_ponto_com_terceira_perna() {
    let total = linhas_da_venda(CAPACIDADES_PADRAO);
    assert_eq!(total, 6);
    let mut desfechos = Vec::new();
    for corte in 0..=total {
        let diario = DiarioMemoria::new();
        let simulador = SimuladorProvedor::new();
        diario.cair_depois_de(corte);
        {
            let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
            let resultado = venda(&mut pagamentos);
            assert_eq!(resultado.is_err(), corte < total, "corte {corte}");
        }
        diario.religar();

        let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("religa");
        let relatorio = pagamentos.recuperar().expect("recupera");
        let estado = pagamentos.registro(&id()).map(|r| r.estado);
        let razao = simulador.situacao(&id());
        desfechos.push((corte, estado, razao.clone()));

        // Nothing stays open, on either side, and the sides agree.
        assert!(!estado.is_some_and(Estado::pendente), "corte {corte}: {estado:?} {relatorio:?}");
        match estado {
            None | Some(Estado::Cancelado) => assert!(razao.is_none(), "corte {corte}: {razao:?}"),
            Some(Estado::Desfeito) => assert_eq!(razao, Some(SituacaoRazao::Desfeita), "corte {corte}"),
            Some(Estado::Confirmado) => assert_eq!(razao, Some(SituacaoRazao::Confirmada), "corte {corte}"),
            outro => panic!("corte {corte}: {outro:?}"),
        }
        // A second recovery finds nothing to do.
        assert_eq!(pagamentos.recuperar().expect("de novo"), Vec::new(), "corte {corte}");
        // And the id stays burned.
        if estado.is_some() {
            assert!(matches!(pagamentos.iniciar(pedido()), Err(ErroPagamento::JaExiste(_))));
        }
    }
    assert_eq!(
        desfechos.iter().map(|(_, estado, _)| *estado).collect::<Vec<_>>(),
        vec![
            None,                     // nothing was written
            Some(Estado::Cancelado),  // opened
            Some(Estado::Cancelado),  // intention written, provider never called
            Some(Estado::Desfeito),   // sent and approved, the answer not written
            Some(Estado::Desfeito),   // between approving and confirming
            Some(Estado::Confirmado), // the decision to confirm was written
            Some(Estado::Confirmado), // all done
        ]
    );
}

#[test]
fn queda_em_cada_ponto_sem_terceira_perna() {
    let total = linhas_da_venda(sem_terceira_perna());
    for corte in 0..=total {
        let diario = DiarioMemoria::new();
        let simulador = SimuladorProvedor::new().com_capacidades(sem_terceira_perna());
        diario.cair_depois_de(corte);
        {
            let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
            let _ = venda(&mut pagamentos);
        }
        diario.religar();
        let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("religa");
        let relatorio = pagamentos.recuperar().expect("recupera");
        let cobrado = simulador.situacao(&id()) == Some(SituacaoRazao::Pendente);
        let estado = pagamentos.registro(&id()).map(|r| r.estado);
        if cobrado && estado == Some(Estado::Aprovado) {
            // Money was taken and the sale did not end: no silence and no invented reversal.
            assert_eq!(relatorio[0].desfecho, Desfecho::CobradoSemConclusao, "corte {corte}");
            assert_eq!(pagamentos.estornar(&id()).expect("estorna"), Estado::Estornado);
            assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Estornada));
        } else if cobrado {
            assert_eq!(estado, Some(Estado::Confirmado), "corte {corte}");
        }
        assert!(simulador.requests_de(Operacao::Desfazer).is_empty(), "corte {corte}");
    }
}

/// A second power loss in the middle of the recovery, at each line of it.
#[test]
fn queda_durante_a_recuperacao() {
    for corte_venda in 0..=6usize {
        for corte_recuperacao in 0..=4usize {
            let diario = DiarioMemoria::new();
            let simulador = SimuladorProvedor::new();
            diario.cair_depois_de(corte_venda);
            {
                let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
                let _ = venda(&mut pagamentos);
            }
            diario.religar();
            diario.cair_depois_de(corte_recuperacao);
            {
                let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("religa");
                let _ = pagamentos.recuperar();
            }
            diario.religar();
            let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("religa de novo");
            pagamentos.recuperar().expect("recupera");
            let estado = pagamentos.registro(&id()).map(|r| r.estado);
            assert!(!estado.is_some_and(Estado::pendente), "{corte_venda}/{corte_recuperacao}: {estado:?}");
            assert_ne!(
                simulador.situacao(&id()),
                Some(SituacaoRazao::Pendente),
                "{corte_venda}/{corte_recuperacao}"
            );
        }
    }
}

#[test]
fn diario_que_falha_impede_de_chamar_o_provedor() {
    let diario = DiarioMemoria::new();
    let simulador = SimuladorProvedor::new();
    diario.cair_depois_de(2);
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("novo");
    assert!(matches!(pagamentos.iniciar(pedido()), Err(ErroPagamento::Diario(_))));
    assert!(simulador.requests().is_empty());
}

#[test]
fn id_repetido_nao_cobra_duas_vezes() {
    let simulador = SimuladorProvedor::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    assert!(matches!(pagamentos.iniciar(pedido()), Err(ErroPagamento::JaExiste(_))));
    assert_eq!(simulador.requests_de(Operacao::Iniciar).len(), 1);
}

#[test]
fn negado_termina_e_nao_vai_a_terceira_perna() {
    let simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::negado)]);
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    assert!(matches!(pagamentos.iniciar(pedido()).expect("inicia"), Passo::Negado { .. }));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Negado);
    assert!(pagamentos.registro(&id()).expect("r").motivo.as_deref().is_some_and(|m| m.contains("SIM-05")));
    assert!(pagamentos.confirmar(&id()).is_err());
    assert!(simulador.requests_de(Operacao::Confirmar).is_empty());
}

#[test]
fn timeout_que_nao_chegou_vira_cancelado_ao_recuperar() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::timeout));
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("novo");
    let erro = pagamentos.iniciar(pedido()).expect_err("tempo");
    assert_eq!(erro, ErroPagamento::Provedor(ErroProvedor::Tempo));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aguardando);
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::CanceladoSemEnvio);
}

#[test]
fn timeout_que_cobrou_e_achado_e_desfeito() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::timeout_depois_de_processar));
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    assert!(pagamentos.iniciar(pedido()).is_err());
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Pendente));
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Desfeito);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Desfeita));
}

#[test]
fn queda_antes_da_confirmacao_mantem_a_decisao_e_tenta_de_novo() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(confirmar(cenarios::queda_antes_da_confirmacao));
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    assert!(pagamentos.concluir(&id(), Impressao::Concluida).is_err());
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Pendente));
    drop(pagamentos);

    // The same decision is carried out, instead of undoing a sale that was finished.
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Confirmado);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Confirmada));
}

#[test]
fn confirmacao_com_resposta_perdida_nao_faz_estrago() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(confirmar(cenarios::queda_depois_da_confirmacao));
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    assert!(pagamentos.concluir(&id(), Impressao::Concluida).is_err());
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Confirmada));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Confirmado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Confirmado);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Confirmada));
}

#[test]
fn provedor_inalcancavel_na_recuperacao_fica_relatado_e_tenta_depois() {
    let simulador = SimuladorProvedor::new();
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    drop(pagamentos);
    simulador.use_once(desfazer(cenarios::timeout));
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("religa");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert!(matches!(relatorio[0].desfecho, Desfecho::Falhou(_)));
    assert_eq!(relatorio[0].depois, Some(Estado::Aprovado));
    assert_eq!(pagamentos.recuperar().expect("de novo")[0].desfecho, Desfecho::Desfeito);
}

#[test]
fn cancelamento_pelo_operador() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(continuar(cenarios::cancelado_pelo_operador));
    simulador.use_once(iniciar(cenarios::aguardando));
    let mut pagamentos = Pagamentos::new(simulador, DiarioMemoria::new()).expect("novo");
    assert!(matches!(pagamentos.iniciar(pedido()).expect("inicia"), Passo::Aguardar { .. }));
    let passo = pagamentos.continuar(&id(), EntradaOperador::Cancelar).expect("continua");
    assert!(matches!(passo, Passo::Cancelado { .. }));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Cancelado);
}

#[test]
fn cancelar_de_fora_e_pelo_provedor() {
    let simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::aguardando)]);
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    assert_eq!(pagamentos.cancelar(&id()).expect("cancela"), Cancelamento::Feito);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Cancelado);
    assert_eq!(simulador.requests_de(Operacao::Cancelar).len(), 1);
}

#[test]
fn perguntas_do_operador_atravessam_e_o_digitado_nao_vai_ao_diario() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::pergunta_parcelas));
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
    let passo = pagamentos.iniciar(pedido()).expect("inicia");
    assert!(matches!(passo, Passo::Perguntar(Pergunta::Campo { .. })));
    let passo = pagamentos.continuar(&id(), EntradaOperador::Texto("11122233344".into())).expect("responde");
    assert!(matches!(passo, Passo::Aprovado(_)));
    let requisicoes = simulador.requests_de(Operacao::Continuar);
    assert_eq!(requisicoes[0].entrada, Some(EntradaOperador::Texto("11122233344".into())));
    let tudo = format!("{:?}", diario.ler().expect("lê"));
    assert!(!tudo.contains("11122233344"));
    assert!(!format!("{:?}", requisicoes[0]).contains("11122233344"));
}

#[test]
fn menu_e_qr_sao_passos() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(continuar(cenarios::qr_pix));
    simulador.use_once(iniciar(cenarios::menu_debito_credito));
    let mut pagamentos = Pagamentos::new(simulador, DiarioMemoria::new()).expect("novo");
    assert!(matches!(pagamentos.iniciar(pedido()).expect("inicia"), Passo::Perguntar(Pergunta::Menu { .. })));
    let passo = pagamentos.continuar(&id(), EntradaOperador::Opcao(0)).expect("escolhe");
    assert!(matches!(passo, Passo::Perguntar(Pergunta::QrCode { .. })));
}

#[test]
fn pendente_de_sessao_anterior_e_desfeito() {
    let simulador = SimuladorProvedor::new();
    let antiga = IdPagamento::new("antiga").expect("id");
    simulador.semear_pendente_de_sessao_anterior(&antiga, 500);
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio.len(), 1);
    assert_eq!(relatorio[0].desfecho, Desfecho::PendenciaDesfeita);
    assert_eq!(relatorio[0].antes, None);
    assert_eq!(simulador.situacao(&antiga), Some(SituacaoRazao::Desfeita));
    assert!(pagamentos.recuperar().expect("de novo").is_empty());
}

#[test]
fn pendencia_de_venda_ja_confirmada_e_confirmada_de_novo() {
    let simulador = SimuladorProvedor::new();
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
    // The provider lost track of our confirmation and lists it as open.
    let referencia =
        pagamentos.registro(&id()).and_then(|r| r.aprovacao.clone()).expect("aprovação").referencia;
    simulador.use_once(pendencias(move |_| {
        let mut lista = referencia.clone();
        lista.id = Some(IdPagamento::new("venda-1").expect("id"));
        ravenrs_pagamento::simulador::Resposta::Pendencias(vec![lista])
    }));
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::PendenciaConfirmada);
    assert_eq!(simulador.requests_de(Operacao::Confirmar).len(), 2);
}

#[test]
fn sem_papel_avisa_e_desfaz_quando_ha_terceira_perna() {
    let simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::sem_papel)]);
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    let Passo::Aprovado(aprovacao) = pagamentos.iniciar(pedido()).expect("inicia") else { panic!("aprova") };
    assert_eq!(aprovacao.avisos, vec![AvisoAprovacao::SemPapel]);
    assert_eq!(pagamentos.concluir(&id(), Impressao::Falhou).expect("conclui"), Estado::Desfeito);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Desfeita));
}

#[test]
fn sem_papel_sem_terceira_perna_nao_finge_desfazer() {
    let simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::sem_papel)])
        .com_capacidades(sem_terceira_perna());
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    assert_eq!(pagamentos.concluir(&id(), Impressao::Falhou).expect("conclui"), Estado::Aprovado);
    assert!(matches!(pagamentos.desfazer(&id()), Err(ErroPagamento::Provedor(ErroProvedor::SemSuporte(_)))));
    assert_eq!(pagamentos.estornar(&id()).expect("estorna"), Estado::Estornado);
}

#[test]
fn c_aut_ausente_chega_como_alerta_e_nunca_como_valor() {
    let simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::c_aut_ausente)]);
    let mut pagamentos = Pagamentos::new(simulador, DiarioMemoria::new()).expect("novo");
    let Passo::Aprovado(aprovacao) = pagamentos.iniciar(pedido()).expect("inicia") else { panic!("aprova") };
    assert_eq!(aprovacao.dados_fiscais.c_aut, None);
    assert!(!aprovacao.dados_fiscais.atende_ya05_10());
    assert!(matches!(aprovacao.dados_fiscais.alertas[0], AlertaFiscal::CAutAusente { .. }));
}

#[test]
fn estorno_manual_nao_finge() {
    let capacidades = Capacidades { estorna_por_api: false, terceira_perna: false, ..CAPACIDADES_PADRAO };
    let simulador = SimuladorProvedor::new().com_capacidades(capacidades);
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
    assert_eq!(pagamentos.estornar(&id()), Err(ErroPagamento::EstornoManual));
    assert!(simulador.requests_de(Operacao::Estornar).is_empty());
    assert_eq!(pagamentos.registrar_estorno_externo(&id()).expect("registra"), Estado::Estornado);
}

#[test]
fn estorno_interrompido_pergunta_antes_de_repetir() {
    let simulador = SimuladorProvedor::new();
    let diario = DiarioMemoria::new();
    let mut pagamentos = Pagamentos::new(simulador.clone(), diario.clone()).expect("novo");
    pagamentos.iniciar(pedido()).expect("inicia");
    pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
    simulador.use_once(estornar(cenarios::queda_depois_da_confirmacao));
    assert!(pagamentos.estornar(&id()).is_err());
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Estornada));
    drop(pagamentos);

    let mut pagamentos = Pagamentos::new(simulador.clone(), diario).expect("religa");
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Reconciliado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Estornado);
    assert_eq!(simulador.requests_de(Operacao::Estornar).len(), 1, "não pode estornar duas vezes");
}

#[test]
fn situacao_indeterminada_nao_e_chutada() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::timeout));
    simulador.use_handler(consultar(|_| {
        ravenrs_pagamento::simulador::Resposta::Situacao(SituacaoProvedor::Indeterminada)
    }));
    let mut pagamentos = Pagamentos::new(simulador, DiarioMemoria::new()).expect("novo");
    assert!(pagamentos.iniciar(pedido()).is_err());
    let relatorio = pagamentos.recuperar().expect("recupera");
    assert_eq!(relatorio[0].desfecho, Desfecho::Indeterminado);
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aguardando);
    assert_eq!(pagamentos.abandonar(&id(), "conferido no terminal").expect("abandona"), Estado::Cancelado);
}

#[test]
fn consulta_traz_o_estado_sem_desfazer() {
    let simulador = SimuladorProvedor::new();
    simulador.use_once(iniciar(cenarios::timeout_depois_de_processar));
    let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
    assert!(pagamentos.iniciar(pedido()).is_err());
    assert!(matches!(pagamentos.consultar(&id()).expect("consulta"), SituacaoProvedor::Aprovada(_)));
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
    assert_eq!(pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui"), Estado::Confirmado);
}

#[test]
fn simulador_tem_use_once_reset_e_on_unhandled() {
    use ravenrs_pagamento::ProvedorPagamento;
    use ravenrs_pagamento::simulador::OnUnhandled;

    let mut simulador = SimuladorProvedor::com_handlers(vec![iniciar(cenarios::negado)]);
    simulador.use_once(iniciar(cenarios::aprovado));
    assert!(matches!(simulador.iniciar(&pedido()), Ok(Passo::Aprovado(_))));
    assert!(matches!(simulador.iniciar(&pedido()), Ok(Passo::Negado { .. })));
    simulador.use_handler(iniciar(cenarios::aprovado));
    assert!(matches!(simulador.iniciar(&pedido()), Ok(Passo::Aprovado(_))));
    simulador.reset_handlers();
    assert!(matches!(simulador.iniciar(&pedido()), Ok(Passo::Negado { .. })));
    simulador.on_unhandled(OnUnhandled::Erro);
    assert!(simulador.continuar(&id(), EntradaOperador::Nada).is_err());
    let sequencias: Vec<u64> = simulador.requests().iter().map(|r| r.sequencia).collect();
    assert_eq!(sequencias, [1, 2, 3, 4, 5]);
}

#[test]
fn simulador_e_deterministico() {
    let rodar = || {
        let simulador = SimuladorProvedor::new();
        let mut pagamentos = Pagamentos::new(simulador.clone(), DiarioMemoria::new()).expect("novo");
        let passo = pagamentos.iniciar(pedido()).expect("inicia");
        pagamentos.concluir(&id(), Impressao::Concluida).expect("conclui");
        (format!("{passo:?}"), format!("{:?}", simulador.requests()))
    };
    assert_eq!(rodar(), rodar());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn diario_em_arquivo_sobrevive_a_queda_real_do_programa() {
    use ravenrs_pagamento::DiarioArquivo;

    let pasta = std::env::temp_dir().join(format!("ravenrs-pagamento-queda-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&pasta);
    let caminho = pasta.join("diario.jsonl");
    let simulador = SimuladorProvedor::new();
    {
        let mut pagamentos =
            Pagamentos::new(simulador.clone(), DiarioArquivo::abrir(&caminho).expect("abre")).expect("novo");
        pagamentos.iniciar(pedido()).expect("inicia");
        // The program ends here, between approving and confirming.
    }
    let mut pagamentos =
        Pagamentos::new(simulador.clone(), DiarioArquivo::abrir(&caminho).expect("reabre")).expect("religa");
    assert_eq!(pagamentos.registro(&id()).expect("r").estado, Estado::Aprovado);
    assert_eq!(pagamentos.recuperar().expect("recupera")[0].desfecho, Desfecho::Desfeito);
    assert_eq!(simulador.situacao(&id()), Some(SituacaoRazao::Desfeita));
    let _ = std::fs::remove_dir_all(&pasta);
}
