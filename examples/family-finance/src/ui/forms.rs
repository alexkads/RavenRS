//! The dialogs: transactions, bills, paying a bill and confirming.

use std::rc::Rc;

use ravenrs::prelude::*;

use super::store::Store;
use crate::model::{
    Account, AccountKind, BillInput, Category, Flow, Member, Occurrence, Recurrence, ScheduleInput,
    TransactionInput, date_label,
};
use crate::money::{format_brl, format_input, parse_brl, validate_positive};

/// The account most money moves through, which forms start on.
fn usual_account(accounts: &[Account]) -> usize {
    accounts.iter().position(|a| a.kind == AccountKind::Checking).unwrap_or(0)
}

fn categories_of(categories: &[Category], flow: Flow) -> Vec<Category> {
    categories.iter().filter(|c| c.flow == flow).cloned().collect()
}

fn member_options(members: &[Member]) -> Vec<String> {
    std::iter::once("Família toda".to_owned()).chain(members.iter().map(|m| m.name.clone())).collect()
}

fn member_at(members: &[Member], index: usize) -> Option<i64> {
    index.checked_sub(1).and_then(|i| members.get(i)).map(|m| m.id)
}

fn member_index(members: &[Member], id: Option<i64>) -> usize {
    id.and_then(|id| members.iter().position(|m| m.id == id)).map_or(0, |i| i + 1)
}

/// A new transaction, or changing one.
#[derive(Clone)]
pub(crate) struct TransactionForm {
    open: State<bool>,
    editing: State<Option<i64>>,
    date: State<Option<Date>>,
    description: State<String>,
    amount: State<String>,
    flow: State<usize>,
    account: State<usize>,
    category: State<usize>,
    member: State<usize>,
    tried: State<bool>,
}

impl TransactionForm {
    pub fn new() -> Self {
        Self {
            open: State::new(false),
            editing: State::new(None),
            date: State::new(None),
            description: State::new(String::new()),
            amount: State::new(String::new()),
            flow: State::new(0),
            account: State::new(0),
            category: State::new(0),
            member: State::new(0),
            tried: State::new(false),
        }
    }

    pub fn start_new(&self, store: &Store, flow: Flow) {
        self.editing.set(None);
        self.date.set(Some(store.today));
        self.description.set(String::new());
        self.amount.set(String::new());
        self.flow.set(usize::from(flow == Flow::Income));
        self.account.set(usual_account(&store.read(|db| db.accounts())));
        self.category.set(0);
        self.member.set(0);
        self.tried.set(false);
        self.open.set(true);
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        let Some(found) = store.read(|db| db.transaction(id)) else { return };
        let accounts = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), found.input.flow);
        let members = store.read(|db| db.members());
        self.editing.set(Some(id));
        self.date.set(Some(found.input.date));
        self.description.set(found.input.description);
        self.amount.set(format_input(found.input.amount_cents));
        self.flow.set(usize::from(found.input.flow == Flow::Income));
        self.account.set(accounts.iter().position(|a| a.id == found.input.account_id).unwrap_or(0));
        self.category.set(categories.iter().position(|c| c.id == found.input.category_id).unwrap_or(0));
        self.member.set(member_index(&members, found.input.member_id));
        self.tried.set(false);
        self.open.set(true);
    }

    fn save(&self, store: &Store) {
        self.tried.set(true);
        let flow = if self.flow.get() == 1 { Flow::Income } else { Flow::Expense };
        let accounts = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), flow);
        let members = store.read(|db| db.members());
        let description = self.description.get();
        let (Some(date), false, Ok(amount_cents), Some(account), Some(category)) = (
            self.date.get(),
            description.trim().is_empty(),
            parse_brl(&self.amount.get()).and_then(|c| if c > 0 { Ok(c) } else { Err(String::new()) }),
            accounts.get(self.account.get()),
            categories.get(self.category.get()),
        ) else {
            return;
        };
        let input = TransactionInput {
            date,
            description,
            amount_cents,
            flow,
            account_id: account.id,
            category_id: category.id,
            member_id: member_at(&members, self.member.get()),
        };
        let editing = self.editing.get();
        let saved =
            store.write(if editing.is_some() { "Lançamento alterado" } else { "Lançamento salvo" }, |db| {
                match editing {
                    Some(id) => db.update_transaction(id, &input),
                    None => db.add_transaction(&input).map(|_| ()),
                }
            });
        if saved.is_some() {
            self.open.set(false);
        }
    }

    pub fn view(&self, store: &Store) -> Option<Dialog> {
        if !self.open.get() {
            return None;
        }
        let flow = if self.flow.get() == 1 { Flow::Income } else { Flow::Expense };
        let accounts: Vec<Account> = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), flow);
        let members = store.read(|db| db.members());
        let tried = self.tried.get();
        let category = self.category.clone();
        let flow_binding = {
            let (read, write) = (self.flow.clone(), self.flow.clone());
            // The categories change with the type, so the choice starts over.
            Binding::new(
                move || read.get(),
                move |value| {
                    write.set(value);
                    category.set(0);
                },
            )
        };
        let content = VStack::new((
            SegmentedControl::new(flow_binding, ["Despesa", "Receita"]).label("Tipo"),
            FormField::new(
                "Descrição",
                TextField::new(self.description.binding()).placeholder("Ex.: Supermercado").autofocus(),
            )
            .required()
            .validate(self.description.binding(), validate::required)
            .show_errors(tried),
            HStack::new((
                FormField::new(
                    "Valor",
                    TextField::new(self.amount.binding()).placeholder("0,00").width(190.0),
                )
                .required()
                .validate(self.amount.binding(), validate_positive)
                .show_errors(tried),
                FormField::new(
                    "Data",
                    DatePicker::new(self.date.binding(), store.today)
                        .today(store.today)
                        .label("Data")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            HStack::new((
                FormField::new(
                    "Conta",
                    Select::new(self.account.binding(), accounts.iter().map(|a| a.name.clone()))
                        .label("Conta")
                        .width(190.0),
                ),
                FormField::new(
                    "Categoria",
                    Select::new(self.category.binding(), categories.iter().map(|c| c.name.clone()))
                        .label("Categoria")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            FormField::new(
                "De quem",
                Select::new(self.member.binding(), member_options(&members)).label("De quem").width(190.0),
            ),
        ))
        .spacing(14.0)
        .align(Align::Stretch);

        let (save, save_store, cancel) = (self.clone(), store.clone(), self.open.clone());
        let title = if self.editing.get().is_some() { "Alterar lançamento" } else { "Novo lançamento" };
        Some(
            Dialog::new(self.open.binding(), title)
                .width(440.0)
                .content(content)
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)),
                )
                .action(
                    Button::new("Salvar lançamento")
                        .icon(IconName::Check)
                        .on_click(move || save.save(&save_store)),
                ),
        )
    }
}

/// A new bill, or changing one.
#[derive(Clone)]
pub(crate) struct BillForm {
    open: State<bool>,
    editing: State<Option<i64>>,
    description: State<String>,
    amount: State<String>,
    due: State<Option<Date>>,
    category: State<usize>,
    member: State<usize>,
    recurrence: State<usize>,
    installments: State<i32>,
    barcode: State<String>,
    tried: State<bool>,
}

impl BillForm {
    pub fn new() -> Self {
        Self {
            open: State::new(false),
            editing: State::new(None),
            description: State::new(String::new()),
            amount: State::new(String::new()),
            due: State::new(None),
            category: State::new(0),
            member: State::new(0),
            recurrence: State::new(0),
            installments: State::new(1),
            barcode: State::new(String::new()),
            tried: State::new(false),
        }
    }

    pub fn start_new(&self, store: &Store) {
        self.editing.set(None);
        self.description.set(String::new());
        self.amount.set(String::new());
        self.due.set(Some(store.today));
        self.category.set(0);
        self.member.set(0);
        self.recurrence.set(0);
        self.installments.set(1);
        self.barcode.set(String::new());
        self.tried.set(false);
        self.open.set(true);
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        let Some(bill) = store.read(|db| db.bill(id)) else { return };
        let categories = categories_of(&store.read(|db| db.categories()), Flow::Expense);
        let members = store.read(|db| db.members());
        self.editing.set(Some(id));
        self.description.set(bill.description);
        self.amount.set(format_input(bill.amount_cents));
        self.due.set(Some(bill.due));
        self.category.set(categories.iter().position(|c| c.id == bill.category_id).unwrap_or(0));
        self.member.set(member_index(&members, bill.member_id));
        self.recurrence.set(Recurrence::ALL.iter().position(|r| *r == bill.recurrence).unwrap_or(0));
        self.installments.set(1);
        self.barcode.set(bill.barcode);
        self.tried.set(false);
        self.open.set(true);
    }

    fn save(&self, store: &Store) {
        self.tried.set(true);
        let categories = categories_of(&store.read(|db| db.categories()), Flow::Expense);
        let members = store.read(|db| db.members());
        let description = self.description.get();
        let (Some(due), false, Ok(amount_cents), Some(category)) = (
            self.due.get(),
            description.trim().is_empty(),
            parse_brl(&self.amount.get()).and_then(|c| if c > 0 { Ok(c) } else { Err(String::new()) }),
            categories.get(self.category.get()),
        ) else {
            return;
        };
        let input = BillInput {
            description,
            category_id: category.id,
            member_id: member_at(&members, self.member.get()),
            amount_cents,
            due,
            recurrence: Recurrence::ALL[self.recurrence.get().min(Recurrence::ALL.len() - 1)],
            installments: self.installments.get().max(1) as u32,
            barcode: self.barcode.get().chars().filter(char::is_ascii_digit).collect(),
        };
        let editing = self.editing.get();
        let parts = input.installments;
        let saved = store.write(
            match (editing.is_some(), parts > 1) {
                (true, _) => "Conta alterada",
                (false, true) => "Parcelas criadas",
                (false, false) => "Conta salva",
            },
            |db| match editing {
                Some(id) => db.update_bill(id, &input),
                None => db.add_bill(&input).map(|_| ()),
            },
        );
        if saved.is_some() {
            self.open.set(false);
        }
    }

    pub fn view(&self, store: &Store) -> Option<Dialog> {
        if !self.open.get() {
            return None;
        }
        let categories = categories_of(&store.read(|db| db.categories()), Flow::Expense);
        let members = store.read(|db| db.members());
        let tried = self.tried.get();
        let editing = self.editing.get().is_some();
        let split = self.installments.get() > 1;
        let hint = match parse_brl(&self.amount.get()) {
            Ok(total) if split => {
                let parts = crate::money::split_installments(total, self.installments.get().max(1) as u32);
                format!("{}x de {}", parts.len(), format_brl(parts.first().copied().unwrap_or(0)))
            }
            _ => String::new(),
        };
        let content = VStack::new((
            FormField::new(
                "Descrição",
                TextField::new(self.description.binding()).placeholder("Ex.: Energia elétrica").autofocus(),
            )
            .required()
            .validate(self.description.binding(), validate::required)
            .show_errors(tried),
            HStack::new((
                FormField::new(
                    if split { "Valor total" } else { "Valor" },
                    TextField::new(self.amount.binding()).placeholder("0,00").width(190.0),
                )
                .required()
                .validate(self.amount.binding(), validate_positive)
                .show_errors(tried),
                FormField::new(
                    if split { "Primeiro vencimento" } else { "Vencimento" },
                    DatePicker::new(self.due.binding(), store.today)
                        .today(store.today)
                        .label("Vencimento")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            HStack::new((
                FormField::new(
                    "Categoria",
                    Select::new(self.category.binding(), categories.iter().map(|c| c.name.clone()))
                        .label("Categoria")
                        .width(190.0),
                ),
                FormField::new(
                    "De quem",
                    Select::new(self.member.binding(), member_options(&members))
                        .label("De quem")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            (!split).then(|| {
                FormField::new(
                    "Repetição",
                    SegmentedControl::new(self.recurrence.binding(), Recurrence::ALL.map(Recurrence::label))
                        .label("Repetição"),
                )
            }),
            (!editing).then(|| {
                HStack::new((
                    Stepper::new(self.installments.binding()).range(1, 60).label("Parcelas"),
                    Text::new(if split { hint.clone() } else { "Parcelas: 1, à vista".to_owned() })
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(12.0)
                .align(Align::Center)
            }),
            FormField::new(
                "Código de barras do boleto",
                TextField::new(self.barcode.binding())
                    .placeholder("Só os números; opcional")
                    .icon(IconName::Rows3),
            ),
        ))
        .spacing(14.0)
        .align(Align::Stretch);
        let (save, save_store, cancel) = (self.clone(), store.clone(), self.open.clone());
        Some(
            Dialog::new(self.open.binding(), if editing { "Alterar conta" } else { "Nova conta a pagar" })
                .width(440.0)
                .content(content)
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)),
                )
                .action(
                    Button::new("Salvar conta")
                        .icon(IconName::Check)
                        .on_click(move || save.save(&save_store)),
                ),
        )
    }
}

/// Paying a bill: from which account, when and how much.
#[derive(Clone)]
pub(crate) struct PayForm {
    open: State<bool>,
    bill: State<Option<i64>>,
    account: State<usize>,
    date: State<Option<Date>>,
    amount: State<String>,
    tried: State<bool>,
}

impl PayForm {
    pub fn new() -> Self {
        Self {
            open: State::new(false),
            bill: State::new(None),
            account: State::new(0),
            date: State::new(None),
            amount: State::new(String::new()),
            tried: State::new(false),
        }
    }

    pub fn start(&self, store: &Store, id: i64) {
        let Some(bill) = store.read(|db| db.bill(id)) else { return };
        self.bill.set(Some(id));
        self.account.set(usual_account(&store.read(|db| db.accounts())));
        self.date.set(Some(store.today));
        self.amount.set(format_input(bill.amount_cents));
        self.tried.set(false);
        self.open.set(true);
    }

    fn save(&self, store: &Store) {
        self.tried.set(true);
        let accounts = store.read(|db| db.accounts());
        let (Some(id), Some(date), Ok(amount), Some(account)) = (
            self.bill.get(),
            self.date.get(),
            parse_brl(&self.amount.get()).and_then(|c| if c > 0 { Ok(c) } else { Err(String::new()) }),
            accounts.get(self.account.get()),
        ) else {
            return;
        };
        if store.write("Conta paga", |db| db.pay_bill(id, account.id, date, amount)).is_some() {
            self.open.set(false);
        }
    }

    pub fn view(&self, store: &Store) -> Option<Dialog> {
        if !self.open.get() {
            return None;
        }
        let bill = self.bill.get().and_then(|id| store.read(|db| db.bill(id)))?;
        let accounts = store.read(|db| db.accounts());
        let description = format!("{}, com vencimento em {}.", bill.title(), date_label(bill.due));
        let content = VStack::new((
            FormField::new(
                "Pagar com",
                Select::new(
                    self.account.binding(),
                    accounts.iter().map(|a| format!("{} ({})", a.name, format_brl(a.balance_cents))),
                )
                .label("Pagar com")
                .width(392.0),
            ),
            HStack::new((
                FormField::new("Valor pago", TextField::new(self.amount.binding()).width(190.0))
                    .hint("Mude se houve juros ou desconto.")
                    .validate(self.amount.binding(), validate_positive)
                    .show_errors(self.tried.get()),
                FormField::new(
                    "Pago em",
                    DatePicker::new(self.date.binding(), store.today)
                        .today(store.today)
                        .label("Pago em")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
        ))
        .spacing(14.0)
        .align(Align::Stretch);
        let (save, save_store, cancel) = (self.clone(), store.clone(), self.open.clone());
        Some(
            Dialog::new(self.open.binding(), "Pagar conta")
                .description(description)
                .width(440.0)
                .content(content)
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)),
                )
                .action(
                    Button::new("Confirmar pagamento")
                        .icon(IconName::Check)
                        .on_click(move || save.save(&save_store)),
                ),
        )
    }
}

/// Planning a payment or a receipt on future days.
#[derive(Clone)]
pub(crate) struct ScheduleForm {
    open: State<bool>,
    editing: State<Option<i64>>,
    flow: State<usize>,
    description: State<String>,
    amount: State<String>,
    start: State<Option<Date>>,
    account: State<usize>,
    category: State<usize>,
    member: State<usize>,
    rule: State<RecurrenceRule>,
    tried: State<bool>,
}

impl ScheduleForm {
    pub fn new() -> Self {
        Self {
            open: State::new(false),
            editing: State::new(None),
            flow: State::new(0),
            description: State::new(String::new()),
            amount: State::new(String::new()),
            start: State::new(None),
            account: State::new(0),
            category: State::new(0),
            member: State::new(0),
            rule: State::new(RecurrenceRule::default()),
            tried: State::new(false),
        }
    }

    /// A new plan, starting on this day and for this person.
    pub fn start_new(&self, store: &Store, start: Date, member_id: Option<i64>) {
        let members = store.read(|db| db.members());
        self.editing.set(None);
        self.flow.set(0);
        self.description.set(String::new());
        self.amount.set(String::new());
        self.start.set(Some(start));
        self.account.set(usual_account(&store.read(|db| db.accounts())));
        self.category.set(0);
        self.member.set(member_index(&members, member_id));
        self.rule.set(RecurrenceRule::default());
        self.tried.set(false);
        self.open.set(true);
    }

    pub fn start_edit(&self, store: &Store, id: i64) {
        let Some(found) = store.read(|db| db.schedule(id)) else { return };
        let input = found.input;
        let accounts = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), input.flow);
        let members = store.read(|db| db.members());
        self.editing.set(Some(id));
        self.flow.set(usize::from(input.flow == Flow::Income));
        self.description.set(input.description);
        self.amount.set(format_input(input.amount_cents));
        self.start.set(Some(input.start));
        self.account.set(accounts.iter().position(|a| a.id == input.account_id).unwrap_or(0));
        self.category.set(categories.iter().position(|c| c.id == input.category_id).unwrap_or(0));
        self.member.set(member_index(&members, input.member_id));
        self.rule.set(input.rule);
        self.tried.set(false);
        self.open.set(true);
    }

    fn save(&self, store: &Store) {
        self.tried.set(true);
        let flow = if self.flow.get() == 1 { Flow::Income } else { Flow::Expense };
        let accounts = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), flow);
        let members = store.read(|db| db.members());
        let description = self.description.get();
        let (Some(start), false, Ok(amount_cents), Some(account), Some(category)) = (
            self.start.get(),
            description.trim().is_empty(),
            parse_brl(&self.amount.get()).and_then(|c| if c > 0 { Ok(c) } else { Err(String::new()) }),
            accounts.get(self.account.get()),
            categories.get(self.category.get()),
        ) else {
            return;
        };
        let input = ScheduleInput {
            description,
            flow,
            amount_cents,
            account_id: account.id,
            category_id: category.id,
            member_id: member_at(&members, self.member.get()),
            start,
            rule: self.rule.get(),
        };
        let editing = self.editing.get();
        let saved = store.write("Programação salva", |db| match editing {
            Some(id) => db.update_schedule(id, &input),
            None => db.add_schedule(&input).map(|_| ()),
        });
        if saved.is_some() {
            self.open.set(false);
        }
    }

    pub fn view(&self, store: &Store) -> Option<Dialog> {
        if !self.open.get() {
            return None;
        }
        let flow = if self.flow.get() == 1 { Flow::Income } else { Flow::Expense };
        let accounts = store.read(|db| db.accounts());
        let categories = categories_of(&store.read(|db| db.categories()), flow);
        let members = store.read(|db| db.members());
        let tried = self.tried.get();
        let first = self.start.get().unwrap_or(store.today);
        let category = self.category.clone();
        let flow_binding = {
            let (read, write) = (self.flow.clone(), self.flow.clone());
            // The categories change with the type, so the choice starts over.
            Binding::new(
                move || read.get(),
                move |value| {
                    write.set(value);
                    category.set(0);
                },
            )
        };
        let content = ScrollView::new((VStack::new((
            SegmentedControl::new(flow_binding, ["Pagamento", "Recebimento"]).label("Tipo"),
            FormField::new(
                "Descrição",
                TextField::new(self.description.binding()).placeholder("Ex.: Aluguel da garagem").autofocus(),
            )
            .required()
            .validate(self.description.binding(), validate::required)
            .show_errors(tried),
            HStack::new((
                FormField::new(
                    "Valor",
                    TextField::new(self.amount.binding()).placeholder("0,00").width(190.0),
                )
                .required()
                .validate(self.amount.binding(), validate_positive)
                .show_errors(tried),
                FormField::new(
                    "Primeiro dia",
                    DatePicker::new(self.start.binding(), first)
                        .today(store.today)
                        .label("Primeiro dia")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            HStack::new((
                FormField::new(
                    "Conta",
                    Select::new(self.account.binding(), accounts.iter().map(|a| a.name.clone()))
                        .label("Conta")
                        .width(190.0),
                ),
                FormField::new(
                    "Categoria",
                    Select::new(self.category.binding(), categories.iter().map(|c| c.name.clone()))
                        .label("Categoria")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
            FormField::new(
                "De quem",
                Select::new(self.member.binding(), member_options(&members)).label("De quem").width(190.0),
            ),
            FormField::new("Repetição", RecurrenceEditor::new(self.rule.binding(), first)),
        ))
        .spacing(12.0)
        .align(Align::Stretch),))
        .height(540.0);
        let (save, save_store, cancel) = (self.clone(), store.clone(), self.open.clone());
        let title = if self.editing.get().is_some() { "Alterar programação" } else { "Programar" };
        Some(
            Dialog::new(self.open.binding(), title)
                .width(480.0)
                .content(content)
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)),
                )
                .action(
                    Button::new("Salvar programação")
                        .icon(IconName::Check)
                        .on_click(move || save.save(&save_store)),
                ),
        )
    }
}

/// Saying a planned day happened: from which account, when and how much.
#[derive(Clone)]
pub(crate) struct ConfirmForm {
    open: State<bool>,
    target: State<Option<(i64, Date)>>,
    account: State<usize>,
    date: State<Option<Date>>,
    amount: State<String>,
    tried: State<bool>,
}

impl ConfirmForm {
    pub fn new() -> Self {
        Self {
            open: State::new(false),
            target: State::new(None),
            account: State::new(0),
            date: State::new(None),
            amount: State::new(String::new()),
            tried: State::new(false),
        }
    }

    pub fn start(&self, store: &Store, occurrence: &Occurrence) {
        let accounts = store.read(|db| db.accounts());
        let input = &occurrence.schedule.input;
        self.target.set(Some((occurrence.schedule_id, occurrence.day)));
        self.account.set(accounts.iter().position(|a| a.id == input.account_id).unwrap_or(0));
        // Something confirmed ahead of its day happened today.
        self.date.set(Some(occurrence.when.min(store.today)));
        self.amount.set(format_input(input.amount_cents));
        self.tried.set(false);
        self.open.set(true);
    }

    fn save(&self, store: &Store) {
        self.tried.set(true);
        let accounts = store.read(|db| db.accounts());
        let (Some((id, day)), Some(when), Ok(amount), Some(account)) = (
            self.target.get(),
            self.date.get(),
            parse_brl(&self.amount.get()).and_then(|c| if c > 0 { Ok(c) } else { Err(String::new()) }),
            accounts.get(self.account.get()),
        ) else {
            return;
        };
        if store.write("Confirmado", |db| db.confirm_occurrence(id, day, account.id, when, amount)).is_some()
        {
            self.open.set(false);
        }
    }

    pub fn view(&self, store: &Store) -> Option<Dialog> {
        if !self.open.get() {
            return None;
        }
        let (id, day) = self.target.get()?;
        let schedule = store.read(|db| db.schedule(id))?;
        let accounts = store.read(|db| db.accounts());
        let verb =
            if schedule.input.flow == Flow::Income { "Confirmar recebimento" } else { "Confirmar pagamento" };
        let content = VStack::new((
            FormField::new(
                "Conta",
                Select::new(self.account.binding(), accounts.iter().map(|a| a.name.clone()))
                    .label("Conta")
                    .width(392.0),
            ),
            HStack::new((
                FormField::new("Valor", TextField::new(self.amount.binding()).width(190.0))
                    .validate(self.amount.binding(), validate_positive)
                    .show_errors(self.tried.get()),
                FormField::new(
                    "Quando",
                    DatePicker::new(self.date.binding(), store.today)
                        .today(store.today)
                        .label("Quando")
                        .width(190.0),
                ),
            ))
            .spacing(12.0),
        ))
        .spacing(14.0)
        .align(Align::Stretch);
        let (save, save_store, cancel) = (self.clone(), store.clone(), self.open.clone());
        Some(
            Dialog::new(self.open.binding(), verb)
                .description(format!("{}, programado para {}.", schedule.input.description, date_label(day)))
                .width(440.0)
                .content(content)
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(false)),
                )
                .action(Button::new(verb).icon(IconName::Check).on_click(move || save.save(&save_store))),
        )
    }
}

/// A question before doing something that cannot be undone.
pub(crate) struct Confirmation {
    pub title: String,
    pub message: String,
    pub action: String,
    pub run: Rc<dyn Fn()>,
}

/// Holds the question being asked.
#[derive(Clone)]
pub(crate) struct Confirm(State<Option<Rc<Confirmation>>>);

impl Confirm {
    pub fn new() -> Self {
        Self(State::new(None))
    }

    pub fn ask(&self, title: &str, message: &str, action: &str, run: impl Fn() + 'static) {
        self.0.set(Some(Rc::new(Confirmation {
            title: title.to_owned(),
            message: message.to_owned(),
            action: action.to_owned(),
            run: Rc::new(run),
        })));
    }

    pub fn view(&self) -> Option<Dialog> {
        let question = self.0.get()?;
        let (read, write) = (self.0.clone(), self.0.clone());
        let open = Binding::new(
            move || read.get().is_some(),
            move |open: bool| {
                if !open {
                    write.set(None);
                }
            },
        );
        let (cancel, done) = (self.0.clone(), self.0.clone());
        let run = question.run.clone();
        Some(
            Dialog::new(open, question.title.clone())
                .description(question.message.clone())
                .action(
                    Button::new("Cancelar").style(ButtonStyle::Secondary).on_click(move || cancel.set(None)),
                )
                .action(Button::new(question.action.clone()).style(ButtonStyle::Danger).on_click(
                    move || {
                        done.set(None);
                        run();
                    },
                )),
        )
    }
}

/// All the dialogs, for the screens that open them.
#[derive(Clone)]
pub(crate) struct Forms {
    pub transaction: TransactionForm,
    pub bill: BillForm,
    pub pay: PayForm,
    pub schedule: ScheduleForm,
    pub occurrence: ConfirmForm,
    pub confirm: Confirm,
}

impl Forms {
    pub fn new() -> Self {
        Self {
            transaction: TransactionForm::new(),
            bill: BillForm::new(),
            pay: PayForm::new(),
            schedule: ScheduleForm::new(),
            occurrence: ConfirmForm::new(),
            confirm: Confirm::new(),
        }
    }
}
