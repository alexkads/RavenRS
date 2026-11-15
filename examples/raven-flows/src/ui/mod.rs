//! The screens: the list of flows, the editor, the runs and the settings, inside a sidebar,
//! with the version and the last message in the status bar.

use std::collections::BTreeSet;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_flows::Catalog;

use crate::casa::{self, Casa};
use ravenrs_app_update::{News, NoticeAction, UpdateStatus, Updater};

mod editor;
mod execucoes;

pub use editor::apply_parameters;
use editor::open_flow;

/// A cell of a grid, apart from the JSON of a node's setup.
type Cell = ravenrs::prelude::Value;

/// The pages of the sidebar.
const FLOWS: usize = 0;
const RUNS: usize = 1;
const SETTINGS: usize = 2;

/// What the screens keep between builds.
struct Screens {
    page: State<usize>,
    /// The flow open in the editor.
    open: State<Option<String>>,
    graph: State<FlowGraph>,
    history: State<FlowHistory>,
    selection: State<FlowSelection>,
    view: State<FlowViewport>,
    arrivals: State<Option<FlowNode>>,
    /// The parameters of the selected node, as typed.
    parameters: State<String>,
    /// The node whose parameters are in `parameters`.
    parameters_of: State<Option<String>>,
    /// The name of the next flow.
    name: State<String>,
    flow_keys: State<BTreeSet<String>>,
    run_keys: State<BTreeSet<String>>,
    /// The diagram of the run open in Execuções, and which run it is.
    run_graph: State<FlowGraph>,
    run_shown: State<Option<String>>,
    days: State<Option<f64>>,
    news: State<usize>,
    message: State<String>,
    confirm: ConfirmDialog,
}

/// The whole app.
pub struct RavenFlows {
    casa: Rc<Casa>,
    /// Looks for new versions, installs them and keeps every version's news.
    updater: Updater,
    catalog: Rc<Catalog>,
    s: Rc<Screens>,
}

impl std::fmt::Debug for RavenFlows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RavenFlows").field("casa", &self.casa).finish_non_exhaustive()
    }
}

impl RavenFlows {
    /// The app over `casa`.
    pub fn new(casa: Rc<Casa>, updater: Updater) -> Self {
        let s = Screens {
            page: State::new(FLOWS),
            open: State::new(None),
            graph: State::new(FlowGraph::new()),
            history: State::new(FlowHistory::new()),
            selection: State::new(FlowSelection::default()),
            view: State::new(FlowViewport::fit()),
            arrivals: State::new(None),
            parameters: State::new(String::new()),
            parameters_of: State::new(None),
            name: State::new(String::new()),
            flow_keys: State::new(BTreeSet::new()),
            run_keys: State::new(BTreeSet::new()),
            run_graph: State::new(FlowGraph::new()),
            run_shown: State::new(None),
            days: State::new(Some(30.0)),
            news: State::new(0),
            message: State::new(String::new()),
            confirm: ConfirmDialog::new(),
        };
        Self { casa, updater, catalog: Rc::new(Catalog::builtin()), s: Rc::new(s) }
    }
}

impl View for RavenFlows {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.casa.start(cx.ui_waker());
        // The updater's threads wake the screen when they have something to say.
        let waker = cx.ui_waker();
        self.updater.connect(move || waker.wake());
        self.updater.poll();
        let s = &self.s;
        let page = s.page.get();
        let item = |icon, label: &str, at: usize| {
            let p = s.page.clone();
            let open = s.open.clone();
            SidebarItem::new(icon, label).selected(page == at).on_click(move || {
                p.set(at);
                if at != FLOWS {
                    open.set(None);
                }
            })
        };
        let waiting =
            self.casa.runs().iter().filter(|r| r.status == ravenrs_flows_runtime::RunStatus::Waiting).count();
        let mut runs_item = item(IconName::History, "Execuções", RUNS);
        if waiting > 0 {
            runs_item = runs_item.count_spoken(waiting.to_string(), format!("{waiting} aguardando"));
        }
        let sidebar = Sidebar::new((SidebarGroup::new(
            "Automação",
            (
                item(IconName::GitBranch, "Fluxos", FLOWS),
                runs_item,
                item(IconName::Settings, "Configurações", SETTINGS),
            ),
        ),))
        .label("Navegação")
        .header(SidebarBrand::new(Icon::new(IconName::GitBranch).color(Semantic::Accent), "Raven Flows"));

        let content: AnyView = match (page, s.open.get()) {
            (FLOWS, Some(id)) => self.editor(cx, &id),
            (FLOWS, None) => self.flows(),
            (RUNS, _) => self.runs(),
            _ => self.settings(),
        };
        let status = self.status_bar();
        cx.child(
            &VStack::new((
                HStack::new((sidebar, VStack::new((content,)).align(Align::Stretch).grow(1.0)))
                    .align(Align::Stretch)
                    .grow(1.0),
                status,
                s.confirm.clone(),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

impl RavenFlows {
    /// The list of flows.
    fn flows(&self) -> AnyView {
        let s = self.s.clone();
        let now = casa::now_ms();
        let rows: Vec<Vec<Cell>> = self
            .casa
            .flows()
            .iter()
            .map(|f| {
                let last = match self.casa.last_run(&f.id) {
                    Some(r) => format!("{} {}", casa::run_words(r.status), casa::ago(r.created_ms, now)),
                    None => "Nunca executado".to_owned(),
                };
                vec![
                    f.id.clone().into(),
                    f.name.clone().into(),
                    (f.nodes.len() as f64).into(),
                    (f64::from(f.revision)).into(),
                    last.into(),
                ]
            })
            .collect();
        let (casa, catalog) = (self.casa.clone(), self.catalog.clone());
        let opener = {
            let (casa, catalog, s) = (casa.clone(), catalog.clone(), s.clone());
            move |row: &Vec<Cell>| open_flow(&casa, &catalog, &s, &row[0].key_text())
        };
        let create = {
            let (casa, catalog, s) = (casa.clone(), catalog.clone(), s.clone());
            move || {
                let name = s.name.get();
                let name =
                    if name.trim().is_empty() { "Novo fluxo".to_owned() } else { name.trim().to_owned() };
                match casa.create(&name) {
                    Ok(flow) => {
                        s.name.set(String::new());
                        s.message.set(format!("Fluxo {} criado.", flow.name));
                        open_flow(&casa, &catalog, &s, &flow.id);
                    }
                    Err(why) => s.message.set(why),
                }
            }
        };
        let open_chosen = {
            let (casa, catalog, s) = (casa.clone(), catalog.clone(), s.clone());
            move || match s.flow_keys.get().into_iter().next() {
                Some(id) => open_flow(&casa, &catalog, &s, &id),
                None => s.message.set("Escolha na lista o fluxo a abrir.".to_owned()),
            }
        };
        let delete = {
            let (casa, s) = (casa.clone(), s.clone());
            move || {
                let keys = s.flow_keys.get();
                if keys.is_empty() {
                    s.message.set("Escolha na lista o fluxo a excluir.".to_owned());
                    return;
                }
                let (casa, s2) = (casa.clone(), s.clone());
                let names: Vec<String> = keys.iter().filter_map(|k| casa.flow(k).map(|f| f.name)).collect();
                s.confirm.ask(
                    "Excluir fluxo?",
                    format!("{} sai da lista. As execuções ficam até a limpeza.", names.join(", ")),
                    "Excluir",
                    move || {
                        for key in s2.flow_keys.get() {
                            if let Err(why) = casa.delete(&key) {
                                s2.message.set(why);
                                return;
                            }
                        }
                        s2.flow_keys.set(BTreeSet::new());
                        s2.message.set("Fluxo excluído.".to_owned());
                    },
                );
            }
        };
        Box::new(
            VStack::new((
                PageHeader::new("Fluxos").description("Cada fluxo é um arquivo em ~/.raven-flows/fluxos."),
                HStack::new((
                    TextField::new(s.name.binding()).placeholder("Nome do novo fluxo"),
                    Button::new("Criar fluxo").icon(IconName::Plus).on_click(create),
                    Spacer::new(),
                    Button::new("Abrir")
                        .icon(IconName::FileText)
                        .style(ButtonStyle::Secondary)
                        .on_click(open_chosen),
                    Button::new("Excluir").style(ButtonStyle::Secondary).on_click(delete),
                ))
                .spacing(8.0)
                .align(Align::Center),
                DataGrid::new(
                    GridData::new(rows),
                    [
                        GridColumn::text("Id").hidden(),
                        GridColumn::text("Nome").fill(),
                        GridColumn::number("Nós").width(90.0),
                        GridColumn::number("Revisão").width(110.0),
                        GridColumn::text("Última execução").width(260.0),
                    ],
                )
                .key_column(0)
                .selected_keys(s.flow_keys.binding())
                .empty_text("Nenhum fluxo ainda. Dê um nome e crie o primeiro.")
                .on_row_open(opener)
                .grow(1.0),
            ))
            .spacing(16.0)
            .padding(24.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }

    /// The settings: theme, how long runs are kept, the version and its news.
    fn settings(&self) -> AnyView {
        let s = self.s.clone();
        let casa = self.casa.clone();
        let prune = {
            let s = s.clone();
            move || {
                let days = s.days.get().unwrap_or(30.0).max(0.0) as u64;
                match casa.prune(days) {
                    Ok(n) => s.message.set(format!("{n} execuções antigas removidas.")),
                    Err(why) => s.message.set(why),
                }
            }
        };
        Box::new(
            VStack::new((
                PageHeader::new("Configurações").description("Valem para este computador."),
                HStack::new((Text::new("Tema"), Spacer::new(), ThemeSwitcher::new())).align(Align::Center),
                HStack::new((
                    Text::new("Guardar execuções por (dias)"),
                    Spacer::new(),
                    NumberField::new(s.days.binding()).range(0.0, 3650.0).label("Dias"),
                    Button::new("Limpar agora").style(ButtonStyle::Secondary).on_click(prune),
                ))
                .spacing(8.0)
                .align(Align::Center),
                HStack::new((
                    Text::new(format!("Versão {}", crate::VERSION)).style(TextStyle::Headline),
                    Spacer::new(),
                    Button::new("Procurar atualização")
                        .icon(IconName::RotateCcw)
                        .style(ButtonStyle::Secondary)
                        .on_click({
                            let updater = self.updater.clone();
                            move || updater.ask()
                        }),
                ))
                .spacing(8.0)
                .align(Align::Center),
                self.failure(),
                ReleaseNotes::new(release_notes(self.updater.timeline()), s.news.binding()).height(360.0),
            ))
            .spacing(16.0)
            .padding(24.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

/// The news of every version, newest first, in the format Raven Finanças keeps them:
/// what changed, who made it and the issues it closed.
pub fn release_notes(timeline: Vec<News>) -> Vec<ReleaseNote> {
    timeline
        .into_iter()
        .map(|news| {
            let issues: Vec<String> = news.issues.iter().map(|n| format!("#{n}")).collect();
            ReleaseNote::new(news.versao, news.titulo)
                .date(news.data.unwrap_or_default())
                .changes(news.novidades)
                .section("Quem contribuiu", news.contribuidores.join(", "))
                .section("Issues resolvidas", issues.join(", "))
        })
        .collect()
}

impl RavenFlows {
    /// The status bar: the version, what the updater says with its buttons, and the last
    /// message of the app.
    fn status_bar(&self) -> StatusBar {
        let page = self.s.page.clone();
        let bar = StatusBar::new()
            .left((StatusBar::action(format!("Raven Flows {}", crate::VERSION))
                .on_click(move || page.set(SETTINGS)),))
            .right((StatusBar::text(self.s.message.get()).truncate(),));
        let Some(notice) = self.updater.notice() else { return bar };
        let buttons: Vec<Button> = notice.actions.iter().map(|&action| self.notice_button(action)).collect();
        let bar = bar.center((
            StatusBar::text(notice.text).foreground(Semantic::Ink).truncate().grow(1.0),
            HStack::new(buttons).spacing(4.0),
        ));
        if notice.important { bar.center_tone(Tone::Warning) } else { bar }
    }

    /// A button of the updater's notice, doing what Raven Finanças does with it.
    fn notice_button(&self, action: NoticeAction) -> Button {
        let (updater, page) = (self.updater.clone(), self.s.page.clone());
        let button = StatusBar::action(action.label());
        let button = if action.is_primary() { button.style(ButtonStyle::Primary) } else { button };
        button.on_click(move || match action {
            NoticeAction::News | NoticeAction::HowToUpdate | NoticeAction::Details => page.set(SETTINGS),
            NoticeAction::Update | NoticeAction::Retry => {
                if let Some(news) = updater.status().news().cloned() {
                    updater.install(news);
                }
            }
            NoticeAction::Reopen => updater.reopen(),
            NoticeAction::CheckAgain => updater.ask(),
            NoticeAction::Later | NoticeAction::Close => updater.dismiss(),
        })
    }

    /// Why the last update failed, where it stopped, the end of the installer's log and how
    /// to install by hand; nothing when it did not fail.
    fn failure(&self) -> AnyView {
        let UpdateStatus::Failed(_, reason) = self.updater.status() else {
            return Box::new(Spacer::new().height(0.0));
        };
        let stage = self.updater.stage().map(|s| format!(" Parou em: {s}.")).unwrap_or_default();
        let tail = crate::versao::APP.installer_log_tail(12).unwrap_or_default();
        Box::new(
            VStack::new((
                Alert::new(
                    "A atualização não deu certo",
                    format!("{reason}.{stage} A versão em uso continua funcionando."),
                )
                .tone(Tone::Danger),
                CodeEditor::new(State::new(tail).binding())
                    .language(CodeLanguage::Plain)
                    .read_only(true)
                    .rows(8),
                Text::new(format!("Para instalar à mão: {}", crate::versao::APP.install_command()))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
            ))
            .spacing(8.0)
            .align(Align::Stretch),
        )
    }
}
