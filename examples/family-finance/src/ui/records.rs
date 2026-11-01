//! The registers: accounts, categories and family members.

use ravenrs::prelude::*;

use super::forms::Forms;
use super::store::{Store, heading};
use crate::model::{AccountKind, Flow};
use crate::money::{format_brl, format_input, parse_brl};

fn accepts_zero(text: &str) -> Result<(), String> {
    if text.trim().is_empty() { Ok(()) } else { parse_brl(text).map(|_| ()) }
}

fn cents_or_zero(text: &str) -> Option<i64> {
    if text.trim().is_empty() { Some(0) } else { parse_brl(text).ok() }
}

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct RecordsState {
    tab: State<usize>,
    account_id: State<Option<i64>>,
    account_name: State<String>,
    account_kind: State<usize>,
    account_opening: State<String>,
    category_id: State<Option<i64>>,
    category_name: State<String>,
    category_flow: State<usize>,
    category_budget: State<String>,
    member_id: State<Option<i64>>,
    member_name: State<String>,
}

impl RecordsState {
    pub fn new() -> Self {
        Self {
            tab: State::new(0),
            account_id: State::new(None),
            account_name: State::new(String::new()),
            account_kind: State::new(0),
            account_opening: State::new(String::new()),
            category_id: State::new(None),
            category_name: State::new(String::new()),
            category_flow: State::new(0),
            category_budget: State::new(String::new()),
            member_id: State::new(None),
            member_name: State::new(String::new()),
        }
    }

    fn clear_account(&self) {
        self.account_id.set(None);
        self.account_name.set(String::new());
        self.account_kind.set(0);
        self.account_opening.set(String::new());
    }

    fn clear_category(&self) {
        self.category_id.set(None);
        self.category_name.set(String::new());
        self.category_flow.set(0);
        self.category_budget.set(String::new());
    }

    fn clear_member(&self) {
        self.member_id.set(None);
        self.member_name.set(String::new());
    }
}

/// A row with an edit and a delete button.
fn record_row(
    leading: impl View + 'static,
    name: &str,
    on_edit: impl Fn() + 'static,
    on_delete: impl Fn() + 'static,
) -> AnyView {
    Box::new(
        HStack::new((
            leading,
            IconButton::new(IconName::Pencil)
                .label(format!("Alterar {name}"))
                .size(ButtonSize::Small)
                .on_click(on_edit),
            IconButton::new(IconName::Trash2)
                .label(format!("Excluir {name}"))
                .size(ButtonSize::Small)
                .on_click(on_delete),
        ))
        .spacing(6.0)
        .align(Align::Center),
    )
}

fn two_panes(list: Vec<AnyView>, form: impl View + 'static) -> impl View {
    HStack::new((
        Card::new(VStack::new(list).spacing(8.0).align(Align::Stretch)).grow(1.0),
        Card::new(form).width(340.0),
    ))
    .spacing(16.0)
    .align(Align::Start)
    .padding((16.0, 0.0))
}

fn form_buttons(
    editing: bool,
    save_label: &str,
    on_save: impl Fn() + 'static,
    on_cancel: impl Fn() + 'static,
) -> impl View {
    HStack::new((
        editing.then(|| Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(on_cancel)),
        Button::new(if editing { "Salvar alterações".to_owned() } else { save_label.to_owned() })
            .icon(if editing { IconName::Check } else { IconName::Plus })
            .on_click(on_save),
    ))
    .spacing(8.0)
}

fn accounts_tab(store: &Store, state: &RecordsState, forms: &Forms) -> impl View + use<> {
    let accounts = store.read(|db| db.accounts());
    let list = accounts
        .iter()
        .map(|account| {
            let (edit, delete) = (state.clone(), forms.confirm.clone());
            let (id, name, kind, opening) =
                (account.id, account.name.clone(), account.kind, account.opening_cents);
            let delete_store = store.clone();
            let row_name = name.clone();
            record_row(
                ListItem::new(name.clone())
                    .subtitle(kind.label())
                    .icon(IconName::Wallet)
                    .trailing(format_brl(account.balance_cents))
                    .grow(1.0),
                &row_name,
                move || {
                    edit.account_id.set(Some(id));
                    edit.account_name.set(name.clone());
                    edit.account_kind.set(AccountKind::ALL.iter().position(|k| *k == kind).unwrap_or(0));
                    edit.account_opening.set(format_input(opening));
                },
                move || {
                    let store = delete_store.clone();
                    delete.ask(
                        "Excluir esta conta?",
                        "Só é possível se ela não tiver lançamentos.",
                        "Excluir",
                        move || {
                            store.write("Conta excluída", |db| db.delete_account(id));
                        },
                    );
                },
            )
        })
        .collect();
    let editing = state.account_id.get();
    let (save, save_store, cancel) = (state.clone(), store.clone(), state.clone());
    two_panes(
        list,
        VStack::new((
            Text::new(if editing.is_some() { "Alterar conta" } else { "Nova conta" })
                .style(TextStyle::Headline),
            FormField::new(
                "Nome",
                TextField::new(state.account_name.binding()).placeholder("Ex.: Banco do bairro"),
            )
            .required(),
            FormField::new(
                "Tipo",
                Select::new(state.account_kind.binding(), AccountKind::ALL.map(AccountKind::label))
                    .label("Tipo")
                    .width(292.0),
            ),
            FormField::new(
                "Saldo inicial",
                TextField::new(state.account_opening.binding()).placeholder("0,00"),
            )
            .validate(state.account_opening.binding(), accepts_zero),
            form_buttons(
                editing.is_some(),
                "Adicionar conta",
                move || {
                    let name = save.account_name.get();
                    let Some(opening) = cents_or_zero(&save.account_opening.get()) else { return };
                    let kind = AccountKind::ALL[save.account_kind.get().min(AccountKind::ALL.len() - 1)];
                    if name.trim().is_empty() {
                        return;
                    }
                    let id = save.account_id.get();
                    let done = save_store.write("Conta salva", |db| match id {
                        Some(id) => db.update_account(id, &name, kind, opening),
                        None => db.add_account(&name, kind, opening).map(|_| ()),
                    });
                    if done.is_some() {
                        save.clear_account();
                    }
                },
                move || cancel.clear_account(),
            ),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
}

fn categories_tab(store: &Store, state: &RecordsState, forms: &Forms) -> impl View + use<> {
    let categories = store.read(|db| db.categories());
    let list = categories
        .iter()
        .map(|category| {
            let (edit, delete) = (state.clone(), forms.confirm.clone());
            let (id, name, flow, budget) =
                (category.id, category.name.clone(), category.flow, category.budget_cents);
            let delete_store = store.clone();
            let row_name = name.clone();
            let subtitle = if budget > 0 {
                format!("{} · limite de {}", flow.label(), format_brl(budget))
            } else {
                flow.label().to_owned()
            };
            record_row(
                ListItem::new(name.clone())
                    .subtitle(subtitle)
                    .icon(if flow == Flow::Income { IconName::TrendingUp } else { IconName::Tag })
                    .grow(1.0),
                &row_name,
                move || {
                    edit.category_id.set(Some(id));
                    edit.category_name.set(name.clone());
                    edit.category_flow.set(usize::from(flow == Flow::Income));
                    edit.category_budget.set(if budget > 0 { format_input(budget) } else { String::new() });
                },
                move || {
                    let store = delete_store.clone();
                    delete.ask(
                        "Excluir esta categoria?",
                        "Só é possível se nada estiver usando.",
                        "Excluir",
                        move || {
                            store.write("Categoria excluída", |db| db.delete_category(id));
                        },
                    );
                },
            )
        })
        .collect();
    let editing = state.category_id.get();
    let (save, save_store, cancel) = (state.clone(), store.clone(), state.clone());
    two_panes(
        list,
        VStack::new((
            Text::new(if editing.is_some() { "Alterar categoria" } else { "Nova categoria" })
                .style(TextStyle::Headline),
            FormField::new("Nome", TextField::new(state.category_name.binding()).placeholder("Ex.: Pets"))
                .required(),
            FormField::new(
                "Tipo",
                SegmentedControl::new(state.category_flow.binding(), ["Despesa", "Receita"]).label("Tipo"),
            ),
            FormField::new(
                "Limite mensal",
                TextField::new(state.category_budget.binding()).placeholder("Sem limite"),
            )
            .validate(state.category_budget.binding(), accepts_zero),
            form_buttons(
                editing.is_some(),
                "Adicionar categoria",
                move || {
                    let name = save.category_name.get();
                    let Some(budget) = cents_or_zero(&save.category_budget.get()) else { return };
                    let flow = if save.category_flow.get() == 1 { Flow::Income } else { Flow::Expense };
                    if name.trim().is_empty() {
                        return;
                    }
                    let id = save.category_id.get();
                    let done = save_store.write("Categoria salva", |db| match id {
                        Some(id) => db.update_category(id, &name, flow, budget),
                        None => db.add_category(&name, flow, budget).map(|_| ()),
                    });
                    if done.is_some() {
                        save.clear_category();
                    }
                },
                move || cancel.clear_category(),
            ),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
}

fn members_tab(store: &Store, state: &RecordsState, forms: &Forms) -> impl View + use<> {
    let members = store.read(|db| db.members());
    let list = members
        .iter()
        .map(|member| {
            let (edit, delete) = (state.clone(), forms.confirm.clone());
            let (id, name) = (member.id, member.name.clone());
            let delete_store = store.clone();
            let row_name = name.clone();
            record_row(
                ListItem::new(name.clone()).avatar(name.clone()).grow(1.0),
                &row_name,
                move || {
                    edit.member_id.set(Some(id));
                    edit.member_name.set(name.clone());
                },
                move || {
                    let store = delete_store.clone();
                    delete.ask(
                        "Remover esta pessoa?",
                        "Os lançamentos dela continuam, sem responsável.",
                        "Remover",
                        move || {
                            store.write("Pessoa removida", |db| db.delete_member(id));
                        },
                    );
                },
            )
        })
        .collect();
    let editing = state.member_id.get();
    let (save, save_store, cancel) = (state.clone(), store.clone(), state.clone());
    two_panes(
        list,
        VStack::new((
            Text::new(if editing.is_some() { "Alterar pessoa" } else { "Nova pessoa" })
                .style(TextStyle::Headline),
            FormField::new("Nome", TextField::new(state.member_name.binding()).placeholder("Ex.: Maria"))
                .required(),
            form_buttons(
                editing.is_some(),
                "Adicionar pessoa",
                move || {
                    let name = save.member_name.get();
                    if name.trim().is_empty() {
                        return;
                    }
                    let id = save.member_id.get();
                    let done = save_store.write("Pessoa salva", |db| match id {
                        Some(id) => db.rename_member(id, &name),
                        None => db.add_member(&name).map(|_| ()),
                    });
                    if done.is_some() {
                        save.clear_member();
                    }
                },
                move || cancel.clear_member(),
            ),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
}

pub(crate) fn records(store: &Store, state: &RecordsState, forms: &Forms) -> impl View + use<> {
    store.version();
    ScrollView::new((VStack::new((
        heading("Cadastros", "As contas, as categorias e as pessoas que os lançamentos usam."),
        Tabs::new(state.tab.binding())
            .tab("Contas", accounts_tab(store, state, forms))
            .tab("Categorias", categories_tab(store, state, forms))
            .tab("Família", members_tab(store, state, forms)),
    ))
    .spacing(16.0)
    .align(Align::Stretch),))
    .padding(24.0)
}
