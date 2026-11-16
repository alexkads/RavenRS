//! The model as code reads it, every feature of the types it is made of that a screen does not
//! show by itself: an entity checked, its fields and what expressions may say about it, calculated
//! values worked out and taken away, what the appearance rules say in each view, a declared action
//! turned into one, where an action reaches, the nodes of the model and a query of a few fields.

use std::collections::BTreeMap;

use ravenrs::prelude::*;
use ravenrs_app::{
    Appearance, AppearanceTarget, DashboardInfo, EntityInfo, FieldInfo, FieldKind, FrameContext, ModelNode,
    Nesting, Object, Query, RecordAction, TargetScope, Value, ViewInfo, ViewKind,
};
use ravenrs_app_ui::{App, Session};

use super::modulos::{button, layout};
use super::screen;

/// The control's name, as the session knows it.
pub const ID: &str = "ModelInsideDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "O modelo por dentro";
/// The group of the navigation of what is written in code over the model.
pub const GROUP: &str = "Modelo";

/// The dashboards of this part.
pub fn dashboards() -> Vec<DashboardInfo> {
    vec![screen(ID, PAGE, GROUP, "code")]
}

/// The controls of [`dashboards`].
pub fn register(session: &Session) {
    let session = session.clone();
    session.clone().register_control(ID, move || Box::new(ModelInside(session.clone())) as AnyView);
}

/// What this part adds to the application: nothing, it only reads it.
pub fn extend(app: App) -> App {
    app
}

/// What each part of the screen says, from the model of the center.
struct ModelInside(Session);

fn yes(on: bool) -> &'static str {
    if on { "sim" } else { "não" }
}

fn tint(appearance: &Appearance) -> String {
    let name = |tint: Option<ravenrs_app::Tint>| {
        tint.map_or_else(|| "nenhuma".to_owned(), |tint| format!("{tint:?}"))
    };
    format!(
        "texto {}, fundo {}, negrito {}",
        name(appearance.font),
        name(appearance.back),
        yes(appearance.bold)
    )
}

impl ModelInside {
    /// An entity checked, a field found by name, and an expression checked against its schema.
    fn entity(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(rules) = self.0.entity("ValidationRules") {
            let checked = rules.check().map_or_else(|why| why.0, |()| "sem problemas".to_owned());
            lines.push(format!("ValidationRules, conferida: {checked}"));
            if let Some(age) = rules.field_named("age") {
                lines.push(format!("O campo age se chama {}", age.caption));
            }
            let expression = "[age] >= 18 And [idade] < 65";
            let problems = ravenrs_criteria::Criteria::parse(expression).map_or_else(
                |why| vec![why.to_string()],
                |parsed| {
                    ravenrs_criteria::validate(&parsed, &rules.schema())
                        .iter()
                        .map(ToString::to_string)
                        .collect()
                },
            );
            lines.push(format!("{expression}, pelo esquema da entidade: {}", problems.join("; ")));
        }
        let keyless = EntityInfo::new("SemChave").field(FieldInfo::new("name", FieldKind::Text));
        let refused = keyless.check().map_or_else(|why| why.0, |()| "aceita".to_owned());
        lines.push(format!("Uma entidade sem chave, conferida: {refused}"));
        lines
    }

    /// A record of the simulation, whose installment is worked out and taken away again.
    fn calculated(&self) -> Vec<String> {
        let Some(simulation) = self.0.entity("Simulation") else { return Vec::new() };
        let values = BTreeMap::from([
            ("amount".to_owned(), Value::Number(1200.0)),
            ("months".to_owned(), Value::Number(12.0)),
            ("owner.name".to_owned(), Value::Text("Ana".into())),
        ]);
        let mut record = Object::from_values("Simulation", values);
        simulation.compute(&mut record, &ravenrs_app::environment());
        let names = |record: &Object| record.values().keys().cloned().collect::<Vec<_>>().join(", ");
        let mut lines = vec![format!(
            "Simulação de 1.200 em 12 meses: parcela {}, com {}",
            record.get("installment").to_text(),
            names(&record)
        )];
        let mut kept = record.clone().without_paths();
        lines.push(format!("Sem o que veio por caminho: {}", names(&kept)));
        simulation.strip_calculated(&mut kept);
        lines.push(format!("Como fica guardado, sem o calculado: {}", names(&kept)));
        kept.keep_only(&["amount"]);
        lines.push(format!("Só com o valor: {}", names(&kept)));
        lines
    }

    /// What the rules of Regras de aparência say about a late record, in the list and in the form.
    fn appearance(&self) -> Vec<String> {
        let (Some(entity), model) = (self.0.entity("AppearanceRules"), self.0.model()) else {
            return Vec::new();
        };
        let late = Object::from_values(
            "AppearanceRules",
            BTreeMap::from([
                ("name".to_owned(), Value::Text("Atrasado".into())),
                ("state".to_owned(), Value::Text("late".into())),
            ]),
        );
        ["ListView", "DetailView"]
            .into_iter()
            .filter_map(|kind| ViewInfo::from_model(&model, &format!("AppearanceRules_{kind}")).ok())
            .map(|view| {
                let shown = entity.appearance_of(AppearanceTarget::Item, "name", &view, Some(&late));
                let place = if view.kind == ViewKind::List { "Na lista" } else { "No formulário" };
                format!("{place}, o nome do atrasado: {}", tint(&shown))
            })
            .collect()
    }

    /// A declared action, its id and the action it turns into, and where actions reach.
    fn actions(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(places) = self.0.entity("FieldPlacesDemo") {
            let archive =
                RecordAction::new("Arquivar").set("archived = True").confirm("Arquivar os selecionados?");
            let action = archive.to_action(&places);
            lines.push(format!(
                "A Action declarada Arquivar tem o id {} e vira a Action {}, que pergunta {}",
                archive.id_on(&places),
                action.caption(),
                action.confirmation_text().unwrap_or_default()
            ));
        }
        let model = self.0.model();
        let view = |id: &str| ViewInfo::from_model(&model, id).ok();
        let lists = TargetScope::any().entity("ActionDemo").view_kind(ViewKind::List).nesting(Nesting::Root);
        let only = TargetScope::any().view_id("VariantsDemo_ListView");
        let cases = [
            (
                "só nas listas de Tipos de Action, na lista",
                &lists,
                view("ActionDemo_ListView"),
                FrameContext::Main,
            ),
            (
                "só nas listas de Tipos de Action, no formulário",
                &lists,
                view("ActionDemo_DetailView"),
                FrameContext::Main,
            ),
            (
                "só nas listas de Tipos de Action, dentro de outro",
                &lists,
                view("ActionDemo_ListView"),
                FrameContext::Nested,
            ),
            ("só na lista de Variantes, nela", &only, view("VariantsDemo_ListView"), FrameContext::Main),
            (
                "só na lista de Variantes, em Regras",
                &only,
                view("ValidationRules_ListView"),
                FrameContext::Main,
            ),
        ];
        for (what, scope, view, context) in cases {
            lines.push(format!("Alcance {what}: {}", yes(scope.covers(view.as_ref(), context))));
        }
        lines
    }

    /// The nodes of the model: the group Validação as the navigation orders it, and one made here.
    fn nodes(&self) -> Vec<String> {
        let model = self.0.model();
        let mut lines = Vec::new();
        if let Some(group) = model.at(&["NavigationItems", "Validação"]) {
            let items: Vec<String> = group
                .in_order()
                .iter()
                .map(|item| item.typed("Caption").unwrap_or_else(|| item.id.clone()))
                .collect();
            lines.push(format!("Validação, na ordem da navegação: {}", items.join(", ")));
        }
        let made = ModelNode::new("Exemplo")
            .with("Caption", "Feito em código")
            .with("Index", 3)
            .with("Visible", false);
        lines.push(format!(
            "Um nó feito aqui: {}, índice {}, visível {}, índice como texto {}",
            made.text("Caption").unwrap_or_default(),
            made.integer("Index").unwrap_or_default(),
            yes(made.flag("Visible").unwrap_or(true)),
            made.typed("Index").unwrap_or_default()
        ));
        lines
    }

    /// The most expensive tasks of the plan, reading only their name and cost.
    fn query(&self) -> Vec<String> {
        let space = self.0.frame().space();
        let asked = Query::all().sort("cost", false).only(["name", "cost"]);
        let found = space.borrow().query("ListEditorsDemo", &asked).unwrap_or_default();
        let shown: Vec<String> = found
            .iter()
            .take(3)
            .map(|task| format!("{} ({} campos lidos)", task.get("name").to_text(), task.values().len()))
            .collect();
        vec![format!("Tarefas da mais cara para a mais barata: {}", shown.join(", "))]
    }
}

impl View for ModelInside {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let part = cx.use_state(|| 0_usize);
        let parts: [(&str, fn(&Self) -> Vec<String>); 6] = [
            ("Entidade", Self::entity),
            ("Calculados", Self::calculated),
            ("Aparência", Self::appearance),
            ("Actions declaradas", Self::actions),
            ("Nós do modelo", Self::nodes),
            ("Consulta", Self::query),
        ];
        let buttons = parts
            .iter()
            .enumerate()
            .map(|(at, (caption, _))| {
                let part = part.clone();
                button(caption, move || part.set(at))
            })
            .collect();
        let (caption, read) = parts[part.get().min(parts.len() - 1)];
        let intro = "O que o código lê do modelo do centro, sem tela própria: escolha uma parte.";
        let view = layout(cx, intro, buttons, read(self), format!("Mostrando: {caption}"));
        cx.child(view.as_ref())
    }
}
