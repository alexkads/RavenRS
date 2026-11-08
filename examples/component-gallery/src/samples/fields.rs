//! Fields that take text, numbers and choices.

use ravenrs::prelude::*;

samples! {
    "TextField" => text_field, "Select" => select, "Combobox" => combobox, "InputOtp" => input_otp,
    "FormField" => form_field, "Label" => label, "TextArea" => text_area, "NumberField" => number_field,
    "MaskedField" => masked_field, "LookupField" => lookup_field, "FormLayout" => form_layout,
    "FilterBuilder" => filter_builder, "ExpressionEditor" => expression_editor, "CodeEditor" => code_editor,
    "Playground" => playground, "TagBox" => tag_box, "InlineRename" => inline_rename,
}

// sample: TextArea, 170
fn text_area() -> impl View {
    let notes = State::new(String::from("Two lines\nto start with."));
    // Grows from 2 to 6 lines as you type; `.rows(4)` would fix the height.
    TextArea::new(notes.binding()).placeholder("Notes").grows(2, 6)
}
// end

// sample: NumberField, 110
fn number_field() -> impl View {
    let (seats, price) = (State::new(Some(12.0)), State::new(Some(129_990_i64)));
    VStack::new((
        NumberField::new(seats.binding()).range(0.0, 100.0).spin_buttons().label("Seats"),
        NumberField::money(price.binding(), "R$ ").label("Price"),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: MaskedField, 90
fn masked_field() -> impl View {
    let phone = State::new(String::from("11987654321"));
    MaskedField::new(phone.binding(), "(00) 00000-0000").label("Phone")
}
// end

// sample: LookupField, 90
fn lookup_field() -> impl View {
    let city = State::new(None);
    LookupField::remote(city.binding(), |request, responder| {
        // Ask a database or a server here; a clone of the responder can answer later.
        let cities = ["Belo Horizonte", "Recife", "Salvador", "São Paulo"];
        let needle = request.query.to_lowercase();
        let found = cities.iter().filter(|c| c.to_lowercase().contains(&needle));
        responder.respond(found.map(|c| LookupItem::new(*c, *c)).collect());
    })
    .placeholder("Choose a city")
    .width(220.0)
    .label("City")
}
// end

// sample: FormLayout, 260
fn form_layout() -> impl View {
    let (name, city, notes) =
        (State::new(String::new()), State::new(String::new()), State::new(String::new()));
    FormLayout::new().columns(2).group(
        FormGroup::titled("Contact")
            .field(FormField::new("Name", TextField::new(name.binding())).required())
            .field(FormField::new("City", TextField::new(city.binding())))
            .wide(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
    )
}
// end

// sample: FilterBuilder, 240
fn filter_builder() -> impl View {
    let filter = State::new(String::from("[Price] > 10 And Contains([Name], 'pro')"));
    FilterBuilder::new(
        filter.binding(),
        [
            FilterField::new("Name", "Name", FilterFieldKind::Text),
            FilterField::new("Price", "Price", FilterFieldKind::Number),
            FilterField::new("Active", "Active", FilterFieldKind::Boolean),
        ],
    )
}
// end

// sample: TagBox, 300
fn tag_box() -> impl View {
    let tags = State::new(vec![String::from("urgent")]);
    TagBox::new(
        tags.binding(),
        vec![
            LookupItem::new("urgent", "Urgent"),
            LookupItem::new("review", "To review"),
            LookupItem::new("paid", "Paid"),
        ],
    )
    .allow_new(true)
    .label("Tags")
}
// end

// sample: TextField, 180
fn text_field() -> impl View {
    let name = State::new(String::new());
    let price = State::new(String::from("49,90"));
    let shown = name.clone();
    VStack::new((
        TextField::new(name.binding()).placeholder("Full name").icon(IconName::User).clearable(),
        TextField::new(price.binding())
            .placeholder("Price")
            .size(ButtonSize::Small)
            .prefix(Text::new("R$").foreground(Semantic::InkMuted))
            .suffix(Badge::new("per month")),
        Text::dynamic(move || format!("Hello, {}", shown.get())).foreground(Semantic::InkMuted),
    ))
    .spacing(12.0)
    .align(Align::Stretch)
}
// end

// sample: Select, 320
fn select() -> impl View {
    let range = State::new(0);
    // `Select::of` holds the value itself; `.searchable()` adds a search box.
    let currency = State::new("BRL");
    VStack::new((
        Select::new(range.binding(), ["Last 7 days", "Last 30 days", "This year"]).label("Date range"),
        Select::of(currency.binding(), [("BRL", "Real"), ("EUR", "Euro"), ("USD", "Dollar")])
            .label("Currency")
            .size(ButtonSize::Small)
            .searchable(),
    ))
    .spacing(8.0)
}
// end

// sample: Combobox, 320
fn combobox() -> impl View {
    let country = State::new(None);
    Combobox::new(country.binding(), ["Argentina", "Brazil", "Chile", "Portugal", "Spain"])
        .placeholder("Choose a country")
        .label("Country")
}
// end

// sample: ExpressionEditor, 420
fn expression_editor() -> impl View {
    let expression = State::new("[Quantity] * [UnitPrice]".to_owned());
    ExpressionEditor::new(expression.binding())
        .field(FilterField::new("Quantity", "Quantity", FilterFieldKind::Number))
        .field(FilterField::new("UnitPrice", "Unit price", FilterFieldKind::Number))
        .parameter(FilterField::new("MinTotal", "Least total", FilterFieldKind::Number))
}
// end

// sample: CodeEditor, 260
fn code_editor() -> impl View {
    let code = State::new(
        "fn total(prices: &[f64]) -> f64 {\n    // Sum of every price\n    prices.iter().sum()\n}\n"
            .to_owned(),
    );
    CodeEditor::new(code.binding())
        .diagnostics(vec![
            CodeDiagnostic::warning(3, 5, 6, "`prices` could be empty").hint("Return 0.0 first"),
        ])
        .rows(10)
}
// end

// sample: Playground, 420
fn playground() -> impl View {
    let run = |code: &str| -> Result<AnyView, Vec<CodeDiagnostic>> {
        match code.trim().strip_prefix("say ") {
            Some(words) => Ok(Box::new(Text::new(words.to_owned()).style(TextStyle::Title))),
            None => Err(vec![CodeDiagnostic::error(1, 1, 1, "Start with `say`").hint("Write `say hello`")]),
        }
    };
    Playground::new(run)
        .language(CodeLanguage::Plain)
        .example("Hello", "say hello")
        .example("Goodbye", "say goodbye")
        .height(380.0)
}
// end

// sample: InputOtp, 90
fn input_otp() -> impl View {
    let code = State::new(String::new());
    InputOtp::new(code.binding(), 6)
}
// end

// sample: FormField, 130
fn form_field() -> impl View {
    let email = State::new(String::new());
    FormField::new("Work email", TextField::new(email.binding()).placeholder("you@company.com"))
        .required()
        .hint("We send the invoice here.")
        .validate(email.binding(), validate::email)
}
// end

// sample: Label, 80
fn label() -> impl View {
    Label::new("Company name")
}
// end

// sample: InlineRename, 80
fn inline_rename() -> impl View {
    let name = State::new(String::from("Warm evening"));
    VStack::new((
        InlineRename::new(name.binding()).label("Preset name").width(220.0),
        Text::new("Double click it, or press F2 on it.")
            .style(TextStyle::Caption)
            .foreground(Semantic::InkMuted),
    ))
    .spacing(4.0)
}
// end
