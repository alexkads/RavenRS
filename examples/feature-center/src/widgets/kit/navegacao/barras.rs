//! `Toolbar`, `FloatingToolbar`, `SplitButton`, `KeyBar`, `ShortcutHintBar` and `ShortcutSheet`,
//! every feature (#579): the commands of an order, a zoom bar over a photo that can be dragged
//! away, a save button with alternatives, the function keys of a counter, the hints under a
//! list and the sheet of every key. Each command writes what it did in a line.

use ravenrs::core::Key;
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "ToolbarsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Barras de ferramentas";

/// Where the zoom bar stays, down the photo.
pub const PLACES: [&str; 3] = ["Em cima", "No meio", "Embaixo"];
/// The sizes of the save button, in the order of `ButtonSize`.
pub const SIZES: [&str; 3] = ["Pequeno", "Médio", "Grande"];

/// A heading and what goes under it, closer together than the sections are.
fn section<C: ViewSeq>(children: C) -> VStack<C> {
    VStack::new(children).spacing(8.0).align(Align::Stretch)
}

/// The page: the bars, the sheet of keys and a line with the last command.
#[derive(Debug, Default)]
pub struct ToolbarsPage;

impl View for ToolbarsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Use uma barra ou uma tecla de função.".to_string());
        let orders = cx.use_state(|| 3_u32);
        let busy = cx.use_state(|| false);
        let zoom = cx.use_state(|| 100_u32);
        let down = cx.use_state(|| 2_usize);
        let moved = cx.use_state(|| None::<Point>);
        let size = cx.use_state(|| 1_usize);
        let sheet = cx.use_state(|| false);

        let say = |text: &'static str| {
            let line = said.clone();
            move || line.set(text.to_string())
        };
        let added = (orders.clone(), said.clone());
        let commands = Toolbar::new()
            .label("Ações do pedido")
            .action(ToolbarAction::new("Novo").icon(IconName::Plus).hotkey(Hotkey::alt("n")).on_click(
                move || {
                    added.0.set(added.0.get() + 1);
                    added.1.set(format!("Novo pedido: agora são {}", added.0.get()));
                },
            ))
            .action(
                ToolbarAction::new("Copiar")
                    .icon(IconName::Copy)
                    .icon_only()
                    .accessible_name("Copiar o pedido")
                    .accessible_description("Põe uma cópia na área de transferência")
                    .on_click(say("Pedido copiado")),
            )
            .action(ToolbarAction::new("Imprimir").icon(IconName::Printer).on_click(say("Pedido impresso")))
            .separator()
            .action(
                ToolbarAction::new("Excluir")
                    .icon(IconName::Trash2)
                    .style(ButtonStyle::Danger)
                    .disabled(busy.get())
                    .on_click(say("Pedido excluído")),
            )
            .view(110.0, Badge::new(format!("{} abertos", orders.get())));
        let ending = Toolbar::new()
            .label("Exibição")
            .aligned_end()
            .action(ToolbarAction::new("Filtrar").icon(IconName::Search).on_click(say("Filtro aberto")))
            .action(ToolbarAction::new("Exportar").icon(IconName::Download).on_click(say("Lista exportada")));

        let (closer, further) = (zoom.clone(), zoom.clone());
        let fit = zoom.clone();
        let photo = VStack::new((
            Icon::new(IconName::Image).size(48.0),
            Text::new(format!("Vitrine da loja · {} %", zoom.get())),
        ))
        .spacing(8.0)
        .align(Align::Center);
        let over = FloatingToolbar::new(photo)
            .label("Zoom")
            .action(ToolbarAction::new("Ajustar").on_click(move || fit.set(100)))
            .separator()
            .action(
                ToolbarAction::new("Diminuir")
                    .icon(IconName::ZoomOut)
                    .icon_only()
                    .on_click(move || closer.set(closer.get().saturating_sub(25).max(25))),
            )
            .action(
                ToolbarAction::new("Aumentar")
                    .icon(IconName::ZoomIn)
                    .icon_only()
                    .on_click(move || further.set((further.get() + 25).min(400))),
            )
            .view(Text::new(format!("{} %", zoom.get())))
            .place(
                Align::Center,
                match down.get() {
                    0 => Align::Start,
                    1 => Align::Center,
                    _ => Align::End,
                },
            )
            .margin(16.0)
            .movable(moved.binding())
            .fill();
        let stage = VStack::new((over,)).height(if narrow { 200.0 } else { 240.0 }).align(Align::Stretch);

        let saved = said.clone();
        let save = SplitButton::new("Salvar")
            .icon(IconName::Save)
            .style(ButtonStyle::Primary)
            .size(match size.get() {
                0 => ButtonSize::Small,
                2 => ButtonSize::Large,
                _ => ButtonSize::Medium,
            })
            .disabled(busy.get())
            .on_click(move || saved.set("Salvo".to_string()))
            .item(MenuItem::new("Salvar e fechar").on_select(say("Salvo e fechado")))
            .item(
                MenuItem::new("Salvar como modelo")
                    .icon(IconName::Bookmark)
                    .on_select(say("Salvo como modelo")),
            );
        let send = SplitButton::new("Enviar")
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(say("Enviado por e-mail"))
            .item(MenuItem::new("Enviar por mensagem").on_select(say("Enviado por mensagem")));

        let keys = KeyBar::new()
            .item(
                KeyBarItem::new(Hotkey::key(Key::Function(2)), "Nova venda", say("F2: nova venda")).primary(),
            )
            .item(KeyBarItem::new(Hotkey::key(Key::Function(4)), "Desconto", say("F4: desconto")))
            .item(
                KeyBarItem::new(Hotkey::key(Key::Function(8)), "Cancelar", say("F8: cancelar"))
                    .disabled(busy.get()),
            );

        let hints = ShortcutHintBar::new()
            .leading(format!("Pedido 3 de {}", orders.get()))
            .hint(["←", "→"], "trocar")
            .hint([Hotkey::alt("n").label()], "novo")
            .hint(["F2"], "venda");
        let opener = sheet.clone();
        let keys_sheet = ShortcutSheet::new(sheet.binding())
            .title("Atalhos do pedido")
            .width(if narrow { 320.0 } else { 560.0 })
            .group("Pedido", [(Hotkey::alt("n").label(), "Novo pedido".to_string())])
            .group("Caixa", [("F2", "Nova venda"), ("F4", "Desconto"), ("F8", "Cancelar")]);

        let place: AnyView = if narrow {
            Box::new(Select::new(down.binding(), PLACES).label("Barra de zoom"))
        } else {
            Box::new(SegmentedControl::new(down.binding(), PLACES))
        };
        let sizes: AnyView = if narrow {
            Box::new(Select::new(size.binding(), SIZES).label("Tamanho"))
        } else {
            Box::new(SegmentedControl::new(size.binding(), SIZES))
        };
        let where_it_is = match moved.get() {
            Some(p) => format!("Barra de zoom solta em ({:.0}, {:.0})", p.x, p.y),
            None => "Barra de zoom no lugar de origem".to_string(),
        };

        cx.child(
            &VStack::new((
                section((
                    Text::new("Comandos").style(TextStyle::Headline),
                    commands,
                    ending,
                    Checkbox::new("Pedido em processamento", busy.binding()),
                )),
                section((
                    Text::new("Sobre a foto").style(TextStyle::Headline),
                    place,
                    stage,
                    Text::new(where_it_is),
                )),
                section((
                    Text::new("Botão com alternativas").style(TextStyle::Headline),
                    sizes,
                    HStack::new((save, send)).spacing(12.0).align(Align::Center),
                )),
                section((
                    Text::new("Teclas").style(TextStyle::Headline),
                    keys,
                    hints,
                    Button::new("Ver atalhos").icon(IconName::Keyboard).on_click(move || opener.set(true)),
                )),
                Text::new(said.get()),
                keys_sheet,
            ))
            .spacing(12.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
