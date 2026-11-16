//! `Dialog`, `Sheet`, `Popover`, `HoverCard`, `Tooltip` and `HelpTip`, every feature: a dialog that
//! asks for a name and one that must not be dismissed by accident, panels that slide from either
//! edge, popovers on and off the panel, a card under a name, bubbles with a key and the "?" that
//! explains a field. A line says what was done.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "OverlaysDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Camadas por cima";

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// Sets `open` to true, for the buttons that open a layer.
fn opener(open: &State<bool>) -> impl Fn() + 'static {
    let open = open.clone();
    move || open.set(true)
}

/// The page: dialogs, sheets, popovers, a hover card, tooltips and help tips.
#[derive(Debug, Default)]
pub struct OverlaysPage;

impl View for OverlaysPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Abra uma camada.".to_string());
        let rename = cx.use_state(|| false);
        let discard = cx.use_state(|| false);
        let name = cx.use_state(|| "Raven Finanças".to_string());
        let filters = cx.use_state(|| false);
        let details = cx.use_state(|| false);
        let menu = cx.use_state(|| false);
        let note = cx.use_state(|| false);
        let late_only = cx.use_state(|| true);
        let rate = cx.use_state(|| "5".to_string());

        // A dialog that asks, and one that only its own buttons close.
        let (save, saved, kept_name) = (rename.clone(), said.clone(), name.clone());
        let (cancel, cancelled) = (rename.clone(), said.clone());
        let rename_dialog = Dialog::new(rename.binding(), "Renomear o projeto")
            .description("O nome aparece no topo de cada relatório.")
            .content(FormField::new(
                "Nome do projeto",
                TextField::new(name.binding()).placeholder("Como o projeto se chama"),
            ))
            .width(if narrow { 320.0 } else { 440.0 })
            .close_label("Fechar sem renomear")
            .action(Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || {
                cancel.set(false);
                cancelled.set("Renomear cancelado".to_string());
            }))
            .action(Button::new("Salvar o nome").on_click(move || {
                save.set(false);
                saved.set(format!("Projeto renomeado para {}", kept_name.get()));
            }));
        let (keep, kept) = (discard.clone(), said.clone());
        let (drop, dropped) = (discard.clone(), said.clone());
        let discard_dialog = Dialog::new(discard.binding(), "Descartar as alterações?")
            .description("Só os botões fecham: um clique fora ou o Esc não perdem o que foi escrito.")
            .close_on_backdrop(false)
            .close_on_escape(false)
            .close_button(false)
            .width(380.0)
            .action(Button::new("Continuar editando").style(ButtonStyle::Secondary).on_click(move || {
                keep.set(false);
                kept.set("As alterações continuam".to_string());
            }))
            .action(Button::new("Descartar").style(ButtonStyle::Danger).on_click(move || {
                drop.set(false);
                dropped.set("Alterações descartadas".to_string());
            }));
        let dialogs = HStack::new((
            Button::new("Renomear o projeto").on_click(opener(&rename)),
            Button::new("Descartar as alterações").style(ButtonStyle::Danger).on_click(opener(&discard)),
            rename_dialog,
            discard_dialog,
        ))
        .spacing(8.0)
        .wrap();

        // Panels from either edge.
        let (apply, applied) = (filters.clone(), said.clone());
        let filters_sheet = Sheet::new(filters.binding(), "Filtros")
            .description("O que entra na lista de faturas.")
            .side(Side::Left)
            .width(if narrow { 280.0 } else { 320.0 })
            .content(
                VStack::new((
                    Checkbox::new("Só as atrasadas", late_only.binding()),
                    Button::new("Aplicar os filtros").on_click(move || {
                        apply.set(false);
                        applied.set("Filtros aplicados".to_string());
                    }),
                ))
                .spacing(8.0)
                .align(Align::Start),
            );
        let details_sheet = Sheet::new(details.binding(), "Detalhes da fatura")
            .description("Fatura 1042, de Ana Souza.")
            .content(Text::new("Vence em 20/11 · R$ 1.280,00 · boleto"));
        let sheets = HStack::new((
            Button::new("Abrir os filtros à esquerda")
                .style(ButtonStyle::Secondary)
                .on_click(opener(&filters)),
            Button::new("Abrir os detalhes à direita")
                .style(ButtonStyle::Secondary)
                .on_click(opener(&details)),
            filters_sheet,
            details_sheet,
        ))
        .spacing(8.0)
        .wrap();

        // Popovers: on the panel below, and bare above.
        let (toggle_menu, toggle_note) = (menu.clone(), note.clone());
        let (archive, archived) = (menu.clone(), said.clone());
        let on_panel = Popover::new(
            menu.binding(),
            Button::new("Opções da exportação").style(ButtonStyle::Secondary).on_click(move || {
                toggle_menu.update(|open| *open = !*open);
            }),
            VStack::new((
                Text::new("Exportar 3 faturas como PDF"),
                Button::new("Exportar agora").on_click(move || {
                    archive.set(false);
                    archived.set("Exportação começou".to_string());
                }),
            ))
            .spacing(8.0)
            .align(Align::Start),
        )
        .min_width(if narrow { 220.0 } else { 280.0 })
        .padding(12.0);
        let bare = Popover::new(
            note.binding(),
            Button::new("Ver a recusa").style(ButtonStyle::Ghost).on_click(move || {
                toggle_note.update(|open| *open = !*open);
            }),
            Badge::new("Dois arquivos foram recusados").tone(Tone::Danger).icon(IconName::CircleAlert),
        )
        .placement(Placement::Above)
        .surface(false);
        let popovers = HStack::new((on_panel, bare)).spacing(8.0).wrap();

        // Hover and help.
        let card = HoverCard::new(
            Text::new("@ana.souza").foreground(Semantic::Accent),
            HStack::new((
                Avatar::new("Ana Souza"),
                VStack::new((
                    Text::new("Ana Souza"),
                    Text::new("Contas a receber · desde 2021").style(TextStyle::Caption),
                ))
                .spacing(2.0)
                .align(Align::Start),
            ))
            .spacing(8.0)
            .align(Align::Center),
        );
        let (saving, saved_by_key) = (said.clone(), said.clone());
        let tips = HStack::new((
            Tooltip::new(
                IconButton::new(IconName::Save)
                    .label("Salvar")
                    .on_click(move || saving.set("Salvo pelo botão".to_string())),
                "Salvar o arquivo",
            )
            .shortcut("⌘S"),
            Tooltip::new(
                IconButton::new(IconName::Printer)
                    .label("Imprimir")
                    .on_click(move || saved_by_key.set("Impressão pedida".to_string())),
                "Imprimir a fatura",
            ),
            Tooltip::new(Badge::new("Beta").tone(Tone::Accent), "Recurso em teste"),
        ))
        .spacing(8.0)
        .align(Align::Center);
        let help = HStack::new((
            FormField::new("Alíquota do ISS", TextField::new(rate.binding())),
            HelpTip::new("Alíquota do ISS", "O imposto municipal sobre o serviço, em porcentagem.")
                .how("Veja o valor na lei do município onde o serviço foi prestado."),
            HelpTip::new("Retenção", "Quando quem paga recolhe o imposto no lugar de quem recebe."),
        ))
        .spacing(8.0)
        .align(Align::End);

        cx.child(
            &VStack::new((
                VStack::new((heading("Diálogos"), dialogs)).spacing(8.0),
                VStack::new((heading("Painéis laterais"), sheets)).spacing(8.0),
                VStack::new((heading("Balões presos a um botão"), popovers)).spacing(8.0),
                VStack::new((
                    heading("Ao passar o ponteiro"),
                    HStack::new((Text::new("Responsável:"), card)).spacing(6.0).align(Align::Center),
                    tips,
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((heading("Ajuda ao lado do campo"), help)).spacing(8.0),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
