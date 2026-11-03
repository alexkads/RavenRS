//! The registers: accounts, categories and family members, generated from what they are.

use std::cell::OnceCell;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, ModelError, Object, Rule, Value as Field};
use ravenrs_app_sqlite::{SqlWords, SqliteDatabase, TableMap};
use ravenrs_app_ui::{EntityPane, PaneWords, Words};

use super::help;
use super::store::{PAGE_INSETS, Page, Store};
use crate::model::{AccountKind, Flow};

/// What this screen keeps while another one is open.
#[derive(Clone)]
pub(crate) struct RecordsState {
    pub(crate) tab: State<usize>,
    panes: Rc<OnceCell<Option<Rc<Panes>>>>,
}

impl RecordsState {
    pub fn new() -> Self {
        Self { tab: State::new(0), panes: Rc::default() }
    }
}

struct Panes {
    accounts: EntityPane,
    categories: EntityPane,
    members: EntityPane,
}

fn options<T: Copy>(all: &[T], code: fn(T) -> &'static str, label: fn(T) -> &'static str) -> FieldKind {
    FieldKind::Enum(all.iter().map(|item| EnumOption::new(code(*item), label(*item))).collect())
}

fn name_field(placeholder: &str) -> FieldInfo {
    FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property().placeholder(placeholder)
}

fn id_field() -> FieldInfo {
    FieldInfo::new("id", FieldKind::Integer).key()
}

/// The table has no empty amounts, and people can leave one empty.
fn empty_is_zero(object: &mut Object, field: &str) {
    if object.get(field) == Field::Null {
        object.set(field, 0.0);
    }
}

fn trimmed_name(object: &mut Object) {
    if let Field::Text(name) = object.get("name") {
        object.set("name", name.trim());
    }
}

fn account_entity() -> EntityInfo {
    EntityInfo::new("Account")
        .caption("Contas")
        .field(id_field())
        .field(name_field("Ex.: Banco do bairro"))
        .field(
            FieldInfo::new("kind", options(&AccountKind::ALL, AccountKind::code, AccountKind::label))
                .caption("Tipo"),
        )
        .field(
            FieldInfo::new("opening", FieldKind::Money)
                .caption("Saldo inicial")
                .in_list(false)
                .placeholder("0,00"),
        )
        .field(FieldInfo::new("balance", FieldKind::Money).caption("Saldo").read_only().in_detail(false))
        .rule(Rule::required("name"))
}

fn category_entity() -> EntityInfo {
    EntityInfo::new("Category")
        .caption("Categorias")
        .field(id_field())
        .field(name_field("Ex.: Pets"))
        .field(
            FieldInfo::new("flow", options(&[Flow::Expense, Flow::Income], Flow::code, Flow::label))
                .caption("Tipo")
                .editor("Segmented"),
        )
        .field(FieldInfo::new("budget", FieldKind::Money).caption("Limite mensal").placeholder("Sem limite"))
        .rule(Rule::required("name"))
}

fn member_entity() -> EntityInfo {
    EntityInfo::new("Member")
        .caption("Família")
        .field(id_field())
        .field(name_field("Ex.: Maria"))
        .rule(Rule::required("name"))
}

fn sql_words() -> SqlWords {
    let mut words = SqlWords::pt_br();
    // The same words as the rest of the app, which come from `DbError`.
    words.unique = "Já existe um registro com esse nome. Escolha outro.".to_owned();
    words.in_use = "Este registro está em uso. Remova antes o que depende dele.".to_owned();
    words.rule = "Os dados não passaram nas regras do banco. Revise o formulário.".to_owned();
    words
}

fn panes(store: &Store) -> Result<Panes, ModelError> {
    // `Database` keeps owning the schema: these tables are only mapped, and migrations stay there.
    let balance = "accounts.opening_cents + COALESCE((SELECT SUM(CASE t.flow WHEN 'income' THEN t.amount_cents \
                   ELSE -t.amount_cents END) FROM transactions t WHERE t.account_id = accounts.id), 0)";
    let database = SqliteDatabase::builder(store.db.connection())
        .words(sql_words())
        .mapped(
            account_entity(),
            TableMap::new("accounts")
                .column("opening", "opening_cents")
                .computed("balance", balance)
                .before_save(|object| {
                    trimmed_name(object);
                    empty_is_zero(object, "opening");
                }),
        )?
        .mapped(
            category_entity(),
            TableMap::new("categories")
                .column("budget", "budget_cents")
                .before_save(|object| {
                    trimmed_name(object);
                    empty_is_zero(object, "budget");
                })
                // No limit reads as an empty field, and not as zero reais.
                .after_load(|object| {
                    if object.get("budget") == Field::Number(0.0) {
                        object.set("budget", Field::Null);
                    }
                }),
        )?
        .mapped(member_entity(), TableMap::new("members").before_save(trimmed_name))?
        .build();
    let pane = |entity: EntityInfo, words: PaneWords, saved: &'static str, deleted: &'static str| {
        let (source, told, failed, seen) = (database.clone(), store.clone(), store.clone(), store.clone());
        EntityPane::new(entity, move || source.space(), Words::pt_br(), words).map(|pane| {
            pane.on_done(move |what| {
                told.write(if what == "saved" { saved } else { deleted }, |_| Ok(()));
            })
            .on_failed(move |message| {
                failed.toasts.show_for("Não foi possível salvar", message, Tone::Danger, 8.0)
            })
            .version(move || seen.version())
        })
    };
    Ok(Panes {
        accounts: pane(
            account_entity(),
            PaneWords::new(
                "Nova conta",
                "Alterar conta",
                "Adicionar conta",
                "Excluir esta conta?",
                "Só é possível se ela não tiver lançamentos.",
                "Excluir",
            ),
            "Conta salva",
            "Conta excluída",
        )?
        // An account named after its kind has nothing more to say.
        .cell_text(|field, account| {
            let kind = AccountKind::ALL
                .iter()
                .find(|kind| account.get("kind") == Field::Text(kind.code().to_owned()))?;
            (field == "kind" && account.get("name") == Field::Text(kind.label().to_owned())).then(String::new)
        }),
        categories: pane(
            category_entity(),
            PaneWords::new(
                "Nova categoria",
                "Alterar categoria",
                "Adicionar categoria",
                "Excluir esta categoria?",
                "Só é possível se nada estiver usando.",
                "Excluir",
            ),
            "Categoria salva",
            "Categoria excluída",
        )?,
        members: pane(
            member_entity(),
            PaneWords::new(
                "Nova pessoa",
                "Alterar pessoa",
                "Adicionar pessoa",
                "Remover esta pessoa?",
                "Os lançamentos dela continuam, sem responsável.",
                "Remover",
            ),
            "Pessoa salva",
            "Pessoa removida",
        )?,
    })
}

/// A pane shown by more than one tab build, since a tab takes what it shows.
struct Shared(Rc<Panes>, fn(&Panes) -> &EntityPane);

impl View for Shared {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child((self.1)(&self.0))
    }
}

pub(crate) fn records(store: &Store, state: &RecordsState) -> Page {
    store.version();
    let made = state.panes.get_or_init(|| {
        panes(store)
            .map_err(|e| tracing::error!(error = %e, "the registers could not be put together"))
            .ok()
            .map(Rc::new)
    });
    let tabs: AnyView = match made {
        Some(panes) => Box::new(
            Tabs::new(state.tab.binding())
                .tab("Contas", Shared(panes.clone(), |p| &p.accounts))
                .tab("Categorias", Shared(panes.clone(), |p| &p.categories))
                .tab("Família", Shared(panes.clone(), |p| &p.members)),
        ),
        None => Box::new(Alert::new("Cadastros", "Não foi possível abrir os cadastros.").tone(Tone::Danger)),
    };
    Page::new(
        help::CADASTROS,
        "As contas, as categorias e as pessoas que os lançamentos usam.",
        ScrollView::new((VStack::new((tabs,)).spacing(16.0).align(Align::Stretch),)).padding(PAGE_INSETS),
    )
}
