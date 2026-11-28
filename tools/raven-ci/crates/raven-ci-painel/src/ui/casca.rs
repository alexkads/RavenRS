//! The parts of the shell around every page (P-00, #884; as the boards of D-01 draw them, #919):
//! the screen of signing in, the sidebar `Sidebar/Compact, with sections` (a search of commands,
//! BUILDS and SERVIDOR, the pages of the plugins under their groups, and "Recolher menu" at its
//! foot), the palette of commands, the status bar with the live channel in its middle, the width
//! the pages lay out by (I7), and reading again while a build runs when there is no live channel.

use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app_remote::LiveState;

use super::{Pagina, Painel, S, nada};

/// Below this width the menu folds to its icons, and the details of a build go under its log (I7).
pub(super) const ICONS_BELOW: f32 = 1100.0;
/// Below this width the menu leaves the screen behind a button, and the pages become those of the
/// phone (T7).
pub(super) const PHONE_BELOW: f32 = 480.0;
/// Below this width the numbers of Jobs go in two columns (I7).
pub(super) const TWO_COLUMNS_BELOW: f32 = 760.0;

impl Painel {
    /// The screen of signing in: the name, the password and why it did not work.
    pub(super) fn sign_in_screen(&self) -> AnyView {
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
                            Icon::new(IconName::Rocket).color(Semantic::Accent),
                            Text::new("Raven CI").style(TextStyle::Title),
                        ))
                        .spacing(12.0)
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

    /// The sidebar as the boards draw it: the brand, "Buscar comandos", BUILDS (Jobs, Histórico,
    /// Fila e agentes), SERVIDOR (Plugins, Credenciais, Usuários e papéis, Configuração) for
    /// administrators, the pages of the plugins under their groups, and at its foot who signed in
    /// and "Recolher menu". It folds to icons below [`ICONS_BELOW`] and hides below
    /// [`PHONE_BELOW`].
    pub(super) fn sidebar(&self) -> AnyView {
        let s = &self.s;
        let page = s.page.get();
        let item = |icon: IconName, label: &str, to: Pagina| {
            let s = s.clone();
            let chosen = match (&page, &to) {
                (Pagina::Build(..), Pagina::Jobs) => true,
                _ => page == to,
            };
            SidebarItem::new(icon, label).compact().selected(chosen).on_click(move || s.go(to.clone()))
        };
        let palette = {
            let open = s.palette_open.clone();
            SidebarItem::new(IconName::Search, "Buscar comandos")
                .compact()
                .shortcut("⌘K")
                .on_click(move || open.set(true))
        };
        let queued = s.overview.get()["queued"].as_u64().unwrap_or(0);
        let mut fila = item(IconName::Server, "Fila e agentes", Pagina::Fila);
        if queued > 0 {
            fila = fila.count_spoken(queued.to_string(), format!("{queued} na fila"));
        }
        let mut content: Vec<AnyView> = vec![
            Box::new(palette),
            Box::new(SidebarSection::new("Builds")),
            Box::new(item(IconName::List, "Jobs", Pagina::Jobs)),
            Box::new(item(IconName::History, "Histórico", Pagina::Historico)),
            Box::new(fila),
        ];
        let mut groups: Vec<(String, Vec<AnyView>)> = Vec::new();
        if s.admin() {
            groups.push((
                "Servidor".to_owned(),
                vec![
                    Box::new(item(IconName::Plug, "Plugins", Pagina::Plugins)),
                    Box::new(item(IconName::Key, "Credenciais", Pagina::Credenciais)),
                    Box::new(item(IconName::Users, "Usuários e papéis", Pagina::Usuarios)),
                    Box::new(item(IconName::Settings, "Configuração", Pagina::Configuracao)),
                ],
            ));
        } else {
            content.push(Box::new(item(IconName::Settings, "Configuração", Pagina::Configuracao)));
        }
        for screen in s.screens.get().as_array().into_iter().flatten().filter(|sc| sc["kind"] == "page") {
            let (name, caption) =
                (screen["name"].as_str().unwrap_or_default(), screen["caption"].as_str().unwrap_or_default());
            let group = screen["group"].as_str().unwrap_or("Plugins").to_owned();
            let entry: AnyView = Box::new(item(IconName::Package, caption, Pagina::Plugin(name.to_owned())));
            match groups.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(&group)) {
                Some((_, items)) => items.push(entry),
                None => groups.push((group, vec![entry])),
            }
        }
        for (title, items) in groups {
            content.push(Box::new(SidebarSection::new(title)));
            content.extend(items);
        }
        let sign_out = {
            let s = s.clone();
            move || s.sign_out()
        };
        let fold = {
            let state = s.sidebar.clone();
            move || state.toggle()
        };
        Box::new(
            Sidebar::new(content)
                .label("Navegação")
                .collapsible(SidebarCollapse::Icons)
                .narrow_below(PHONE_BELOW)
                .state(s.sidebar.clone())
                .header(
                    SidebarBrand::new(Icon::new(IconName::Rocket), "Raven CI").subtitle("Servidor de builds"),
                )
                .footer(
                    VStack::new((
                        SidebarItem::new(IconName::LogOut, format!("Sair ({})", s.me.get()))
                            .compact()
                            .on_click(sign_out),
                        SidebarItem::new(IconName::PanelLeft, "Recolher menu")
                            .compact()
                            .shortcut("⌘B")
                            .on_click(fold),
                    ))
                    .spacing(4.0)
                    .align(Align::Stretch),
                ),
        )
    }

    /// The palette of commands (⌘K): every page, and Disparar for the job chosen.
    pub(super) fn palette(&self) -> AnyView {
        let s = &self.s;
        let go = |label: &str, icon: IconName, to: Pagina| {
            let s = s.clone();
            Command::new(label, move || s.go(to.clone())).group("Ir para").icon(icon)
        };
        let mut palette = CommandPalette::new(s.palette_open.binding())
            .placeholder("Buscar páginas e ações")
            .command(go("Jobs", IconName::List, Pagina::Jobs).shortcut("G J"))
            .command(go("Histórico", IconName::History, Pagina::Historico))
            .command(go("Fila e agentes", IconName::Server, Pagina::Fila).shortcut("G A"))
            .command(go("Configuração", IconName::Settings, Pagina::Configuracao));
        if s.admin() {
            palette = palette
                .command(go("Plugins", IconName::Plug, Pagina::Plugins).shortcut("G P"))
                .command(go("Credenciais", IconName::Key, Pagina::Credenciais))
                .command(go("Usuários e papéis", IconName::Users, Pagina::Usuarios));
        }
        if s.runs() {
            for job in s.overview.get()["jobs"].as_array().into_iter().flatten() {
                let Some(name) = job["name"].as_str().map(str::to_owned) else { continue };
                let s = s.clone();
                let label = format!("Disparar {name}");
                palette = palette.command(
                    Command::new(label, move || super::jobs::ask_build(&s, &name))
                        .group("Disparar")
                        .icon(IconName::Play),
                );
            }
        }
        Box::new(palette)
    }

    /// The status bar: the version on the left, the live channel in the middle with its dot, the
    /// plugins on and the last message on the right.
    pub(super) fn status_bar(&self) -> StatusBar {
        let s = &self.s;
        let server = s.server.get();
        let (live, tone) = match s.live_state() {
            Some(LiveState::Connected) => ("Ao vivo: os builds chegam sem recarregar", Tone::Success),
            Some(LiveState::Connecting) => ("Ligando o canal ao vivo…", Tone::Warning),
            Some(_) => ("Sem canal ao vivo: o log é lido de novo a cada segundo", Tone::Warning),
            None => ("Atualiza ao abrir cada página", Tone::Neutral),
        };
        let on = server["plugins_on"].as_array().map_or(0, Vec::len);
        StatusBar::new()
            .left((StatusBar::text(format!(
                "Raven CI {} · painel {}",
                server["version"].as_str().unwrap_or("—"),
                crate::VERSION
            )),))
            .center((Dot::new().tone(tone), StatusBar::text(live)))
            .center_tone(Tone::Neutral)
            .right((
                StatusBar::text(s.message.get()).truncate(),
                StatusBar::text(format!("{on} plugins ligados")),
            ))
    }
}

impl S {
    /// Takes the width of the window: crossing [`ICONS_BELOW`] folds or opens the menu, as the
    /// window narrows or widens; in between, "Recolher menu" decides.
    pub(super) fn fit_to(&self, width: f32) {
        let before = self.width.replace(width);
        if (before >= ICONS_BELOW) != (width >= ICONS_BELOW) {
            self.sidebar.set_open(width >= ICONS_BELOW);
        }
    }

    /// Whether the window is as narrow as a phone's (T7).
    pub(super) fn phone(&self) -> bool {
        self.width.get() < PHONE_BELOW
    }

    /// Whether the window is narrower than [`ICONS_BELOW`].
    pub(super) fn narrow(&self) -> bool {
        self.width.get() < ICONS_BELOW
    }

    /// With no live channel, a build that runs reads its new lines again each second.
    pub(super) fn poll_without_live(self: &Rc<Self>, cx: &mut BuildCx<'_>) {
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
