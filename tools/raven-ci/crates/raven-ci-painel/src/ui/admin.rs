//! What administrators see (P-05, #889; page T4 and interaction I4): the plugins, each with its
//! `Toggle` and a sheet with what it depends on, what it asked for and its configuration; the
//! credentials; people and their roles; and the audit of each.

use std::rc::Rc;

use ravenrs::prelude::*;
use serde_json::{Value as Json, json};

use super::{Pagina, S, nada, plugin};
use crate::texto;

/// The roles a new person can get, in the order the form offers them.
const ROLES: [&str; 3] = ["Administradores", "Executores", "Leitores"];

/// A cell of a grid.
type Cell = ravenrs::prelude::Value;

/// The plugins: the grid with their toggles, the filter, the sheet of the one open and the
/// refusals.
pub(super) fn plugins(s: &Rc<S>) -> AnyView {
    let all: Vec<Json> = s.plugins.get().as_array().cloned().unwrap_or_default();
    let on = all.iter().filter(|p| p["state"]["state"] == "on").count();
    let filter = s.plugin_filter.get();
    let shown: Vec<&Json> = all
        .iter()
        .filter(|p| match filter {
            1 => p["enabled"] == true,
            2 => p["enabled"] != true,
            _ => true,
        })
        .collect();
    let rows: Vec<Vec<Cell>> = shown
        .iter()
        .map(|p| {
            let name = p["name"].as_str().unwrap_or_default().to_owned();
            let points: Vec<&str> =
                p["points"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
            let (state, _) = texto::plugin_state(&p["state"]);
            vec![
                name.clone().into(),
                texto::origin_words(&p["origin"]).into(),
                points.join(", ").into(),
                p["version"].as_str().unwrap_or("—").to_owned().into(),
                texto::memory_words(&p["memory"]).into(),
                state.into(),
                format!("{name}|{}", p["enabled"] == true).into(),
            ]
        })
        .collect();
    let toggle = {
        let s = s.clone();
        move |value: &Cell| -> AnyView {
            let text = value.key_text();
            let (name, enabled) = text.rsplit_once('|').unwrap_or((&text, "false"));
            let (name, enabled) = (name.to_owned(), enabled == "true");
            let s = s.clone();
            let label = format!("Ligar {name}");
            let binding = Binding::new(move || enabled, move |want: bool| switch(&s, &name, want));
            Box::new(Toggle::new(binding).small().accessible_name(label))
        }
    };
    let open = {
        let s = s.clone();
        move |row: &Vec<Cell>| {
            s.plugin_open.set(Some(row[0].key_text()));
            s.sheet_open.set(true);
            s.read(&format!("/api/ci/audit?plugin={}&limit=20", row[0].key_text()), |s, body| {
                s.audit.set(body)
            });
        }
    };
    let grid = DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Plugin").width(170.0),
            GridColumn::text("Forma").width(110.0),
            GridColumn::text("Oferece").fill(),
            GridColumn::text("Versão").width(90.0),
            GridColumn::text("Memória").width(120.0),
            GridColumn::text("Estado").width(110.0).template(|value| -> AnyView {
                let state = value.key_text();
                let tone = match state.as_str() {
                    "Ligado" => Tone::Success,
                    "Recusado" => Tone::Danger,
                    _ => Tone::Neutral,
                };
                Box::new(Badge::new(state).tone(tone))
            }),
            GridColumn::text("Ligado")
                .width(90.0)
                // The cell says the state; the switch in it is the one called "Ligar …".
                .format(|value| {
                    if value.key_text().ends_with("|true") {
                        "ligado".to_owned()
                    } else {
                        "desligado".to_owned()
                    }
                })
                .template(toggle),
        ],
    )
    .key_column(0)
    .striped(true)
    .on_row_open(open)
    .empty_text("Nenhum plugin neste filtro.")
    .height(420.0);
    let refused: Vec<AnyView> = all
        .iter()
        .filter(|p| p["state"]["state"] == "refused")
        .map(|p| -> AnyView {
            Box::new(
                Alert::new(
                    format!(
                        "{} {} não foi carregado",
                        p["name"].as_str().unwrap_or_default(),
                        p["version"].as_str().unwrap_or("")
                    ),
                    p["state"]["reason"].as_str().unwrap_or("sem motivo"),
                )
                .tone(Tone::Danger),
            )
        })
        .collect();
    Box::new(
        VStack::new((
            PageHeader::new("Plugins")
                .description("Tudo o que o servidor faz vem de um plugin. Desligado, ele não ocupa as telas nem as rotas.")
                .badge(Badge::new(all.len().to_string())),
            HStack::new((
                SegmentedControl::new(s.plugin_filter.binding(), ["Todos", "Ligados", "Desligados"]).label("Mostrar"),
                Spacer::new(),
                Text::new(format!("{on} ligados · {} desligados ou recusados", all.len() - on)).foreground(Semantic::InkMuted),
            ))
            .align(Align::Center),
            grid,
            VStack::new(refused).spacing(8.0).align(Align::Stretch),
            sheet(s, &all),
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// Switches the plugin `name` on or off; the toggle goes back if the server refuses (I4).
fn switch(s: &Rc<S>, name: &str, on: bool) {
    let path = if on { "/api/ci/plugins/enable" } else { "/api/ci/plugins/disable" };
    let name = name.to_owned();
    s.send(path, &json!({ "name": name }), move |s, status, body| {
        if status == 200 {
            let words = if on { "ligado" } else { "desligado" };
            s.message.set(format!("Plugin {name} {words}."));
        } else {
            s.message.set(body["message"].as_str().unwrap_or("o servidor recusou").to_owned());
        }
        s.load_shell();
        s.go(Pagina::Plugins);
    });
}

/// The sheet of the plugin open: what it is for, what it depends on, its permissions, its
/// configuration and its audit.
fn sheet(s: &Rc<S>, all: &[Json]) -> AnyView {
    let Some(name) = s.plugin_open.get() else { return nada() };
    let Some(p) = all.iter().find(|p| p["name"] == name.as_str()) else { return nada() };
    let badges = |list: &Json| -> AnyView {
        let items: Vec<Badge> = list
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .map(|t| Badge::new(t.to_owned()))
            .collect();
        if items.is_empty() {
            Box::new(Text::new("nenhuma").foreground(Semantic::InkMuted))
        } else {
            Box::new(HStack::new(items).spacing(8.0).wrap())
        }
    };
    let requires: Vec<PropertyRow> = p["requires"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            PropertyRow::new(
                r["plugin"].as_str().unwrap_or_default(),
                r["accepts"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    let requires: AnyView = if requires.is_empty() {
        Box::new(Text::new("De nenhum outro plugin.").foreground(Semantic::InkMuted))
    } else {
        Box::new(VStack::new(requires).spacing(4.0).align(Align::Stretch))
    };
    let caption = |t: &str| Text::new(t.to_owned()).style(TextStyle::Caption).foreground(Semantic::InkMuted);
    let content = VStack::new((
        Text::new(p["summary"].as_str().unwrap_or_default().to_owned()),
        caption("DEPENDE DE"),
        requires,
        caption("PERMISSÕES DECLARADAS"),
        badges(&p["declared"]),
        caption("PERMISSÕES DADAS"),
        badges(&p["granted"]),
        caption("CONFIGURAÇÃO"),
        plugin::json_view(&p["settings"]),
        caption("AUDITORIA DESTE PLUGIN"),
        audit_grid(&s.audit.get(), 220.0),
    ))
    .spacing(12.0)
    .align(Align::Stretch);
    Box::new(
        Sheet::new(s.sheet_open.binding(), name).description("Ficha do plugin").content(content).width(420.0),
    )
}

/// The audit, the newest first.
fn audit_grid(audit: &Json, height: f32) -> AnyView {
    let rows: Vec<Vec<Cell>> = audit
        .as_array()
        .into_iter()
        .flatten()
        .map(|line| {
            vec![
                texto::clock(line["at_ms"].as_i64().unwrap_or(0)).into(),
                line["who"].as_str().unwrap_or_default().to_owned().into(),
                line["permission"].as_str().unwrap_or_default().to_owned().into(),
                line["what"].as_str().unwrap_or_default().to_owned().into(),
                (if line["granted"] == true { "sim" } else { "não" }).to_owned().into(),
            ]
        })
        .collect();
    Box::new(
        DataGrid::new(
            GridData::new(rows),
            [
                GridColumn::text("Hora (UTC)").width(100.0),
                GridColumn::text("Quem").width(140.0),
                GridColumn::text("Permissão").width(120.0),
                GridColumn::text("O quê").fill(),
                GridColumn::text("Aceito").width(80.0),
            ],
        )
        .toolbar(false)
        .empty_text("Nada na auditoria ainda.")
        .height(height),
    )
}

/// The credentials, with no value.
pub(super) fn credentials(s: &Rc<S>) -> AnyView {
    let list = s.credentials.get();
    let now = s.server.get()["now_ms"].as_i64().unwrap_or(0);
    let rows: Vec<Vec<Cell>> = list
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            vec![
                c["name"].as_str().unwrap_or_default().to_owned().into(),
                c["kind"].as_str().unwrap_or_default().to_owned().into(),
                c["created_by"].as_str().unwrap_or("—").to_owned().into(),
                c["created_ms"].as_i64().map(|t| texto::ago(t, now)).unwrap_or_default().into(),
                c["updated_ms"].as_i64().map(|t| texto::ago(t, now)).unwrap_or_default().into(),
            ]
        })
        .collect();
    Box::new(
        VStack::new((
            PageHeader::new("Credenciais")
                .description("Os segredos que os jobs usam, cifrados no cofre do servidor. O valor nunca sai do servidor.")
                .badge(Badge::new(rows.len().to_string())),
            DataGrid::new(
                GridData::new(rows),
                [
                    GridColumn::text("Nome").fill(),
                    GridColumn::text("Tipo").width(160.0),
                    GridColumn::text("Criada por").width(160.0),
                    GridColumn::text("Criada").width(120.0),
                    GridColumn::text("Mudou").width(120.0),
                ],
            )
            .empty_text("Nenhuma credencial. Crie com raven-ci credencial criar.")
            .height(360.0),
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}

/// People and their roles, the form of a new person, and the audit of what people did.
pub(super) fn users(s: &Rc<S>) -> AnyView {
    let rows: Vec<Vec<Cell>> = s
        .users
        .get()
        .as_array()
        .into_iter()
        .flatten()
        .map(|u| {
            let roles: Vec<&str> =
                u["roles"].as_array().into_iter().flatten().filter_map(Json::as_str).collect();
            vec![u["name"].as_str().unwrap_or_default().to_owned().into(), roles.join(", ").into()]
        })
        .collect();
    let create = {
        let s = s.clone();
        move || {
            let name = s.new_user.get();
            let role = ROLES[s.new_role.get().min(ROLES.len() - 1)];
            let body = json!({ "name": name.trim(), "password": s.new_password.get(), "role": role });
            s.send("/api/ci/users", &body, move |s, status, body| {
                if status == 200 {
                    s.new_user.set(String::new());
                    s.new_password.set(String::new());
                    s.message.set(format!("Usuário {} criado.", body["name"].as_str().unwrap_or_default()));
                    s.go(Pagina::Usuarios);
                } else {
                    s.message.set(body["message"].as_str().unwrap_or("não criou").to_owned());
                }
            });
        }
    };
    let people = DataGrid::new(
        GridData::new(rows),
        [GridColumn::text("Usuário").width(220.0), GridColumn::text("Papéis").fill()],
    )
    .empty_text("Ninguém ainda.")
    .height(260.0);
    let form = Card::new(
        VStack::new((
            Text::new("Novo usuário").style(TextStyle::Headline),
            TextField::new(s.new_user.binding()).placeholder("Nome"),
            TextField::new(s.new_password.binding()).placeholder("Senha").secure(),
            Select::new(s.new_role.binding(), ROLES).label("Papel"),
            Button::new("Criar usuário").icon(IconName::Plus).on_click(create),
        ))
        .spacing(12.0)
        .align(Align::Stretch),
    )
    .padding(16.0);
    let tabs = Tabs::new(s.admin_tab.binding())
        .tab(
            "Usuários",
            HStack::new((
                VStack::new((people,)).align(Align::Stretch).grow(1.0),
                VStack::new((form,)).width(320.0),
            ))
            .spacing(16.0)
            .align(Align::Start)
            .wrap(),
        )
        .tab("Auditoria", audit_grid(&s.audit.get(), 360.0));
    Box::new(
        VStack::new((
            PageHeader::new("Usuários e papéis").description(
                "Administradores cuidam do servidor; Executores disparam, repetem e cancelam; Leitores só leem.",
            ),
            tabs,
        ))
        .spacing(16.0)
        .padding(24.0)
        .align(Align::Stretch),
    )
}
