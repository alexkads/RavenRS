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
mod config;
mod fila;
mod jobs;
mod plugin;

/// The role that administers the server.
const ADMIN: &str = "Administradores";
/// The role that runs builds.
const RUN: &str = "Executores";

/// A page of the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pagina {
    /// The jobs (T1).
    Jobs,
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
    plugin_data: State<BTreeMap<String, Json>>,
    message: State<String>,
    // The live channel, and when the log was last read without it.
    live: RefCell<Option<LiveFeed>>,
    waker: RefCell<Option<UiWaker>>,
    polled_at: Cell<f64>,
    confirm: ConfirmDialog,
    // The jobs (P-01).
    folder: State<Option<String>>,
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
    stage_of_step: RefCell<BTreeMap<String, String>>,
    build_tab: State<usize>,
    graph: State<FlowGraph>,
    graph_of: RefCell<String>,
    // The queue (P-03).
    selected_event: State<Option<u64>>,
    // The pipeline (P-04).
    config_job: State<Option<String>>,
    config_pick: State<usize>,
    jobs_full: State<Json>,
    config_tab: State<usize>,
    config_text: State<String>,
    config_checked: RefCell<String>,
    config_errors: State<Vec<(usize, String)>>,
    // Plugins and people (P-05).
    plugin_filter: State<usize>,
    plugin_open: State<Option<String>>,
    sheet_open: State<bool>,
    admin_tab: State<usize>,
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
            plugin_data: State::new(BTreeMap::new()),
            message: State::new(String::new()),
            live: RefCell::new(None),
            waker: RefCell::new(None),
            polled_at: Cell::new(0.0),
            confirm: ConfirmDialog::new(),
            folder: State::new(None),
            job_keys: State::new(BTreeSet::new()),
            dispatch_open: State::new(false),
            dispatch_job: State::new(String::new()),
            dispatch_values: State::new(ParamValues::new()),
            dispatch_error: State::new(String::new()),
            log: LogBuffer::new(),
            log_of: RefCell::new(None),
            log_seq: Cell::new(0),
            log_section: RefCell::new(String::new()),
            stage_of_step: RefCell::new(BTreeMap::new()),
            build_tab: State::new(0),
            graph: State::new(FlowGraph::new()),
            graph_of: RefCell::new(String::new()),
            selected_event: State::new(None),
            config_job: State::new(None),
            config_pick: State::new(0),
            jobs_full: State::new(json!([])),
            config_tab: State::new(0),
            config_text: State::new(String::new()),
            config_checked: RefCell::new(String::new()),
            config_errors: State::new(Vec::new()),
            plugin_filter: State::new(0),
            plugin_open: State::new(None),
            sheet_open: State::new(false),
            admin_tab: State::new(0),
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
            Pagina::Jobs => self.load_overview(),
            Pagina::Build(job, number) => self.open_build(&job, number),
            Pagina::Fila => {
                self.load_occupancy();
                self.load_overview();
            }
            Pagina::Configuracao => {
                self.load_overview();
                self.read("/api/ci/jobs", |s, body| s.jobs_full.set(body));
            }
            Pagina::Plugins => {
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
            match self.page.get() {
                Pagina::Build(job, number) => self.reload_build(&job, number),
                Pagina::Plugins if plugins => self.go(Pagina::Plugins),
                Pagina::Jobs | Pagina::Fila | Pagina::Configuracao => self.go(self.page.get()),
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
        let content: AnyView = match s.page.get() {
            Pagina::Jobs => jobs::page(s),
            Pagina::Build(job, number) => build::page(s, &job, number),
            Pagina::Fila => fila::page(s),
            Pagina::Configuracao => config::page(s),
            Pagina::Plugins => admin::plugins(s),
            Pagina::Credenciais => admin::credentials(s),
            Pagina::Usuarios => admin::users(s),
            Pagina::Plugin(name) => plugin::page(s, &name),
        };
        cx.child(
            &VStack::new((
                HStack::new((self.sidebar(), ScrollView::new((content,)).grow(1.0)))
                    .align(Align::Stretch)
                    .grow(1.0),
                self.status_bar(),
                s.confirm.clone(),
                jobs::dispatch_dialog(s),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0),
        )
    }
}

impl Painel {
    /// The screen of signing in: the name, the password and why it did not work.
    fn sign_in_screen(&self) -> AnyView {
        let s = self.s.clone();
        let enter = {
            let s = s.clone();
            move || S::sign_in(&s)
        };
        let submit = {
            let s = s.clone();
            move |_: String| S::sign_in(&s)
        };
        let error = s.sign_in_error.get();
        let alert: AnyView = if error.is_empty() {
            nada()
        } else {
            Box::new(Alert::new("Não entrou", error).tone(Tone::Danger))
        };
        Box::new(
            VStack::new((
                Spacer::new(),
                VStack::new((Card::new(
                    VStack::new((
                        HStack::new((
                            Icon::new(IconName::GitBranch).color(Semantic::Accent),
                            Text::new("Raven CI").style(TextStyle::Title),
                        ))
                        .spacing(10.0)
                        .align(Align::Center),
                        Text::new("Entre com o usuário do servidor de builds.")
                            .foreground(Semantic::InkMuted),
                        TextField::new(s.name.binding()).placeholder("Usuário").autofocus(),
                        TextField::new(s.password.binding()).placeholder("Senha").secure().on_submit(submit),
                        alert,
                        Button::new("Entrar").icon(IconName::LogIn).on_click(enter),
                    ))
                    .spacing(12.0)
                    .align(Align::Stretch),
                )
                .padding(24.0),))
                .align(Align::Stretch)
                .width(380.0),
                Spacer::new(),
            ))
            .align(Align::Center)
            .grow(1.0),
        )
    }

    /// The sidebar: BUILDS and SERVIDOR, as the design has them, and the pages of the plugins
    /// under their groups. SERVIDOR is only for administrators.
    fn sidebar(&self) -> AnyView {
        let s = &self.s;
        let page = s.page.get();
        let item = |icon: IconName, label: &str, to: Pagina, keys: &str| {
            let s = s.clone();
            let chosen = match (&page, &to) {
                (Pagina::Build(..), Pagina::Jobs) => true,
                _ => page == to,
            };
            let mut item = SidebarItem::new(icon, label).selected(chosen).on_click(move || s.go(to.clone()));
            if !keys.is_empty() {
                item = item.shortcut(keys);
            }
            item
        };
        let queued = s.overview.get()["queued"].as_u64().unwrap_or(0);
        let mut fila = item(IconName::Layers, "Fila e agentes", Pagina::Fila, "G A");
        if queued > 0 {
            fila = fila.count_spoken(queued.to_string(), format!("{queued} na fila"));
        }
        let mut groups: Vec<(String, Vec<SidebarItem>)> = vec![(
            "Builds".to_owned(),
            vec![
                item(IconName::Activity, "Jobs", Pagina::Jobs, "G J"),
                fila,
                item(IconName::Code, "Configuração", Pagina::Configuracao, ""),
            ],
        )];
        if s.admin() {
            groups.push((
                "Servidor".to_owned(),
                vec![
                    item(IconName::Plug, "Plugins", Pagina::Plugins, "G P"),
                    item(IconName::Key, "Credenciais", Pagina::Credenciais, ""),
                    item(IconName::Users, "Usuários e papéis", Pagina::Usuarios, ""),
                ],
            ));
        }
        for screen in s.screens.get().as_array().into_iter().flatten().filter(|sc| sc["kind"] == "page") {
            let (name, caption) =
                (screen["name"].as_str().unwrap_or_default(), screen["caption"].as_str().unwrap_or_default());
            let group = screen["group"].as_str().unwrap_or("Plugins").to_owned();
            let entry = item(IconName::Package, caption, Pagina::Plugin(name.to_owned()), "");
            match groups.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(&group)) {
                Some((_, items)) => items.push(entry),
                None => groups.push((group, vec![entry])),
            }
        }
        let groups: Vec<SidebarGroup<Vec<SidebarItem>>> =
            groups.into_iter().map(|(title, items)| SidebarGroup::new(title, items)).collect();
        let sign_out = {
            let s = s.clone();
            move || s.sign_out()
        };
        Box::new(
            Sidebar::new(groups)
                .label("Navegação")
                .header(
                    SidebarBrand::new(Icon::new(IconName::GitBranch).color(Semantic::Accent), "Raven CI")
                        .subtitle("Servidor de builds"),
                )
                .footer(
                    SidebarItem::new(IconName::LogOut, format!("Sair ({})", s.me.get())).on_click(sign_out),
                ),
        )
    }

    /// The status bar: the version, how the live channel is doing, the plugins on and the last
    /// message.
    fn status_bar(&self) -> StatusBar {
        let s = &self.s;
        let server = s.server.get();
        let live = match s.live_state() {
            Some(LiveState::Connected) => "Ao vivo: os builds chegam sem recarregar",
            Some(LiveState::Connecting) => "Ligando o canal ao vivo…",
            Some(_) => "Sem canal ao vivo: o log é lido de novo a cada segundo",
            None => "Atualiza ao abrir cada página",
        };
        let on = server["plugins_on"].as_array().map_or(0, Vec::len);
        StatusBar::new()
            .left((
                StatusBar::text(format!(
                    "Raven CI {} · painel {}",
                    server["version"].as_str().unwrap_or("—"),
                    crate::VERSION
                )),
                StatusBar::text(live),
            ))
            .right((
                StatusBar::text(s.message.get()).truncate(),
                StatusBar::text(format!("{on} plugins ligados")),
            ))
    }
}

impl S {
    /// With no live channel, a build that runs reads its new lines again each second.
    fn poll_without_live(self: &Rc<Self>, cx: &mut BuildCx<'_>) {
        if matches!(self.live_state(), Some(LiveState::Connected)) {
            return;
        }
        let Pagina::Build(job, number) = self.page.get() else { return };
        let running = matches!(self.build.get()["build"]["status"].as_str(), Some("running" | "queued"));
        if !running {
            return;
        }
        let now = cx.now();
        if now - self.polled_at.get() >= 1.0 {
            self.polled_at.set(now);
            self.reload_build(&job, number);
        }
        cx.wake_at(now + 1.0);
    }
}
