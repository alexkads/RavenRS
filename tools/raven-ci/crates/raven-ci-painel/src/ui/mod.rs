//! The shell of the panel (P-00, #884): signing in, the sidebar with the pages of the core and the
//! ones the plugins on put in, the status bar, and what keeps the screens up to date: the answers
//! of the server and the events of the live channel.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app_remote::{LiveFeed, LiveState, RemoteClient, SignInError};
use serde_json::{Value as Json, json};

mod admin;
mod build;
mod casca;
mod config;
mod fila;
mod historico;
mod jobs;
mod plugin;

/// The role that administers the server.
/// The margins of a page beside the sidebar, as the boards draw them: 24 above, 32 on the sides
/// and below (T1).
const PAGE_PADDING: Insets = Insets { top: 24.0, right: 32.0, bottom: 32.0, left: 32.0 };

const ADMIN: &str = "Administradores";
/// The role that runs builds.
const RUN: &str = "Executores";

/// A page of the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pagina {
    /// The jobs (T1).
    Jobs,
    /// Every build, the newest first.
    Historico,
    /// A build: its job and number (T2).
    Build(String, u64),
    /// The queue and the executors (T3).
    Fila,
    /// The pipeline of a job (T5).
    Configuracao,
    /// The plugins (T4).
    Plugins,
    /// The credentials.
    Credenciais,
    /// People and their roles.
    Usuarios,
    /// A page a plugin put in, by its name.
    Plugin(String),
}

/// Everything the screens keep between builds. Clones of `State` share their value.
pub(crate) struct S {
    client: RemoteClient,
    live_wanted: bool,
    page: State<Pagina>,
    // Signing in.
    name: State<String>,
    password: State<String>,
    sign_in_error: State<String>,
    signed_in: State<bool>,
    roles: State<Vec<String>>,
    me: State<String>,
    // What the server said.
    server: State<Json>,
    screens: State<Json>,
    overview: State<Json>,
    occupancy: State<Json>,
    plugins: State<Json>,
    credentials: State<Json>,
    users: State<Json>,
    audit: State<Json>,
    build: State<Json>,
    /// The explanation of a failure asked on the page of a build (#942): `{job, number, state,
    /// texto, modelo, ms, message}`, `state` being `gerando`, `pronta` or `erro`.
    explanation: State<Json>,
    plugin_data: State<BTreeMap<String, Json>>,
    history: State<Json>,
    // How wide the window is, read at each build: the pages lay out by it (I7).
    width: Cell<f32>,
    sidebar: SidebarState,
    palette_open: State<bool>,
    message: State<String>,
    // The live channel, and when the log was last read without it.
    live: RefCell<Option<LiveFeed>>,
    waker: RefCell<Option<UiWaker>>,
    polled_at: Cell<f64>,
    confirm: ConfirmDialog,
    // The jobs (P-01).
    job_keys: State<BTreeSet<String>>,
    dispatch_open: State<bool>,
    dispatch_job: State<String>,
    dispatch_values: State<ParamValues>,
    dispatch_error: State<String>,
    // A build (P-02).
    log: LogBuffer,
    log_of: RefCell<Option<(String, u64)>>,
    log_seq: Cell<u64>,
    log_section: RefCell<String>,
    log_sections: RefCell<BTreeMap<String, usize>>,
    stage_pick: State<FlowSelection>,
    stage_picked: RefCell<Vec<String>>,
    stage_of_step: RefCell<BTreeMap<String, String>>,
    build_tab: State<usize>,
    stage_only: State<Option<String>>,
    graph: State<FlowGraph>,
    graph_of: RefCell<String>,
    // The queue (P-03).
    selected_event: State<Option<u64>>,
    agent_dialog: State<bool>,
    agent_name: State<String>,
    agent_labels: State<String>,
    agent_token: State<String>,
    // The pipeline (P-04).
    config_job: State<Option<String>>,
    config_new: State<bool>,
    config_name: State<String>,
    config_pick: State<usize>,
    jobs_full: State<Json>,
    config_tab: State<usize>,
    config_text: State<String>,
    config_checked: RefCell<String>,
    config_errors: State<Vec<(usize, String)>>,
    jenkinsfile: State<String>,
    jenkinsfile_done: RefCell<String>,
    conversion: State<Json>,
    jenkinsfile_caret: State<Cursor>,
    jenkinsfile_focus: State<bool>,
    // Plugins and people (P-05).
    plugin_filter: State<usize>,
    plugin_query: State<String>,
    plugin_settings: State<ParamValues>,
    add_plugin_open: State<bool>,
    plugin_open: State<Option<String>>,
    sheet_open: State<bool>,
    admin_tab: State<usize>,
    // The form of a credential (#922): its name, kind, description and the values of its kind.
    cred_name: State<String>,
    cred_kind: State<usize>,
    cred_description: State<String>,
    cred_first: State<String>,
    cred_second: State<String>,
    new_user: State<String>,
    new_password: State<String>,
    new_role: State<usize>,
}

/// The whole panel.
pub struct Painel {
    s: Rc<S>,
}

impl std::fmt::Debug for Painel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Painel").field("client", &self.s.client).finish_non_exhaustive()
    }
}

impl Painel {
    /// The panel over `client`; with `live`, it listens to the live channel once signed in.
    pub fn new(client: RemoteClient, live: bool) -> Self {
        let s = S {
            client,
            live_wanted: live,
            page: State::new(Pagina::Jobs),
            name: State::new(String::new()),
            password: State::new(String::new()),
            sign_in_error: State::new(String::new()),
            signed_in: State::new(false),
            roles: State::new(Vec::new()),
            me: State::new(String::new()),
            server: State::new(Json::Null),
            screens: State::new(json!([])),
            overview: State::new(Json::Null),
            occupancy: State::new(Json::Null),
            plugins: State::new(json!([])),
            credentials: State::new(json!([])),
            users: State::new(json!([])),
            audit: State::new(json!([])),
            build: State::new(Json::Null),
            explanation: State::new(Json::Null),
            plugin_data: State::new(BTreeMap::new()),
            history: State::new(Json::Null),
            width: Cell::new(1440.0),
            sidebar: SidebarState::new(true),
            palette_open: State::new(false),
            message: State::new(String::new()),
            live: RefCell::new(None),
            waker: RefCell::new(None),
            polled_at: Cell::new(0.0),
            confirm: ConfirmDialog::new(),
            job_keys: State::new(BTreeSet::new()),
            dispatch_open: State::new(false),
            dispatch_job: State::new(String::new()),
            dispatch_values: State::new(ParamValues::new()),
            dispatch_error: State::new(String::new()),
            log: LogBuffer::new(),
            log_of: RefCell::new(None),
            log_seq: Cell::new(0),
            log_section: RefCell::new(String::new()),
            log_sections: RefCell::new(BTreeMap::new()),
            stage_pick: State::new(FlowSelection::default()),
            stage_picked: RefCell::new(Vec::new()),
            stage_of_step: RefCell::new(BTreeMap::new()),
            build_tab: State::new(0),
            stage_only: State::new(None),
            graph: State::new(FlowGraph::new()),
            graph_of: RefCell::new(String::new()),
            selected_event: State::new(None),
            agent_dialog: State::new(false),
            agent_name: State::new(String::new()),
            agent_labels: State::new(String::new()),
            agent_token: State::new(String::new()),
            config_job: State::new(None),
            config_new: State::new(false),
            config_name: State::new(String::new()),
            config_pick: State::new(0),
            jobs_full: State::new(json!([])),
            config_tab: State::new(0),
            config_text: State::new(String::new()),
            config_checked: RefCell::new(String::new()),
            config_errors: State::new(Vec::new()),
            jenkinsfile: State::new(String::new()),
            jenkinsfile_done: RefCell::new(String::new()),
            conversion: State::new(Json::Null),
            jenkinsfile_caret: State::new(Cursor::at(0)),
            jenkinsfile_focus: State::new(false),
            plugin_filter: State::new(0),
            plugin_query: State::new(String::new()),
            plugin_settings: State::new(ParamValues::new()),
            add_plugin_open: State::new(false),
            plugin_open: State::new(None),
            sheet_open: State::new(false),
            admin_tab: State::new(0),
            cred_name: State::new(String::new()),
            cred_kind: State::new(0),
            cred_description: State::new(String::new()),
            cred_first: State::new(String::new()),
            cred_second: State::new(String::new()),
            new_user: State::new(String::new()),
            new_password: State::new(String::new()),
            new_role: State::new(2),
        };
        Self { s: Rc::new(s) }
    }

    /// Signs in as `name` with `password`, as the screen of sign in does: what a test does
    /// before its first step.
    pub fn sign_in(&self, name: &str, password: &str) {
        self.s.name.set(name.to_owned());
        self.s.password.set(password.to_owned());
        S::sign_in(&self.s);
    }

    /// The client it talks to the server with.
    pub fn client(&self) -> RemoteClient {
        self.s.client.clone()
    }

    /// The page on screen.
    pub fn page(&self) -> Pagina {
        self.s.page.get()
    }
}

/// Nothing on screen: what a part that has nothing to show puts in its place.
pub(crate) fn nada() -> AnyView {
    Box::new(VStack::new(Vec::<AnyView>::new()))
}

/// The message a refusal of the server carries.
fn said(body: &Json) -> String {
    body["message"].as_str().map_or_else(|| "O servidor recusou.".to_owned(), str::to_owned)
}

/// Text put in a path of the API: `/` and the other reserved characters escaped.
pub(crate) fn escaped(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl S {
    /// Wakes the screen after an answer that came later, as in a browser.
    fn wake(&self) {
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker.wake();
        }
    }

    /// Reads `path` and hands the JSON to `done` when it is a 200; anything else says why in the
    /// status bar, and a 401 goes back to signing in.
    pub(crate) fn read(self: &Rc<Self>, path: &str, done: impl FnOnce(&Rc<S>, Json) + 'static) {
        let s = self.clone();
        self.client.get_json(path, move |answer| {
            match answer {
                Ok((200, body)) => done(&s, body),
                Ok((401, _)) => s.signed_in.set(false),
                Ok((_, body)) => s.message.set(said(&body)),
                Err(why) => s.message.set(format!("O servidor não respondeu: {why}")),
            }
            s.wake();
        });
    }

    /// Sends `body` to `path` and hands `done` the status and the JSON, whatever the status.
    pub(crate) fn send(
        self: &Rc<Self>,
        path: &str,
        body: &Json,
        done: impl FnOnce(&Rc<S>, u16, Json) + 'static,
    ) {
        let s = self.clone();
        self.client.send_json("POST", path, body, move |answer| {
            match answer {
                Ok((401, _)) => s.signed_in.set(false),
                Ok((status, body)) => done(&s, status, body),
                Err(why) => s.message.set(format!("O servidor não respondeu: {why}")),
            }
            s.wake();
        });
    }

    /// Asks `path` with `method` and no body (a `DELETE`), and hands `done` the status and the
    /// JSON, whatever the status.
    pub(crate) fn ask(
        self: &Rc<Self>,
        method: &str,
        path: &str,
        done: impl FnOnce(&Rc<S>, u16, Json) + 'static,
    ) {
        let s = self.clone();
        self.client.send_json(method, path, &Json::Null, move |answer| {
            match answer {
                Ok((401, _)) => s.signed_in.set(false),
                Ok((status, body)) => done(&s, status, body),
                Err(why) => s.message.set(format!("O servidor não respondeu: {why}")),
            }
            s.wake();
        });
    }

    fn sign_in(self: &Rc<Self>) {
        let s = self.clone();
        let name = self.name.get();
        self.client.sign_in(name.trim(), &self.password.get(), move |answer| {
            match answer {
                Ok(_) => {
                    s.password.set(String::new());
                    s.sign_in_error.set(String::new());
                    s.signed_in.set(true);
                    s.read("/api/me", |s, me| {
                        s.me.set(me["name"].as_str().unwrap_or_default().to_owned());
                        let roles = me["roles"]
                            .as_array()
                            .map(|r| r.iter().filter_map(|role| role.as_str().map(str::to_owned)).collect());
                        s.roles.set(roles.unwrap_or_default());
                    });
                    s.load_shell();
                    s.go(s.page.get());
                    s.listen();
                }
                Err(SignInError::Rejected) => {
                    s.sign_in_error.set("O nome ou a senha não conferem.".to_owned())
                }
                Err(other) => s.sign_in_error.set(format!("Não deu para entrar: {other:?}")),
            }
            s.wake();
        });
    }

    fn sign_out(self: &Rc<Self>) {
        self.client.sign_out();
        if let Some(feed) = self.live.borrow_mut().take() {
            feed.close();
        }
        self.signed_in.set(false);
        self.roles.set(Vec::new());
    }

    /// Whether whoever signed in administers the server.
    fn admin(&self) -> bool {
        self.roles.get().iter().any(|r| r == ADMIN)
    }

    /// Whether whoever signed in may act on builds.
    fn runs(&self) -> bool {
        self.roles.get().iter().any(|r| r == ADMIN || r == RUN)
    }

    /// What every page shows around it: the server and the screens of the plugins.
    fn load_shell(self: &Rc<Self>) {
        self.read("/api/ci/server", |s, body| s.server.set(body));
        self.read("/api/ci/screens", |s, body| s.screens.set(body));
    }

    /// Opens the live channel, when it is wanted and not open.
    fn listen(self: &Rc<Self>) {
        if !self.live_wanted || self.live.borrow().is_some() {
            return;
        }
        let Some(waker) = self.waker.borrow().clone() else { return };
        *self.live.borrow_mut() = Some(self.client.listen("ci", None, waker));
    }

    /// How the live channel is doing, when there is one.
    fn live_state(&self) -> Option<LiveState> {
        self.live.borrow().as_ref().map(LiveFeed::state)
    }

    /// Goes to `page` and reads what it shows.
    pub(crate) fn go(self: &Rc<Self>, page: Pagina) {
        self.page.set(page.clone());
        match page {
            Pagina::Jobs => {
                self.load_overview();
                self.load_occupancy();
            }
            Pagina::Historico => self.read("/api/ci/history?limit=200", |s, body| s.history.set(body)),
            Pagina::Build(job, number) => self.open_build(&job, number),
            Pagina::Fila => {
                self.load_occupancy();
                self.load_overview();
            }
            Pagina::Configuracao => {
                self.load_overview();
                self.load_occupancy();
                self.read("/api/ci/jobs", |s, body| s.jobs_full.set(body));
            }
            Pagina::Plugins => {
                self.load_overview();
                self.read("/api/ci/plugins", |s, body| s.plugins.set(body));
                self.read("/api/ci/audit?limit=50", |s, body| s.audit.set(body));
            }
            Pagina::Credenciais => self.read("/api/ci/credentials", |s, body| s.credentials.set(body)),
            Pagina::Usuarios => {
                self.read("/api/ci/users", |s, body| s.users.set(body));
                self.read("/api/ci/audit?limit=50", |s, body| s.audit.set(body));
            }
            Pagina::Plugin(name) => self.load_plugin_page(&name, None),
        }
    }

    fn load_overview(self: &Rc<Self>) {
        self.read("/api/ci/overview", |s, body| s.overview.set(body));
    }

    fn load_occupancy(self: &Rc<Self>) {
        let now = self.server.get()["now_ms"].as_i64().unwrap_or(0);
        let since = (now - 2 * 3_600_000).max(0);
        self.read(&format!("/api/ci/occupancy?since_ms={since}"), |s, body| s.occupancy.set(body));
    }

    /// Takes what the live channel heard and sets each screen right: lines go to the log of the
    /// build open, a build that changed reads its page again, plugins that changed read the shell.
    fn hear(self: &Rc<Self>) {
        let events = match self.live.borrow().as_ref() {
            Some(feed) => feed.drain(),
            None => return,
        };
        let (mut builds, mut plugins, mut all) = (false, false, false);
        for event in events {
            match event.kind.as_str() {
                "line" => self.heard_line(&event.data),
                "build" => builds = true,
                "plugins" => plugins = true,
                _ => all = true,
            }
        }
        if plugins || all {
            self.load_shell();
        }
        if builds || plugins || all {
            // The count of the queue in the menu comes from the overview, whatever the page.
            if !matches!(
                self.page.get(),
                Pagina::Jobs | Pagina::Fila | Pagina::Configuracao | Pagina::Plugins
            ) {
                self.load_overview();
            }
            match self.page.get() {
                Pagina::Build(job, number) => self.reload_build(&job, number),
                Pagina::Plugins if plugins => self.go(Pagina::Plugins),
                Pagina::Jobs | Pagina::Fila | Pagina::Historico => self.go(self.page.get()),
                _ => {}
            }
        }
    }
}

impl View for Painel {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let s = &self.s;
        if s.waker.borrow().is_none() {
            *s.waker.borrow_mut() = Some(cx.ui_waker());
        }
        if !s.signed_in.get() {
            return cx.child(&self.sign_in_screen());
        }
        s.listen();
        s.hear();
        S::poll_without_live(s, cx);
        s.fit_to(cx.viewport().width);
        let content: AnyView = match s.page.get() {
            Pagina::Jobs => jobs::page(s),
            Pagina::Historico => historico::page(s),
            Pagina::Build(job, number) => build::page(s, &job, number),
            Pagina::Fila => fila::page(s),
            Pagina::Configuracao => config::page(s),
            Pagina::Plugins => admin::plugins(s),
            Pagina::Credenciais => admin::credentials(s),
            Pagina::Usuarios => admin::users(s),
            Pagina::Plugin(name) => plugin::page(s, &name),
        };
        // On a phone the menu hides behind a button on a bar of its own, and the status bar goes (T7).
        let top: AnyView = if s.phone() {
            let search = s.palette_open.clone();
            Box::new(
                TitleBar::new("Raven CI")
                    .leading(SidebarTrigger::new(s.sidebar.clone()).icon(IconName::Menu))
                    .action(
                        IconButton::new(IconName::Search)
                            .label("Buscar comandos")
                            .on_click(move || search.set(true)),
                    )
                    .mobile(),
            )
        } else {
            nada()
        };
        let status: AnyView = if s.phone() { nada() } else { Box::new(self.status_bar()) };
        // The sidebar runs the height of the window; the page and its status bar sit beside it,
        // with no gap (T1).
        cx.child(
            &VStack::new((
                top,
                HStack::new((
                    self.sidebar(),
                    VStack::new((ScrollView::new((content,)).grow(1.0), status))
                        .spacing(0.0)
                        .align(Align::Stretch)
                        .grow(1.0),
                ))
                .spacing(0.0)
                .align(Align::Stretch)
                .grow(1.0),
                s.confirm.clone(),
                jobs::dispatch_dialog(s),
                self.palette(),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}
