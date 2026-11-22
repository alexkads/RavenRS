//! The parts of the shell around every page (P-00, #884): the screen of signing in, the sidebar
//! with the pages of the core and of the plugins, the status bar, and reading again while a build
//! runs when there is no live channel.

use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app_remote::LiveState;

use super::{Pagina, Painel, S, nada};

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
                            Icon::new(IconName::GitBranch).color(Semantic::Accent),
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

    /// The sidebar: BUILDS and SERVIDOR, as the design has them, and the pages of the plugins
    /// under their groups. SERVIDOR is only for administrators.
    pub(super) fn sidebar(&self) -> AnyView {
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
    pub(super) fn status_bar(&self) -> StatusBar {
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
