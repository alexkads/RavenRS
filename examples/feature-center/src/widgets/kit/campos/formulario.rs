//! The fields of a form, every feature (#579): an order laid out by `FormLayout` in groups and
//! tabs, with numbers, masks, a long text that undoes from a button, stars, a value that opens a
//! slider and a name renamed in place. A line says what changed last.

use ravenrs::core::Color;
use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "FormFieldsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Campos de formulário";

/// The name the order starts with.
pub const ORDER: &str = "Pedido 1042";

fn section(title: &str, body: impl View + 'static) -> impl View {
    VStack::new((Text::new(title.to_owned()).style(TextStyle::Headline), body))
        .spacing(8.0)
        .align(Align::Stretch)
}

/// The page: the order, the variants of each field beside it, and a line with the last change.
#[derive(Debug, Default)]
pub struct FormFieldsPage;

impl View for FormFieldsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let wide = if narrow { 220.0 } else { 280.0 };
        let said = cx.use_state(|| "Mude um campo e saia dele com Tab.".to_string());
        let name = cx.use_state(|| ORDER.to_string());
        let renaming = cx.use_state(|| false);
        let quantity = cx.use_state(|| Some(3.0_f64));
        let discount = cx.use_state(|| Some(5.0_f64));
        let weight = cx.use_state(|| Some(1250.5_f64));
        let price = cx.use_state(|| Some(4_990_i64));
        let phone = cx.use_state(|| "11987654321".to_string());
        let cpf = cx.use_state(String::new);
        let plate = cx.use_state(|| "ABC1D23".to_string());
        let notes = cx.use_state(|| "Entregar pela manhã.".to_string());
        let cursor = cx.use_state(Cursor::default);
        let history = cx.use_state(TextHistory::new);
        let focus = cx.use_state(|| false);
        let service = cx.use_state(|| 4_u32);
        let grade = cx.use_state(|| 7_u32);
        let off = cx.use_state(|| 10.0_f32);
        let volume = cx.use_state(|| 50.0_f32);

        // What keeps the order from being saved, at the top of the form.
        let mut errors = Vec::new();
        if name.with(|n| n.trim().is_empty()) {
            errors.push("Dê um nome ao pedido".to_string());
        }
        if phone.with(|p| p.chars().count() < 11) {
            errors.push("O telefone precisa de 11 dígitos".to_string());
        }
        let total = quantity.get().unwrap_or(0.0) * price.get().unwrap_or(0) as f64 / 100.0
            * (1.0 - discount.get().unwrap_or(0.0) / 100.0);

        let left = said.clone();
        let order = FormGroup::titled("Pedido")
            .columns(if narrow { 1 } else { 2 })
            .field(FormField::new(
                "Quantidade",
                NumberField::new(quantity.binding())
                    .decimals(0)
                    .range(1.0, 999.0)
                    .step(1.0)
                    .spin_buttons()
                    .label("Quantidade")
                    .on_leave(move || left.set("Quantidade conferida ao sair do campo".to_string())),
            ))
            .field(FormField::new(
                "Preço unitário",
                NumberField::money(price.binding(), "R$ ").grouping(true).label("Preço unitário"),
            ))
            .field(FormField::new(
                "Desconto",
                NumberField::new(discount.binding())
                    .decimals(1)
                    .range(0.0, 100.0)
                    .suffix(" %")
                    .placeholder("0,0")
                    .label("Desconto")
                    .width(140.0),
            ))
            .field(FormField::new(
                "Peso",
                NumberField::new(weight.binding())
                    .decimals(1)
                    .prefix("≈ ")
                    .suffix(" g")
                    .grouping(false)
                    .label("Peso"),
            ))
            .field(FormField::new(
                "Total",
                NumberField::new(Binding::constant(Some(total)))
                    .decimals(2)
                    .prefix("R$ ")
                    .read_only(true)
                    .label("Total"),
            ))
            .field(FormField::new(
                "Frete",
                NumberField::new(Binding::constant(None)).placeholder("Grátis").disabled(true).label("Frete"),
            ));
        let contact = FormGroup::titled("Contato")
            .columns(if narrow { 1 } else { 3 })
            .field(FormField::new(
                "Telefone",
                MaskedField::new(phone.binding(), "(00) 00000-0000")
                    .label("Telefone")
                    .placeholder("(11) 91234-5678")
                    .width(wide),
            ))
            .field(FormField::new(
                "CPF com pontos",
                MaskedField::new(cpf.binding(), "000.000.000-00").keep_literals().label("CPF com pontos"),
            ))
            .field(FormField::new(
                "Placa",
                MaskedField::new(plate.binding(), "aaa0*00").read_only(true).label("Placa"),
            ))
            .field(FormField::new(
                "Código interno",
                MaskedField::new(Binding::constant(String::new()), "\\X-000")
                    .disabled(true)
                    .label("Código interno"),
            ));

        let (undo_notes, undo_cursor, undo_history, undoing) =
            (notes.clone(), cursor.clone(), history.clone(), said.clone());
        let (focusing, focus_said) = (focus.clone(), said.clone());
        let caret = cursor.get().caret;
        let observations = VStack::new((
            TextArea::new(notes.binding())
                .placeholder("Observações para a entrega")
                .rows(4)
                .width(if narrow { 260.0 } else { 520.0 })
                .cursor(cursor.clone())
                .history(history.clone())
                .focus_request(focus.clone()),
            HStack::new((
                Button::new("Desfazer nas observações").style(ButtonStyle::Secondary).on_click(move || {
                    let (mut text, mut at) = (undo_notes.get(), undo_cursor.get());
                    let mut steps = undo_history.get();
                    if steps.undo(&mut text, &mut at) {
                        undo_notes.set(text);
                        undo_cursor.set(at);
                        undo_history.set(steps);
                        undoing.set("Observações: desfeito".to_string());
                    } else {
                        undoing.set("Observações: nada a desfazer".to_string());
                    }
                }),
                Button::new("Escrever nas observações").style(ButtonStyle::Secondary).on_click(move || {
                    focusing.set(true);
                    focus_said.set("Observações com o teclado".to_string());
                }),
                Text::new(format!("Cursor na posição {caret}")).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0)
            .wrap(),
            TextArea::new(Binding::constant(String::new()))
                .placeholder("Cresce de 2 a 6 linhas")
                .grows(2, 6)
                .width(if narrow { 260.0 } else { 520.0 }),
            FormField::new(
                "Origem, só para ler",
                TextArea::new(Binding::constant("Pedido criado pelo site.".to_string()))
                    .rows(2)
                    .read_only(true),
            ),
            FormField::new(
                "Notas internas, desligadas",
                TextArea::new(Binding::constant("Sem acesso".to_string())).rows(2).disabled(true),
            ),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let score = service.get();
        let ratings = VStack::new((
            HStack::new((
                Rating::new(service.binding())
                    .label("Atendimento")
                    .max(5)
                    .size(24.0)
                    .tone(Tone::Warning)
                    .clearable(),
                Text::new(format!("Atendimento: {score} de 5")),
            ))
            .spacing(12.0),
            HStack::new((
                Rating::new(grade.binding())
                    .label("Nota de 0 a 10")
                    .max(10)
                    .size(14.0)
                    .color(Color::hex(0xE0457B)),
                Text::new(format!("Nota: {}", grade.get())),
            ))
            .spacing(12.0),
            HStack::new((
                Rating::read_only(4).label("Média da loja").tone(Tone::Success),
                Text::new("Média da loja, só para ler").foreground(Semantic::InkMuted),
            ))
            .spacing(12.0),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let form = FormLayout::new()
            .columns(if narrow { 1 } else { 2 })
            .errors(errors)
            .group(order)
            .group(contact)
            .tab("Observações", observations)
            .tab("Avaliação", ratings);

        let (renamed, clicked, starting) = (said.clone(), said.clone(), renaming.clone());
        let title = HStack::new((
            InlineRename::new(name.binding())
                .label("Nome do pedido")
                .placeholder("Sem nome")
                .size(ButtonSize::Medium)
                .width(wide)
                .editing(renaming.binding())
                .on_rename(move |new| renamed.set(format!("Renomeado para {new}")))
                .on_click(move || clicked.set("Clique no nome: F2 ou dois cliques renomeiam".to_string())),
            Button::new("Renomear").style(ButtonStyle::Secondary).on_click(move || starting.set(true)),
            InlineRename::new(Binding::constant("Pedido arquivado".to_string()))
                .label("Arquivado")
                .disabled(true),
        ))
        .spacing(12.0)
        .wrap();

        let quick = HStack::new((
            ValueSlider::new(off.binding())
                .range(0.0, 30.0)
                .step(0.5)
                .unit(" %")
                .label("Desconto rápido")
                .show_label(),
            ValueSlider::new(volume.binding())
                .range(0.0, 100.0)
                .label("Volume do aviso")
                .value_format(|v| if v < 1.0 { "mudo".to_string() } else { format!("{v:.0} de 100") })
                .neutral(50.0)
                .slider_width(if narrow { 180.0 } else { 260.0 })
                .field(),
            ValueSlider::new(Binding::constant(0.5)).label("Brilho").disabled(true),
            Text::new(
                format!("Desconto rápido: {:.1} % · volume {:.0}", off.get(), volume.get()).replace('.', ","),
            )
            .foreground(Semantic::InkMuted),
        ))
        .spacing(12.0)
        .wrap();

        cx.child(
            &VStack::new((
                section("Nome no lugar", title),
                section("Pedido", form),
                section("Valores rápidos", quick),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
