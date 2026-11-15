//! `Select`, `Combobox` and `CommandPalette`, every feature: a delivery form with a list to search,
//! a value of any type, sizes and widths, fields that are off or only read, a city typed to filter,
//! and a palette of commands opened by a button or Cmd/Ctrl+K. A line says what was picked or run.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "PickListsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Listas de escolha";

/// The countries of the searchable select.
pub const COUNTRIES: [&str; 6] = ["Brasil", "Argentina", "Chile", "Portugal", "Uruguai", "Paraguai"];
/// The cities of the combobox.
pub const CITIES: [&str; 5] = ["São Paulo", "Rio de Janeiro", "Belo Horizonte", "Curitiba", "Recife"];

/// A currency, picked by value and not by position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Currency {
    /// The Brazilian real.
    Real,
    /// The US dollar.
    Dollar,
    /// The euro.
    Euro,
}

impl Currency {
    fn code(self) -> &'static str {
        match self {
            Self::Real => "BRL",
            Self::Dollar => "USD",
            Self::Euro => "EUR",
        }
    }
}

fn heading(text: &str) -> Text {
    Text::new(text.to_owned()).style(TextStyle::Headline)
}

/// The page: the form, the palette and a line with the last thing picked or run.
#[derive(Debug, Default)]
pub struct PickListsPage;

impl View for PickListsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha um país, uma cidade ou rode um comando.".to_string());
        let country = cx.use_state(|| 0_usize);
        let currency = cx.use_state(|| Currency::Real);
        let speed = cx.use_state(|| 0_usize);
        let store = cx.use_state(|| 0_usize);
        let city = cx.use_state(|| None::<usize>);
        let neighbour = cx.use_state(|| Some(0_usize));
        let open = cx.use_state(|| false);
        let width = if narrow { 160.0 } else { 220.0 };

        let lists = VStack::new((
            Select::new(country.binding(), COUNTRIES).label("País").searchable().width(width),
            Select::of(
                currency.binding(),
                [(Currency::Real, "Real"), (Currency::Dollar, "Dólar"), (Currency::Euro, "Euro")],
            )
            .label("Moeda")
            .width(width),
            Select::new(speed.binding(), ["Normal", "Expressa"])
                .label("Entrega")
                .size(ButtonSize::Small)
                .width(width),
            VStack::new((Select::new(store.binding(), ["Loja Centro", "Loja Norte"]).label("Loja").fill(),))
                .width(if narrow { 240.0 } else { 320.0 })
                .align(Align::Stretch),
            Select::new(store.binding(), ["Loja Centro", "Loja Norte"])
                .label("Loja de origem (só leitura)")
                .read_only(true),
            Select::new(speed.binding(), ["Normal", "Expressa"])
                .label("Transportadora (desligada)")
                .disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let cities = VStack::new((
            Combobox::new(city.binding(), CITIES)
                .label("Cidade")
                .placeholder("Digite para filtrar")
                .width(width),
            VStack::new((Combobox::new(neighbour.binding(), CITIES).label("Cidade vizinha").fill(),))
                .width(if narrow { 240.0 } else { 320.0 })
                .align(Align::Stretch),
            Combobox::new(neighbour.binding(), CITIES).label("Cidade da matriz (só leitura)").read_only(true),
            Combobox::new(city.binding(), CITIES).label("Cidade de cobrança (desligada)").disabled(true),
        ))
        .spacing(8.0)
        .align(Align::Start);

        let summary = format!(
            "Destino: {} · {} · moeda {} · {}",
            COUNTRIES[country.get().min(COUNTRIES.len() - 1)],
            city.get().map_or("cidade não escolhida", |i| CITIES[i.min(CITIES.len() - 1)]),
            currency.get().code(),
            if speed.get() == 0 { "entrega normal" } else { "entrega expressa" },
        );

        let run = |text: &'static str| {
            let line = said.clone();
            move || line.set(format!("Comando: {text}"))
        };
        let palette = CommandPalette::new(open.binding())
            .placeholder("Procure um comando")
            .command(
                Command::new("Novo pedido", run("Novo pedido"))
                    .group("Pedidos")
                    .icon(IconName::Plus)
                    .shortcut("⌘N"),
            )
            .command(
                Command::new("Duplicar pedido", run("Duplicar pedido")).group("Pedidos").icon(IconName::Copy),
            )
            .command(
                Command::new("Imprimir etiqueta", run("Imprimir etiqueta"))
                    .group("Entrega")
                    .icon(IconName::Printer),
            )
            .command(Command::new("Rastrear entrega", run("Rastrear entrega")).group("Entrega"));
        // The center has a palette of its own on Cmd/Ctrl+K, and a palette always answers to those
        // keys: this one is only there while it is open, so the keys keep opening the center's.
        let opener = open.clone();
        let shown: AnyView = if open.get() { Box::new(palette) } else { Box::new(Spacer::new()) };
        let commands = HStack::new((
            Button::new("Abrir a paleta de comandos")
                .icon(IconName::Command)
                .on_click(move || opener.set(true)),
            shown,
        ))
        .spacing(8.0);

        cx.child(
            &VStack::new((
                VStack::new((heading("Listas"), lists)).spacing(8.0),
                VStack::new((heading("Digitar para filtrar"), cities)).spacing(8.0),
                Text::new(summary),
                VStack::new((heading("Paleta de comandos"), commands)).spacing(8.0).align(Align::Start),
                Text::new(said.get()),
            ))
            .spacing(16.0)
            .padding(if narrow { 0.0 } else { 16.0 })
            .align(Align::Stretch),
        )
    }
}
