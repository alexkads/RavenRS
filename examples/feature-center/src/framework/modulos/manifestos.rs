//! `ApplicationBuilder` and `ModuleManifest`, every feature: two modules that say their version,
//! where they run, the storage and the capabilities they need, the roles, migrations and jobs they
//! bring, and the versions they accept of each other, put together in code over a host that the
//! switches describe. What does not fit is refused when it is put together, not on a screen later.

use ravenrs::prelude::*;
use ravenrs_app::{
    Application, EntityInfo, FieldInfo, FieldKind, Module, ModuleManifest, Target, VersionRange,
};
use ravenrs_app_ui::Session;

use super::vendas::Sale;
use super::{button, layout};

/// The control's name, as the session knows it.
pub const ID: &str = "ModulesDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Módulos e manifestos";

const STOCK: &str = "Estoque";
const LABELS: &str = "Etiquetas";

fn entity(name: &str, caption: &str) -> EntityInfo {
    EntityInfo::new(name)
        .caption(caption)
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
}

/// A module that keeps the stock, in the version the switch says.
struct Stock {
    version: &'static str,
}

impl Module for Stock {
    fn name(&self) -> &str {
        STOCK
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![entity("StockItem", "Itens do estoque")]
    }

    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new()
            .version(self.version)
            .unwrap_or_default()
            .target(Target::Desktop)
            .target(Target::Server)
            .database("sqlite")
            .capability("events.status")
            .role("Estoquistas")
            .migration("estoque-001-saldos")
            .job("estoque.recontagem")
    }
}

/// A module of shelf labels that needs the stock in a version 2.
struct Labels;

impl Module for Labels {
    fn name(&self) -> &str {
        LABELS
    }

    fn dependencies(&self) -> Vec<String> {
        vec![STOCK.to_owned()]
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![entity("ShelfLabel", "Etiquetas de gôndola")]
    }

    fn manifest(&self) -> ModuleManifest {
        let two = VersionRange::compatible("2.0.0").unwrap_or_else(|_| unreachable_range());
        ModuleManifest::new().version("1.0.0").unwrap_or_default().requires(STOCK, two)
    }
}

/// A range nothing falls in, should a constant above ever stop parsing.
fn unreachable_range() -> VersionRange {
    let zero = ravenrs_app::Version { major: 0, minor: 0, patch: 0 };
    VersionRange { from: zero, below: zero }
}

/// What the actions of the shop ask for by type, given to them by the application.
#[derive(Debug)]
struct Margin(f64);

/// The host the switches describe.
#[derive(Clone, Copy, Default, PartialEq)]
struct Host {
    web: bool,
    postgres: bool,
    no_status: bool,
    labels_off: bool,
    stock_three: bool,
}

/// Puts the application together over this host, and says what came of it.
fn put_together(host: Host) -> Vec<String> {
    let margin = Margin(0.3);
    let margin_text = format!("Serviço dado às Actions: margem de {:.0} %", margin.0 * 100.0);
    let mut builder = Application::builder("Loja")
        .target(if host.web { Target::Web } else { Target::Desktop })
        .database(if host.postgres { "postgres" } else { "sqlite" })
        .module(Stock { version: if host.stock_three { "3.0.0" } else { "2.1.0" } })
        .module(Labels)
        .entity_of::<Sale>()
        .service(margin);
    if !host.no_status {
        builder = builder.capability("events.status");
    }
    if host.labels_off {
        builder = builder.switch_off(LABELS);
    }
    let application = match builder.build() {
        Ok(application) => application,
        Err(why) => return vec![format!("Recusada ao montar: {}", why.0)],
    };
    let mut lines = vec![format!(
        "Montada: {} entidades ({})",
        application.entities().len(),
        application.entities().iter().map(|e| e.caption.clone()).collect::<Vec<_>>().join(", ")
    )];
    for (name, manifest) in
        application.manifests().into_iter().filter(|(name, _)| name == STOCK || name == LABELS)
    {
        let version = manifest.version.map_or_else(|| "sem versão".to_owned(), |v| v.to_string());
        let mut said = vec![format!("{name} {version}")];
        if !manifest.targets.is_empty() {
            said.push(format!("roda em {:?}", manifest.targets));
        }
        for (module, range) in &manifest.requires {
            said.push(format!("aceita {module} {range}"));
        }
        for (what, list) in [
            ("banco", &manifest.databases),
            ("pede", &manifest.capabilities),
            ("papel", &manifest.roles),
            ("migração", &manifest.migrations),
            ("tarefa", &manifest.jobs),
        ] {
            if !list.is_empty() {
                said.push(format!("{what} {}", list.join(", ")));
            }
        }
        lines.push(said.join(" · "));
    }
    let off = application.switched_off_entities();
    lines.push(if off.is_empty() {
        "Nenhum módulo desligado".to_owned()
    } else {
        format!("Desligado, só para ler: {}", off.join(", "))
    });
    lines.push(margin_text);
    lines
}

/// The screen.
struct ModulesScreen;

impl View for ModulesScreen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let host = cx.use_state(Host::default);
        let said =
            cx.use_state(|| "Mude o que o host oferece e veja a aplicação montada de novo.".to_owned());
        let flip = |caption: &'static str, change: fn(&mut Host)| {
            let (host, said) = (host.clone(), said.clone());
            button(caption, move || {
                let mut now = host.get();
                change(&mut now);
                host.set(now);
                said.set(format!("{caption}: montada de novo"));
            })
        };
        let now = host.get();
        let on = |yes: bool| if yes { "sim" } else { "não" };
        let buttons = vec![
            flip("Rodar na Web", |h| h.web = !h.web),
            flip("Usar Postgres", |h| h.postgres = !h.postgres),
            flip("Tirar events.status", |h| h.no_status = !h.no_status),
            flip("Desligar Etiquetas", |h| h.labels_off = !h.labels_off),
            flip("Estoque na versão 3", |h| h.stock_three = !h.stock_three),
        ];
        let mut shown = vec![format!(
            "Host: Web {}, Postgres {}, sem events.status {}, Etiquetas desligadas {}, Estoque 3 {}",
            on(now.web),
            on(now.postgres),
            on(now.no_status),
            on(now.labels_off),
            on(now.stock_three)
        )];
        shown.extend(put_together(now));
        let intro = "Dois módulos montados em código: Estoque diz onde roda, o banco e o que pede ao host, \
                     e Etiquetas aceita só o Estoque 2. O que não combina é recusado ao montar.";
        let view = layout(cx, intro, buttons, shown, said.get());
        cx.child(view.as_ref())
    }
}

/// Registers the screen with the session.
pub fn register(session: &Session) {
    session.register_control(ID, || Box::new(ModulesScreen) as AnyView);
}
