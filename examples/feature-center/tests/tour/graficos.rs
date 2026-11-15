//! The steps of the family `ravenrs-widgets-charts`: the pages Gráficos, Diagrama de Sankey,
//! Código de barras and Painéis do fluxo.

use super::*;

#[test]
fn the_charts_show_every_kind_and_the_card_follows_its_tabs_and_its_period() {
    use feature_center::widgets::graficos::tipos::{CURVES, STACKS, USES};
    let mut c = Center::memory("ana");
    c.press("Gráficos");
    // Each kind of chart is named by its title, and a screen reader hears its values.
    for (tab, title) in USES.iter().zip([
        "Vendas por mês",
        "Receita acumulada",
        "Vendas por canal",
        "Avaliação da equipe",
        "Cotação da semana",
        "Loja central",
    ]) {
        c.activate(tab);
        assert!(c.says(title), "{tab}: {:?}", c.texts());
    }
    c.activate(USES[0]);
    assert!(c.says("Produtos mais vendidos") && c.says("Estoque por depósito"));
    assert!(c.says("Pão de queijo"), "the categories are read: {:?}", c.texts());
    for stack in STACKS {
        c.activate(stack);
        assert!(c.says("Vendas por mês"), "{stack} keeps the chart");
    }
    c.press("Destacar o próximo mês");
    // A period swaps the areas' data, and the line under the switch says which.
    c.activate(USES[1]);
    for curve in CURVES {
        c.activate(curve);
        assert!(c.says("Visitas"), "{curve}");
    }
    assert!(c.says("Período: 1º semestre"));
    c.press("Trocar o semestre");
    assert!(c.says("Período: 2º semestre"), "{:?}", c.texts());
    c.activate(USES[3]);
    assert!(c.says("Metas do trimestre") && c.says("Só os contornos"));
    c.activate(USES[4]);
    assert!(c.says("Entrega por distância"));
    // The card: its tabs choose what it shows, and its action changes the period.
    c.activate(USES[5]);
    assert!(c.says("O cartão mostra: Receita"));
    assert!(c.says("Subiu 12% no mês") && c.says("Valores em milhares de reais"), "trend and note");
    c.activate("Pedidos: 1.240");
    assert!(c.says("O cartão mostra: Pedidos"), "{:?}", c.texts());
    c.press("Trocar período");
    assert!(c.says("Período: 1º semestre"), "back from the second half: {:?}", c.texts());
    assert!(c.says("Visitas da semana, subindo"), "a sparkline's alt");
}

#[test]
fn the_sankey_follows_the_money_in_each_column_and_month() {
    use feature_center::widgets::graficos::sankey::{ALIGNS, MONTHS};
    let mut c = Center::memory("ana");
    c.press("Diagrama de Sankey");
    assert!(c.says("Para onde foi o dinheiro em Outubro"), "{:?}", c.texts());
    assert!(c.says("R$ 2400"), "the flows are read in its format: {:?}", c.texts());
    for align in ALIGNS {
        c.activate(align);
        assert!(c.says(&format!("Colunas: {align}")));
    }
    c.activate(MONTHS[1]);
    assert!(c.says("Para onde foi o dinheiro em Novembro"));
    assert!(c.says("R$ 1600"), "November saved more: {:?}", c.texts());
    c.activate("Fitas em degradê");
    assert!(c.says("Energia: da usina às casas e às fábricas"), "the quiet one is named by its alt");
}

#[test]
fn the_barcodes_show_every_format_and_the_qr_code_holds_what_is_typed() {
    use feature_center::widgets::graficos::barras::FORMATS;
    let mut c = Center::memory("ana");
    c.press("Código de barras");
    for (name, _, _) in FORMATS {
        assert!(c.says(name), "{name}: {:?}", c.texts());
    }
    assert!(c.says("QR Code do Pix da loja"), "the alt replaces the data");
    c.activate("Texto sob as barras");
    c.activate("Maiores");
    assert!(c.says("EAN-13"));
    c.type_into("Endereço do QR Code", "/loja");
    let held = c.texts().into_iter().find(|t| t.starts_with("O QR Code guarda:")).expect("the line");
    assert!(held.contains("/loja") && held.contains("ravenrs.dev"), "{held}");
}

#[test]
fn the_flow_panels_drop_a_model_pick_a_layer_change_it_hide_it_and_keep_the_diagram() {
    let mut c = Center::memory("ana");
    c.press("Painéis do fluxo");
    assert!(c.says("Peças do pedido") && c.says("Fluxos do pedido"), "{:?}", c.texts());
    assert!(c.says("Nós no diagrama: 3"));
    assert!(c.says("Selecione um nó ou uma ligação"), "nothing picked: {:?}", c.texts());
    // A group of models opens, and a model, with Enter, lands in the middle of the canvas.
    assert!(!c.has_name("Embalagem"), "only the first group starts open");
    c.activate("Atalhos");
    let tile =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Embalagem"))
            .map(|n| n.id)
            .unwrap_or_else(|| panic!("the model Embalagem: {:?}", c.texts()));
    c.ui.perform_action(tile, AccessAction::Focus);
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Nós no diagrama: 4"), "{:?}", c.texts());
    // The layers hide a node and show it again.
    c.press("Ocultar Conferir estoque");
    assert!(c.says("Ocultos: 1"), "{:?}", c.texts());
    c.press("Mostrar Conferir estoque");
    assert!(c.says("Ocultos: 0"));
    c.press("Trazer Entregar para frente");
    // The node that arrived is the one selected, and the inspector changes its title.
    assert!(c.says("Selecionados: 1"), "{:?}", c.texts());
    assert!(!c.says("Selecione um nó ou uma ligação"), "a node picked");
    c.type_into("Título", " hoje");
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Propriedades:"), "the inspector asks on_change: {:?}", c.texts());
    assert!(c.has_name("Ocultar Embalar hoje"), "the layers follow the new title: {:?}", c.texts());
    // Picked in the layers, which list it after the canvas, another node shows there.
    let row =
        c.ui.access_tree()
            .nodes
            .iter()
            .rev()
            .find(|n| n.name.as_deref() == Some("Pedido chegou"))
            .map(|n| n.id);
    c.ui.perform_action(row.expect("the layer"), AccessAction::Click);
    c.settle();
    let title =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some("Título"))
            .and_then(|n| n.value.clone());
    assert_eq!(title.as_deref(), Some("Pedido chegou"), "the inspector shows the node picked");
    c.activate("Painéis só de leitura");
    assert!(c.says("Edição travada: só leitura"), "{:?}", c.texts());
    c.activate("Painéis só de leitura");
    // The library keeps it, and a new one starts empty.
    c.press("Salvar");
    assert!(c.says("Salvo"), "{:?}", c.texts());
    c.press("Novo");
    assert!(c.says("Nós no diagrama: 0"), "{:?}", c.texts());
    c.activate("Busca nas peças");
    assert!(c.says("Peças do pedido"));
}
