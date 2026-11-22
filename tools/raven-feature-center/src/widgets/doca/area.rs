//! `DockArea`, every feature (#595): an editor whose documents stay in the center and whose tools
//! dock around it, a photo desk where anything goes anywhere, floats and hides at an edge, and a
//! fixed page of columns that only the app opens and closes. A line says what the last change was.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "DockAreaDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Doca";

/// The uses, in the order of the tabs.
pub const USES: [&str; 3] = ["Editor", "Livre", "Fixo"];

/// The document that has changes not saved, which the editor does not let close.
pub const UNSAVED: &str = "vendas.rs";

/// The editor as it opens: two documents, tools on three sides.
pub fn editor_start() -> DockLayout {
    DockLayout::new()
        .center(DockNode::tabs([UNSAVED, "leia-me.md"]))
        .left(DockNode::tabs(["arquivos", "busca"]), 220.0)
        .right(DockNode::tabs(["propriedades"]), 200.0)
        .bottom(DockNode::tabs(["saida", "terminal"]), 120.0)
}

fn editor_panels(unsaved: bool) -> Vec<DockPanel> {
    vec![
        DockPanel::new(UNSAVED, UNSAVED)
            .icon(IconName::FileText)
            .kind(DockPanelKind::Document)
            .unsaved(unsaved),
        DockPanel::new("leia-me.md", "leia-me.md")
            .icon(IconName::FileText)
            .kind(DockPanelKind::Document)
            .close_action(DockCloseAction::Remove),
        DockPanel::new("arquivos", "Arquivos").icon(IconName::Folder).closable(false),
        DockPanel::new("busca", "Busca").icon(IconName::Search),
        DockPanel::new("propriedades", "Propriedades").icon(IconName::Layers),
        DockPanel::new("saida", "Saída").icon(IconName::Rows3),
        DockPanel::new("terminal", "Terminal").icon(IconName::Terminal),
    ]
}

/// The photo desk: a photo in the center, a navigator floating and the history hidden at an edge.
pub fn desk_start() -> DockLayout {
    let mut layout = DockLayout::new()
        .center(DockNode::tabs(["foto"]))
        .left(DockNode::tabs(["pastas", "navegador"]), 200.0)
        .right(DockNode::vertical([DockNode::tabs(["camadas"]), DockNode::tabs(["historico"])]), 200.0)
        .top(DockNode::tabs(["filtros"]), 64.0);
    layout.float("navegador", Rect::new(260.0, 120.0, 220.0, 150.0));
    layout.auto_hide("historico");
    layout
}

fn desk_panels() -> Vec<DockPanel> {
    vec![
        DockPanel::new("foto", "Foto").icon(IconName::Camera).kind(DockPanelKind::Document).closable(false),
        DockPanel::new("pastas", "Pastas").icon(IconName::Folder),
        DockPanel::new("camadas", "Camadas").icon(IconName::Layers),
        DockPanel::new("historico", "Histórico").icon(IconName::History),
        DockPanel::new("filtros", "Filtros").icon(IconName::Filter),
        DockPanel::new("navegador", "Navegador").icon(IconName::Search),
    ]
}

/// The fixed page: an index, the text and notes, as columns with no tabs.
pub fn fixed_start() -> DockLayout {
    DockLayout::new()
        .center(DockNode::tabs(["texto"]))
        .left(DockNode::tabs(["indice"]), 200.0)
        .right(DockNode::tabs(["notas"]), 200.0)
}

fn fixed_panels() -> Vec<DockPanel> {
    vec![
        DockPanel::new("indice", "Índice").closable(false),
        DockPanel::new("texto", "Texto").closable(false),
        DockPanel::new("notas", "Notas").closable(false),
    ]
}

fn content(id: &str) -> AnyView {
    let (title, line) = match id {
        UNSAVED => ("vendas.rs", "fn total(itens: &[Item]) -> Valor { … }  ·  mudanças não salvas"),
        "leia-me.md" => ("leia-me.md", "# Vendas · fechar este documento o tira de vez"),
        "arquivos" => ("Arquivos", "src/ · vendas.rs · leia-me.md"),
        "busca" => ("Busca", "Nada procurado ainda"),
        "propriedades" => ("Propriedades", "Linhas: 42 · UTF-8"),
        "saida" => ("Saída", "Compilado em 1,2 s"),
        "terminal" => ("Terminal", "$ cargo run"),
        "foto" => ("Foto", "praia.jpg · 4032 × 3024"),
        "pastas" => ("Pastas", "Viagens · Família · Trabalho"),
        "camadas" => ("Camadas", "Fundo · Ajuste de cor"),
        "historico" => ("Histórico", "Abrir · Recortar · Clarear"),
        "filtros" => ("Filtros", "Brilho · Contraste · Saturação"),
        "navegador" => ("Navegador", "100% · arraste para ver outra parte"),
        "indice" => ("Índice", "1. Começo · 2. Meio · 3. Fim"),
        "texto" => ("Texto", "Era uma vez uma doca que não mudava de lugar."),
        "notas" => ("Notas", "Rever o capítulo 2"),
        other => (other, ""),
    };
    Box::new(
        VStack::new((
            Text::new(title.to_owned()).style(TextStyle::Headline),
            Text::new(line.to_owned()).foreground(Semantic::InkMuted),
        ))
        .spacing(6.0)
        .padding(12.0)
        .align(Align::Start),
    )
}

fn count(layout: &DockLayout) -> String {
    let open = layout.panels().len();
    let closed = layout.closed_panels().len();
    format!("{open} abertos, {closed} fechados")
}

/// The page: a tab per use, and a line with the last change.
#[derive(Debug, Default)]
pub struct DockAreaPage;

impl View for DockAreaPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let said = cx.use_state(|| "Arraste uma aba, feche um painel ou mexa numa barra.".to_string());
        let narrow = cx.viewport().width < 720.0;
        // On a phone the side docks start collapsed, so the center has room; their strips open them.
        let roomy = |mut layout: DockLayout| {
            if narrow {
                layout.set_open(DockSide::Left, false);
                layout.set_open(DockSide::Right, false);
            }
            layout
        };
        let editor = cx.use_state(|| roomy(editor_start()));
        let desk = cx.use_state(|| roomy(desk_start()));
        let fixed = cx.use_state(fixed_start);
        let unsaved = cx.use_state(|| true);
        let kept = cx.use_state(|| None::<String>);
        let height = if narrow { 380.0 } else { 480.0 };

        let body: AnyView = match tab.get() {
            0 => {
                let (guard_unsaved, ended, refused) = (unsaved.clone(), said.clone(), said.clone());
                let (save, saving) = (unsaved.clone(), said.clone());
                let (keep, keeping, kept_layout) = (kept.clone(), said.clone(), editor.clone());
                let (back, backing, back_layout) = (kept.clone(), said.clone(), editor.clone());
                let again = editor.clone();
                let area = DockArea::new(editor.binding(), editor_panels(unsaved.get()), content)
                    .height(height)
                    .label("Editor de código")
                    .dock_limits(DockSide::Left, 160.0, 360.0)
                    .dock_limits(DockSide::Bottom, 80.0, 260.0)
                    .center_min(if narrow { 120.0 } else { 280.0 }, 140.0)
                    .pane_min(80.0)
                    .on_change(move |_, _, change| match change {
                        DockChange::Close { panel } if panel == UNSAVED && guard_unsaved.get() => {
                            let why = format!("{UNSAVED} tem mudanças não salvas: salve antes de fechar");
                            refused.set(format!("Recusado: {why}"));
                            Err(why)
                        }
                        _ => Ok(()),
                    })
                    .on_end(move |now| ended.set(format!("Arranjo salvo: {}", count(now))))
                    .on_documents_empty(move || {
                        let again = again.clone();
                        Box::new(
                            VStack::new((
                                Text::new("Nenhum documento aberto").style(TextStyle::Headline),
                                Button::new("Abrir os documentos de novo")
                                    .on_click(move || again.set(editor_start())),
                            ))
                            .spacing(8.0)
                            .padding(16.0),
                        )
                    });
                let actions = HStack::new((
                    Button::new(format!("Salvar {UNSAVED}")).on_click(move || {
                        save.set(false);
                        saving.set(format!("{UNSAVED} salvo: agora ele fecha"));
                    }),
                    Button::new("Guardar o arranjo").style(ButtonStyle::Secondary).on_click(move || {
                        let text = kept_layout.get().to_text();
                        keeping.set(format!("Arranjo guardado ({} caracteres)", text.len()));
                        keep.set(Some(text));
                    }),
                    Button::new("Restaurar o guardado").style(ButtonStyle::Secondary).on_click(move || {
                        match back.get() {
                            Some(text) => match DockLayout::parse(&text) {
                                Ok(layout) => {
                                    back_layout.set(layout);
                                    backing.set("Arranjo restaurado".to_string());
                                }
                                Err(why) => backing.set(format!("O texto guardado não serve: {why}")),
                            },
                            None => backing.set("Guarde um arranjo antes".to_string()),
                        }
                    }),
                ))
                .spacing(8.0)
                .wrap();
                Box::new(VStack::new((actions, area)).spacing(8.0).align(Align::Stretch))
            }
            1 => Box::new(
                DockArea::new(desk.binding(), desk_panels(), content)
                    .height(height)
                    .label("Mesa de fotos")
                    .free_placement(true)
                    .tab_bar(DockTabBar::WhenSeveral)
                    .floatable(true)
                    .auto_hide(true)
                    .on_end({
                        let said = said.clone();
                        move |now| {
                            let floating = now.floating_panels().len();
                            let hidden = now.auto_hidden_panels(DockSide::Right).len();
                            said.set(format!(
                                "{} · {floating} flutuando, {hidden} oculto à direita",
                                count(now)
                            ))
                        }
                    }),
            ),
            _ => {
                let toggle = |side: DockSide, name: &'static str| {
                    let (layout, line) = (fixed.clone(), said.clone());
                    Button::new(name).style(ButtonStyle::Secondary).on_click(move || {
                        layout.update(|l| {
                            let open = !l.is_open(side);
                            l.set_open(side, open);
                            line.set(format!("{name}: {}", if open { "aberto" } else { "fechado" }));
                        })
                    })
                };
                let area = DockArea::new(fixed.binding(), fixed_panels(), content)
                    .height(height)
                    .label("Página fixa")
                    .tab_bar(DockTabBar::Never)
                    .movable(false)
                    .floatable(false)
                    .auto_hide(false)
                    .edge_toggles(false);
                Box::new(
                    VStack::new((
                        HStack::new((toggle(DockSide::Left, "Índice"), toggle(DockSide::Right, "Notas")))
                            .spacing(8.0),
                        area,
                    ))
                    .spacing(8.0)
                    .align(Align::Stretch),
                )
            }
        };
        let picker: AnyView = if narrow {
            Box::new(Select::new(tab.binding(), USES).label("Uso"))
        } else {
            Box::new(SegmentedControl::new(tab.binding(), USES))
        };
        cx.child(
            &VStack::new((picker, Keyed::new(tab.get() as u64, body), Text::new(said.get())))
                .spacing(12.0)
                .padding(if narrow { 0.0 } else { 16.0 })
                .align(Align::Stretch),
        )
    }
}
