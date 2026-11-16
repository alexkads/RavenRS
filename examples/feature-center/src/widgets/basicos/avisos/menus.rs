//! `DropdownMenu` and `ContextMenu`, every feature: menus behind a button in each style and size,
//! one with only its icon for a tight bar, one opened from another control, items with icons, keys,
//! check marks, a submenu, a disabled one and a destructive one, and the menu a right click opens
//! over a row. A line says what was chosen.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "MenusDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Menus";

/// The row the context menu is over.
pub const ROW: &str = "Fatura 1042 · Ana Souza · R$ 1.280,00";

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// An item that writes `what` in the line when it is chosen.
fn item(line: &State<String>, label: &str, what: &str) -> MenuItem {
    let (line, what) = (line.clone(), what.to_owned());
    MenuItem::new(label).on_select(move || line.set(what.clone()))
}

/// The page: dropdown menus side by side, one opened from outside, and a context menu.
#[derive(Debug, Default)]
pub struct MenusPage;

impl View for MenusPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha num menu.".to_string());
        let grid = cx.use_state(|| true);
        let file_open = cx.use_state(|| false);

        let flip = (grid.clone(), said.clone());
        let file = DropdownMenu::new("Arquivo")
            .icon(IconName::File)
            .open(file_open.binding())
            .item(item(&said, "Novo", "Arquivo › Novo").icon(IconName::Plus).shortcut("⌘N"))
            .item(item(&said, "Abrir…", "Arquivo › Abrir").icon(IconName::FolderOpen).shortcut("⌘O"))
            .separator()
            .item(
                MenuItem::new("Exportar")
                    .icon(IconName::Download)
                    .item(item(&said, "Como PDF", "Exportar › PDF"))
                    .separator()
                    .item(item(&said, "Como planilha", "Exportar › planilha")),
            )
            .item(MenuItem::new("Enviar por e-mail").disabled("Sem conta de e-mail"))
            .separator()
            .item(item(&said, "Excluir o arquivo", "Arquivo excluído").destructive().icon(IconName::Trash2));
        let view = DropdownMenu::new("Exibir")
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .item(MenuItem::new("Mostrar a grade").checked(grid.get()).on_select(move || {
                flip.0.update(|on| *on = !*on);
                flip.1.set(format!("Grade: {}", if flip.0.get() { "à mostra" } else { "escondida" }));
            }))
            .item(item(&said, "Tema", "Tema escolhido").element(
                HStack::new((Badge::new("Escuro").tone(Tone::Accent), Text::new("Tema"))).spacing(6.0),
            ));
        let more = DropdownMenu::new("Mais ações")
            .icon(IconName::Ellipsis)
            .icon_only()
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .item(item(&said, "Duplicar", "Duplicado"))
            .item(item(&said, "Arquivar", "Arquivado"));
        let primary = DropdownMenu::new("Novo lançamento")
            .style(ButtonStyle::Primary)
            .size(if narrow { ButtonSize::Medium } else { ButtonSize::Large })
            .item(item(&said, "Receita", "Nova receita"))
            .item(item(&said, "Despesa", "Nova despesa"));
        let opens = file_open.clone();
        let from_outside = Button::new("Abrir o menu Arquivo daqui")
            .style(ButtonStyle::Secondary)
            .on_click(move || opens.set(true));

        let row = ContextMenu::new(
            HStack::new((Avatar::new("Ana Souza").size(28.0), Text::new(ROW)))
                .spacing(8.0)
                .align(Align::Center)
                .padding(8.0),
        )
        .item(item(&said, "Copiar o número", "Número copiado").icon(IconName::Copy))
        .item(item(&said, "Marcar como paga", "Fatura marcada como paga").icon(IconName::CircleCheck))
        .separator()
        .item(item(&said, "Excluir a fatura", "Fatura excluída").destructive());

        let status =
            format!("{} · menu Arquivo {}", said.get(), if file_open.get() { "aberto" } else { "fechado" });

        cx.child(
            &VStack::new((
                VStack::new((
                    heading("Menus de botão"),
                    HStack::new((file, view, more, primary)).spacing(8.0).wrap().align(Align::Center),
                    from_outside,
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    heading("Menu de contexto"),
                    Text::new("Clique com o botão direito na linha.").foreground(Semantic::InkMuted),
                    row,
                ))
                .spacing(8.0)
                .align(Align::Stretch),
                Text::new(status),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
