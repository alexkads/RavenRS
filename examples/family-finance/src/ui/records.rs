//! The registers: accounts, categories and family members, generated from what they are.

use std::cell::OnceCell;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{EntityInfo, ModelError, Value as Field};
use ravenrs_app_ui::{EntityPane, PaneWords, Words};

use super::entities::{self, account_entity, category_entity, member_entity};
use super::help;
use super::store::{PAGE_INSETS, Page, Store};
use crate::model::AccountKind;

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

fn panes(store: &Store) -> Result<Panes, ModelError> {
    let database =
        entities::database(store).ok_or_else(|| ModelError("the records are not there".to_owned()))?;
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
