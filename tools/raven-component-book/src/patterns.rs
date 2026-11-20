//! Patterns: small screens made only of components, as a design composes them. The book measures
//! and exports them as it does a component (family `Patterns`), so the design tool sees how the
//! pieces meet, the spaces between them and what stretches, and does not invent a layout.

use ravenrs::prelude::*;
use ravenrs::widgets::book::{Args, Extent, Meta, Sizing};

use crate::index::Entry;

/// The patterns, in the order the design system lists them.
pub fn entries() -> Vec<Entry> {
    [
        Meta::new("Patterns/FormPattern", |_: &Args| -> AnyView { Box::new(form()) })
            .summary("A form: labeled fields in two columns, a hint, an error and the actions at the end, the one that matters accented."),
        Meta::new("Patterns/SidebarPage", |_: &Args| -> AnyView { Box::new(sidebar_page()) })
            .summary("A page beside its navigation: the sidebar on the left, the page header and the content on the same margin."),
        Meta::new("Patterns/FilteredGrid", |_: &Args| -> AnyView { Box::new(filtered_grid()) })
            .summary("A list of records: the page header, a bar with search and filters, and the grid under it, filling the width."),
        Meta::new("Patterns/Confirmation", |_: &Args| -> AnyView { Box::new(confirmation()) })
            .summary("Asking before an action that cannot be undone: the question, what follows and the two answers."),
        Meta::new("Patterns/EmptyPage", |_: &Args| -> AnyView { Box::new(empty_page()) })
            .summary("A page with nothing yet: what will be here, how to start and a way to learn more."),
    ]
    .into_iter()
    // A pattern is a screen: it takes the width it is given.
    .map(|meta| Entry {
        meta: meta.story("Pattern", Args::new()).sizing(Sizing::new().width(Extent::Fill)),
        pending: false,
        code: None,
    })
    .collect()
}

fn form() -> impl View {
    let (name, email, phone, plan, notes) = (
        State::new(String::from("Ana Lima")),
        State::new(String::from("ana@")),
        State::new(String::new()),
        State::new(0),
        State::new(String::new()),
    );
    let check = email.clone();
    VStack::new((
        FormLayout::new().columns(2).group(
            FormGroup::titled("Customer")
                .field(FormField::new("Name", TextField::new(name.binding())).required())
                .field(
                    FormField::new("Email", TextField::new(email.binding()))
                        .required()
                        .validate(check.binding(), validate::email)
                        .show_errors(true),
                )
                .field(
                    FormField::new("Phone", TextField::new(phone.binding()).placeholder("(11) 91234-5678"))
                        .hint("For the delivery only."),
                )
                .field(FormField::new("Plan", Select::new(plan.binding(), ["Monthly", "Yearly"])))
                .wide(FormField::new("Notes", TextArea::new(notes.binding()).rows(2))),
        ),
        HStack::new((
            Spacer::new(),
            Button::new("Cancel").style(ButtonStyle::Secondary),
            Button::new("Save customer").icon(IconName::Check),
        ))
        .spacing(10.0)
        .align(Align::Center),
    ))
    .spacing(24.0)
    .align(Align::Stretch)
}

fn sidebar_page() -> impl View {
    let state = SidebarState::new(true);
    HStack::new((
        Sidebar::new((SidebarGroup::new(
            "Workspace",
            (
                SidebarItem::new(IconName::LayoutDashboard, "Overview"),
                SidebarItem::new(IconName::Users, "Customers").selected(true).badge(Badge::new("12")),
                SidebarItem::new(IconName::Settings, "Settings"),
            ),
        ),))
        .state(state)
        // In a narrow window it takes no room and opens as a sheet, as an application asks.
        .narrow_below(600.0),
        VStack::new((
            PageHeader::new("Customers")
                .description("Everyone who bought this year.")
                .action(Button::new("New customer").icon(IconName::Plus)),
            Card::new(Text::new("The content of the page.").foreground(Semantic::InkMuted)),
        ))
        .spacing(16.0)
        .padding(16.0)
        .align(Align::Stretch)
        .grow(1.0),
    ))
    .height(420.0)
    .align(Align::Stretch)
}

fn filtered_grid() -> impl View {
    let (search, status) = (State::new(String::new()), State::new(0));
    let rows = (0..8)
        .map(|i| {
            vec![
                Value::Text(format!("INV-{}", 1040 + i)),
                Value::Text(["Northwind Labs", "Aurora Studio", "Lumen Co."][i % 3].to_owned()),
                Value::Number(f64::from(i as u32 + 1) * 480.0),
            ]
        })
        .collect();
    VStack::new((
        PageHeader::new("Invoices").description("Sent this month."),
        HStack::new((
            TextField::new(search.binding())
                .placeholder("Search invoices")
                .icon(IconName::Search)
                .width(280.0),
            Select::new(status.binding(), ["All", "Open", "Paid"]).label("Status"),
            Spacer::new(),
            Button::new("Export").style(ButtonStyle::Secondary).icon(IconName::Download),
        ))
        .spacing(8.0)
        .align(Align::Center),
        DataGrid::new(
            GridData::new(rows),
            [
                GridColumn::text("Invoice").width(140.0),
                GridColumn::text("Customer").width(240.0),
                GridColumn::currency("Amount", "R$ ", 2).width(160.0).summary(Summary::Sum),
            ],
        )
        .striped(true)
        .height(340.0),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}

fn confirmation() -> impl View {
    let open = State::new(true);
    let (cancel, confirm) = (open.clone(), open.clone());
    Dialog::new(open.binding(), "Delete this invoice?")
        .description("INV-1042 goes away with its payments. This cannot be undone.")
        .action(Button::new("Cancel").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)))
        .action(
            Button::new("Delete")
                .style(ButtonStyle::Danger)
                .icon(IconName::Trash2)
                .on_click(move || confirm.set(false)),
        )
        .close_on_backdrop(false)
}

fn empty_page() -> impl View {
    VStack::new((
        PageHeader::new("Invoices").action(Button::new("New invoice").icon(IconName::Plus)),
        Card::new(
            EmptyState::new("No invoices yet")
                .icon(IconName::FileText)
                .description("Invoices you send show up here, newest first.")
                .action(Button::new("New invoice"))
                .action(Button::new("Learn more").style(ButtonStyle::Ghost)),
        ),
    ))
    .spacing(16.0)
    .align(Align::Stretch)
}
