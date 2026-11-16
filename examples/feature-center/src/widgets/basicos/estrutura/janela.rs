//! `TitleBar`, `Menubar`, `ToolRail` and `StatusBar`, every feature: the bar of a window with its
//! buttons, active and quiet, the menus of an app, a rail of tools with variants in one or two
//! columns, and the line at the bottom with a notice in the middle. A line says what was done.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "WindowDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Janela e barras";

/// The tools of the rail, in the order `selected` counts them: the variants of a place included.
pub const TOOLS: [&str; 5] = ["Selecionar", "Texto", "Retângulo", "Elipse", "Régua"];

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The page: title bars, the menubar, the rails and the status bar.
#[derive(Debug, Default)]
pub struct WindowPage;

impl View for WindowPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let maximized = cx.use_state(|| false);
        let tool = cx.use_state(|| 0_usize);
        let compact_tool = cx.use_state(|| 0_usize);
        let saved = cx.use_state(|| true);
        let event = cx.use_state(|| String::from("Nada feito ainda"));

        // The bar of a window: an icon before the title, an action, and the three buttons.
        let said_min = event.clone();
        let (toggle, said_max) = (maximized.clone(), event.clone());
        let said_close = event.clone();
        let said_menu = event.clone();
        let said_share = event.clone();
        let big = maximized.get();
        let title = TitleBar::new("Relatório de vendas")
            .leading(Icon::new(IconName::FileText).color(Semantic::Accent))
            .action(
                Button::new("Compartilhar")
                    .style(ButtonStyle::Ghost)
                    .size(ButtonSize::Small)
                    .on_click(move || said_share.set(String::from("Compartilhar"))),
            )
            .on_minimize(move || said_min.set(String::from("Janela minimizada")))
            .on_maximize(move || {
                toggle.update(|on| *on = !*on);
                said_max.set(String::from(if big { "Janela restaurada" } else { "Janela maximizada" }));
            })
            .on_close(move || said_close.set(String::from("Janela fechada")))
            .on_context_menu(move |at| said_menu.set(format!("Menu da janela em {:.0}, {:.0}", at.x, at.y)))
            .maximized(big);
        let quiet = TitleBar::new("Outra janela, sem o teclado").on_close(|| {}).active(false);

        // The menus of an app; each item says what it ran.
        let item = |label: &str, shortcut: &str| {
            let said = event.clone();
            let text = label.to_owned();
            MenuItem::new(label.to_owned())
                .shortcut(shortcut.to_owned())
                .on_select(move || said.set(format!("Menu: {text}")))
        };
        let menubar = Menubar::new()
            .menu("Arquivo")
            .item(item("Novo", "⌘N"))
            .item(item("Abrir…", "⌘O"))
            .separator()
            .item(item("Salvar", "⌘S"))
            .menu("Editar")
            .item(item("Desfazer", "⌘Z"))
            .item(item("Refazer", "⇧⌘Z"))
            .menu("Exibir")
            .item(item("Tela cheia", "⌃⌘F"));

        // A rail of tools: one with variants and a key, a line between groups, a drag out of it.
        let said_drag = event.clone();
        let rail =
            ToolRail::new(tool.binding(), [(IconName::MousePointer, TOOLS[0]), (IconName::Type, TOOLS[1])])
                .separator()
                .tool(
                    ToolRailItem::new(IconName::Square, TOOLS[2])
                        .shortcut("R")
                        .variant(IconName::Circle, TOOLS[3]),
                )
                .tool(ToolRailItem::new(IconName::Ruler, TOOLS[4]).shortcut("U"))
                .width(88.0)
                .on_drag_tool(move |index, drag| {
                    if drag.phase == DragPhase::End {
                        said_drag.set(format!(
                            "Ferramenta arrastada: {}",
                            TOOLS.get(index).copied().unwrap_or("?")
                        ));
                    }
                });
        let icons = ToolRail::new(
            compact_tool.binding(),
            [
                (IconName::MousePointer, "Mover"),
                (IconName::Pencil, "Lápis"),
                (IconName::Paintbrush, "Pincel"),
                (IconName::Highlighter, "Marca-texto"),
            ],
        )
        .icons_only(true);
        let two = ToolRail::new(
            compact_tool.binding(),
            [
                (IconName::MousePointer, "Mover"),
                (IconName::Pencil, "Lápis"),
                (IconName::Paintbrush, "Pincel"),
                (IconName::Highlighter, "Marca-texto"),
            ],
        )
        .columns(2);
        let rails = HStack::new((
            rail,
            icons,
            two,
            VStack::new((
                Text::new(format!("Ferramenta: {}", TOOLS.get(tool.get()).copied().unwrap_or("?"))),
                Text::new(format!("Na compacta: {}", compact_tool.get() + 1)).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
        ))
        .spacing(if narrow { 8.0 } else { 24.0 })
        .align(Align::Start)
        .height(260.0);

        // The line at the bottom: what is open, a notice in the middle and an action at the end.
        let (save, said_save) = (saved.clone(), event.clone());
        let clean = saved.get();
        let status = StatusBar::new()
            .left((Icon::new(IconName::GitBranch).size(12.0), StatusBar::text("master")))
            .center((StatusBar::text(if clean { "Tudo salvo" } else { "Há mudanças não salvas" })
                .truncate()
                .grow(1.0),))
            .right((
                StatusBar::text("Ln 12, Col 4"),
                StatusBar::action(if clean { "Marcar como alterado" } else { "Salvar agora" }).on_click(
                    move || {
                        save.update(|on| *on = !*on);
                        said_save.set(String::from(if clean { "Arquivo alterado" } else { "Arquivo salvo" }));
                    },
                ),
            ))
            .center_tone(if clean { Tone::Success } else { Tone::Warning });

        cx.child(
            &VStack::new((
                VStack::new((heading("Barra de título"), title, quiet)).spacing(8.0),
                VStack::new((heading("Menus"), menubar)).spacing(8.0),
                VStack::new((heading("Trilho de ferramentas"), rails)).spacing(8.0),
                VStack::new((heading("Barra de status"), status)).spacing(8.0),
                Text::bound(&event, |said| format!("Evento: {said}")),
            ))
            .spacing(20.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
