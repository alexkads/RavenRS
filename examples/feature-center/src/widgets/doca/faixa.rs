//! `Ribbon`, every feature (#595): the menu of the app, quick access, pages of groups with large,
//! small and icon-only commands, toggles, a drop-down with a separator, a command turned off with
//! its tip, a dialog launcher, a contextual page that shows with a table selected, the search and
//! the page that is open. Every command writes what it did in a line.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "RibbonDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Faixa de opções";

/// The pages of the ribbon, in order; the contextual one is last, so the others keep their place.
pub const PAGES: [&str; 5] = ["Início", "Inserir", "Dados", "Exibir", "Layout"];

/// The page: the ribbon over a report, and lines with the open page, the search and the last command.
#[derive(Debug, Default)]
pub struct RibbonDemoPage;

impl View for RibbonDemoPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let said = cx.use_state(|| "Escolha um comando da faixa.".to_string());
        let page = cx.use_state(|| 0_usize);
        let search = cx.use_state(String::new);
        let bold = cx.use_state(|| false);
        let italic = cx.use_state(|| false);
        let ruler = cx.use_state(|| true);
        let grid = cx.use_state(|| false);
        let table = cx.use_state(|| false);
        let align = cx.use_state(|| 0_usize);
        let zoom = cx.use_state(|| 100_u32);

        let tell = |what: &'static str| {
            let said = said.clone();
            move || said.set(what.to_string())
        };
        let command = |label: &'static str, icon: IconName, what: &'static str| {
            RibbonItem::button(label, icon).on_click(tell(what))
        };
        let align_toggle = |i: usize, label: &'static str, icon: IconName| {
            let (read, write, line) = (align.clone(), align.clone(), said.clone());
            RibbonItem::toggle(
                label,
                icon,
                Binding::new(
                    move || read.get() == i,
                    move |_| {
                        write.set(i);
                        line.set(format!("Alinhamento: {label}"));
                    },
                ),
            )
            .icon_only()
        };
        let (zin, zout) = (zoom.clone(), zoom.clone());
        let (zin_line, zout_line) = (said.clone(), said.clone());

        let ribbon = Ribbon::new()
            .app_menu(
                "Arquivo",
                vec![
                    MenuItem::new("Novo relatório")
                        .icon(IconName::FilePlus)
                        .shortcut("⌘N")
                        .on_select(tell("Novo relatório")),
                    MenuItem::new("Abrir…")
                        .icon(IconName::FolderOpen)
                        .shortcut("⌘O")
                        .on_select(tell("Abrir…")),
                    MenuItem::new("Salvar").icon(IconName::Save).shortcut("⌘S").on_select(tell("Salvo")),
                    MenuItem::new("Exportar em PDF")
                        .icon(IconName::Download)
                        .on_select(tell("Exportado: fatura-1042.pdf")),
                    MenuItem::new("Imprimir")
                        .icon(IconName::Printer)
                        .shortcut("⌘P")
                        .on_select(tell("Imprimindo")),
                ],
            )
            .quick_access(command("Salvar", IconName::Save, "Salvo").tooltip("Salvar (⌘S)"))
            .quick_access(command("Desfazer", IconName::Undo2, "Desfeito"))
            .quick_access(
                RibbonItem::button("Refazer", IconName::Redo2).enabled(false).tooltip("Nada para refazer"),
            )
            .search(search.binding())
            .selected_page(page.binding())
            .page(
                RibbonPage::new(PAGES[0])
                    .group(
                        RibbonGroup::new("Área de transferência")
                            .item(
                                command("Colar", IconName::ClipboardPaste, "Colado")
                                    .menu_item(
                                        MenuItem::new("Colar")
                                            .icon(IconName::ClipboardPaste)
                                            .on_select(tell("Colado")),
                                    )
                                    .menu_separator()
                                    .menu_item(
                                        MenuItem::new("Colar só os valores")
                                            .icon(IconName::Type)
                                            .on_select(tell("Colados só os valores")),
                                    ),
                            )
                            .item(command("Recortar", IconName::Scissors, "Recortado").small())
                            .item(command("Copiar", IconName::Copy, "Copiado").small())
                            .item(
                                command("Pincel de formatação", IconName::Paintbrush, "Formato copiado")
                                    .small(),
                            ),
                    )
                    .group(
                        RibbonGroup::new("Fonte")
                            .item(RibbonItem::toggle("Negrito", IconName::Bold, bold.binding()).icon_only())
                            .item(
                                RibbonItem::toggle("Itálico", IconName::Italic, italic.binding()).icon_only(),
                            )
                            .item(command("Sublinhado", IconName::Underline, "Sublinhado").icon_only())
                            .dialog_launcher(tell("Abriria o diálogo de fonte")),
                    )
                    .group(
                        RibbonGroup::new("Alinhamento")
                            .item(align_toggle(0, "À esquerda", IconName::AlignLeft))
                            .item(align_toggle(1, "Centralizado", IconName::AlignCenter))
                            .item(align_toggle(2, "À direita", IconName::AlignRight)),
                    ),
            )
            .page(
                RibbonPage::new(PAGES[1]).group(
                    RibbonGroup::new("Controles")
                        .item(command("Rótulo", IconName::Type, "Rótulo inserido").large())
                        .item(command("Tabela", IconName::Table2, "Tabela inserida").large())
                        .item(command("Imagem", IconName::ImagePlus, "Imagem inserida").large())
                        .item(command("Gráfico", IconName::ChartColumn, "Gráfico inserido").large()),
                ),
            )
            .page(
                RibbonPage::new(PAGES[2])
                    .group(
                        RibbonGroup::new("Fonte de dados")
                            .item(command("Nova fonte de dados", IconName::Database, "Fonte de dados nova"))
                            .item(command("Consulta", IconName::Filter, "Consulta aberta")),
                    )
                    .group(
                        RibbonGroup::new("Cálculos")
                            .item(
                                command("Campo calculado", IconName::Calculator, "Campo calculado novo")
                                    .small(),
                            )
                            .item(command("Resumo", IconName::Sigma, "Resumo novo").small()),
                    ),
            )
            .page(
                RibbonPage::new(PAGES[3])
                    .group(
                        RibbonGroup::new("Mostrar")
                            .item(RibbonItem::toggle("Régua", IconName::Ruler, ruler.binding()).large())
                            .item(
                                RibbonItem::toggle("Linhas de grade", IconName::Grid2x2, grid.binding())
                                    .large(),
                            )
                            .item(
                                RibbonItem::toggle("Tabela selecionada", IconName::Table, table.binding())
                                    .large(),
                            ),
                    )
                    .group(
                        RibbonGroup::new("Zoom")
                            .item(RibbonItem::button("Aproximar", IconName::ZoomIn).on_click(move || {
                                zin.update(|z| *z = (*z + 10).min(400));
                                zin_line.set(format!("Zoom: {}%", zin.get()));
                            }))
                            .item(RibbonItem::button("Afastar", IconName::ZoomOut).on_click(move || {
                                zout.update(|z| *z = z.saturating_sub(10).max(10));
                                zout_line.set(format!("Zoom: {}%", zout.get()));
                            })),
                    ),
            )
            .page(
                RibbonPage::new(PAGES[4])
                    .contextual("Ferramentas de tabela", Tone::Warning, table.get())
                    .group(
                        RibbonGroup::new("Linhas e colunas")
                            .item(command("Inserir linha", IconName::Rows3, "Linha inserida"))
                            .item(command("Inserir coluna", IconName::Columns3, "Coluna inserida")),
                    ),
            );

        let open = PAGES.get(page.get()).copied().unwrap_or("?");
        let looking = search.get();
        let style = format!(
            "{}{}{}",
            if bold.get() { "negrito " } else { "" },
            if italic.get() { "itálico " } else { "" },
            if ruler.get() { "com régua" } else { "sem régua" },
        );
        let narrow = cx.viewport().width < 720.0;
        cx.child(
            &VStack::new((
                ribbon,
                VStack::new((
                    Text::new(format!("Página aberta: {open}")),
                    Text::new(if looking.is_empty() {
                        "Nada procurado".to_string()
                    } else {
                        format!("Procurando: {looking}")
                    }),
                    Text::new(format!("Fatura 1042 · {}% · {style}", zoom.get()))
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                    Text::new(said.get()),
                ))
                .spacing(8.0)
                .padding(if narrow { 8.0 } else { 16.0 })
                .align(Align::Start),
            ))
            .spacing(0.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
