//! `Sidebar`, `SidebarBrand`, `Breadcrumb`, `Tabs`, `Accordion` and `Disclosure`, every feature
//! (#579): the menu of a shop that closes to icons, floats or sits on the other side, the path
//! of an order that goes back a level at a time, the tabs of the order and of its period, and
//! its details folded in an accordion. A line says what happened last.

use ravenrs::prelude::*;

/// The control's name, as the session knows it.
pub const ID: &str = "NavigationDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Navegação";

/// How the menu sits next to the page, in the order of `SidebarVariant`.
pub const VARIANTS: [&str; 3] = ["Padrão", "Flutuante", "Embutida"];
/// What the menu does when it closes, in the order of `SidebarCollapse`.
pub const COLLAPSES: [&str; 3] = ["Fixa", "Ícones", "Some"];
/// The path from the shop to the order.
pub const PATH: [&str; 4] = ["Loja", "Pedidos", "2026", "Pedido 1042"];
/// The tabs of the order.
pub const TABS: [&str; 3] = ["Resumo", "Itens", "Entrega"];
/// The periods of the second bar of tabs.
pub const PERIODS: [&str; 3] = ["Dia", "Semana", "Mês"];

/// The page: the menu beside the order, and a line with the last event.
#[derive(Debug, Default)]
pub struct NavigationPage;

impl View for NavigationPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let narrow = cx.viewport().width < 720.0;
        let said = cx.use_state(|| "Escolha um item do menu ou um nível do caminho.".to_string());
        let variant = cx.use_state(|| 0_usize);
        let collapse = cx.use_state(|| 1_usize);
        let right = cx.use_state(|| false);
        let open = cx.use_state(|| true);
        let depth = cx.use_state(|| PATH.len() - 1);
        let tab = cx.use_state(|| 0_usize);
        let period = cx.use_state(|| 1_usize);
        let locked = cx.use_state(|| false);
        let delivery = cx.use_state(|| false);
        let notify = cx.use_state(|| true);
        // Kept between builds, so the trigger and the menu share it.
        let state = cx.use_state(|| SidebarState::controlled(open.binding())).get();

        let item = |icon: IconName, label: &'static str, line: &State<String>| {
            let line = line.clone();
            SidebarItem::new(icon, label).on_click(move || line.set(format!("Menu: {label}")))
        };
        let menu = Sidebar::new((
            SidebarSection::new("Vendas"),
            item(IconName::LayoutDashboard, "Painel", &said).selected(true),
            item(IconName::ShoppingCart, "Pedidos", &said).count("3"),
            item(IconName::Users, "Clientes", &said),
            SidebarSection::new("Ajustes"),
            item(IconName::Settings, "Configurações", &said),
        ))
        .width(if narrow { 200.0 } else { 220.0 })
        .variant(match variant.get() {
            1 => SidebarVariant::Floating,
            2 => SidebarVariant::Inset,
            _ => SidebarVariant::Standard,
        })
        .collapsible(match collapse.get() {
            0 => SidebarCollapse::Fixed,
            2 => SidebarCollapse::Hidden,
            _ => SidebarCollapse::Icons,
        })
        .side(if right.get() { Side::Right } else { Side::Left })
        // On a phone it opens as a sheet over the page.
        .narrow_below(600.0)
        // Cmd/Ctrl+B belongs to the center's own navigation.
        .shortcut(false)
        .rail(true)
        .label("Menu da loja")
        .state(state.clone())
        .header(SidebarBrand::new(Icon::new(IconName::Compass).size(26.0), "Raven Loja").subtitle("Matriz"))
        .footer(Text::new("Ana · caixa 2").style(TextStyle::Caption));

        let back = said.clone();
        let level = depth.clone();
        let path = Breadcrumb::new(PATH.iter().take(depth.get() + 1).copied())
            .root_icon(IconName::House)
            .on_navigate(move |i| {
                level.set(i);
                back.set(format!("Caminho: voltou para {}", PATH[i]));
            });

        let order = Tabs::new(tab.binding())
            .tab(TABS[0], Text::new("Pedido 1042 · R$ 318,40 · pago"))
            .tab(TABS[1], Text::new("3 itens: café, filtro e caneca"))
            .tab(TABS[2], Text::new("Entrega na sexta, das 9h às 12h"));
        let periods = Tabs::new(period.binding())
            .variant(TabsVariant::Segmented)
            .size(ButtonSize::Small)
            .disabled(locked.get())
            .item(PERIODS[0])
            .item(PERIODS[1])
            .item(PERIODS[2]);

        let details = Accordion::new(vec![
            Disclosure::new("Pagamento", Text::new("Cartão de crédito em 2 vezes")).open(true).key(1),
            Disclosure::new("Endereço de entrega", Text::new("Rua das Flores, 120 · Centro"))
                .expanded(delivery.binding())
                .mark(Dot::new().tone(Tone::Warning).label("Falta confirmar"))
                .key(2),
            Disclosure::new("Avisos ao cliente", Text::new("Mensagem quando sair para entrega"))
                .title_view(
                    HStack::new((Icon::new(IconName::Bell).size(16.0), Text::new("Avisos ao cliente")))
                        .spacing(8.0)
                        .align(Align::Center),
                )
                .switch(notify.binding())
                .key(3),
        ]);

        let pickers: AnyView = if narrow {
            Box::new(
                VStack::new((
                    Select::new(variant.binding(), VARIANTS).label("Jeito do menu"),
                    Select::new(collapse.binding(), COLLAPSES).label("Ao fechar"),
                ))
                .spacing(8.0),
            )
        } else {
            Box::new(
                HStack::new((
                    SegmentedControl::new(variant.binding(), VARIANTS),
                    SegmentedControl::new(collapse.binding(), COLLAPSES),
                ))
                .spacing(12.0),
            )
        };

        let page = VStack::new((
            HStack::new((SidebarTrigger::new(state.clone()), path)).spacing(12.0).align(Align::Center),
            pickers,
            Checkbox::new("Menu do lado direito", right.binding()),
            Text::new("Pedido").style(TextStyle::Headline),
            order,
            HStack::new((Text::new("Período"), periods)).spacing(12.0).align(Align::Center),
            Checkbox::new("Travar o período", locked.binding()),
            details,
            Text::new(format!(
                "Menu {} · período: {} · endereço {} · avisos {}",
                if open.get() { "aberto" } else { "fechado" },
                PERIODS[period.get().min(2)],
                if delivery.get() { "à vista" } else { "dobrado" },
                if notify.get() { "ligados" } else { "desligados" },
            )),
            Text::new(said.get()),
        ))
        .spacing(12.0)
        .padding(if narrow { 0.0 } else { 16.0 })
        .align(Align::Stretch)
        .grow(1.0);

        let row: AnyView = if right.get() {
            Box::new(HStack::new((page, menu)).spacing(12.0).align(Align::Stretch))
        } else {
            Box::new(HStack::new((menu, page)).spacing(12.0).align(Align::Stretch))
        };
        cx.child(&VStack::new((row,)).height(if narrow { 900.0 } else { 640.0 }).align(Align::Stretch))
    }
}
