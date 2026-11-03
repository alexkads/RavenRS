//! Goes through every demonstration of the center, as a person would. If something in the framework
//! stops working, it shows here.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{ActionInput, Object, ObjectSpace, Query, SpaceError, Value};
use ravenrs_app_ui::Session;
use ravenrs_core::{AccessAction, Role, Size};
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

use feature_center::seed::PASSWORD;
use feature_center::{memory_database, watched};

type Space = Rc<dyn Fn() -> Box<dyn ObjectSpace>>;

struct Center {
    ui: Ui,
    session: Rc<RefCell<Option<Session>>>,
    space: Space,
}

impl Center {
    fn over(space: Space, name: &str) -> Self {
        let session = Rc::new(RefCell::new(None));
        let (keep, making) = (session.clone(), space.clone());
        let app = watched(move || making(), move |seen| *keep.borrow_mut() = Some(seen.clone()));
        let mut ui = Ui::new(app.build().expect("it puts together"), Theme::raven_dark());
        ui.set_viewport(Size::new(1600.0, 1200.0), 1.0);
        let mut center = Self { ui, session, space };
        center.sign_in(name);
        center
    }

    fn memory(name: &str) -> Self {
        let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
        Self::over(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>), name)
    }

    fn sign_in(&mut self, name: &str) {
        self.type_into("Nome", name);
        self.type_into("Senha", PASSWORD);
        self.press("Entrar");
        assert!(self.session.borrow().is_some(), "{name} got in: {:?}", self.texts());
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn session(&self) -> Session {
        self.session.borrow().clone().expect("signed in")
    }

    fn texts(&mut self) -> Vec<String> {
        self.settle();
        self.ui
            .access_tree()
            .nodes
            .iter()
            .flat_map(|node| [node.name.clone(), node.value.clone(), node.description.clone()])
            .flatten()
            .collect()
    }

    fn says(&mut self, text: &str) -> bool {
        self.texts().iter().any(|found| found.contains(text))
    }

    fn press(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }

    fn type_into(&mut self, field: &str, text: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
            .unwrap_or_else(|| panic!("no field {field}: {:?}", self.texts()))
            .id;
        self.ui.perform_action(id, AccessAction::Focus);
        self.settle();
        for character in text.chars() {
            self.ui.handle_event(&ravenrs_core::Event::KeyDown {
                key: ravenrs_core::Key::Character(character.to_string()),
                modifiers: ravenrs_core::Modifiers::default(),
            });
        }
        self.settle();
    }

    fn stored(&self, entity: &str, id: f64) -> Object {
        (self.space)().find(entity, &Value::Number(id)).expect("reads").expect("there")
    }

    fn open_list(&mut self, entity: &str) {
        self.session().open_view(&format!("{entity}_ListView"));
        self.settle();
    }

    fn open_record(&mut self, entity: &str, id: f64) {
        self.session().open_detail(entity, &Value::Number(id));
        self.settle();
    }

    fn set(&mut self, field: &str, value: impl Into<Value>) {
        self.session().set_form_value(field, value.into());
        self.settle();
    }

    fn run(&mut self, action: &str, input: ActionInput) {
        let session = self.session();
        let found = session.frame().action(action).unwrap_or_else(|| panic!("no action {action}"));
        session.run_action(&found, input);
        self.settle();
    }

    fn save(&mut self) {
        self.run("Save", ActionInput::None);
    }

    fn select(&mut self, entity: &str, id: f64) {
        let record = self.stored(entity, id);
        self.session().frame().set_selected(vec![record]);
        self.settle();
    }

    fn lines_of_trail(&self, entity: &str) -> Vec<(String, Value)> {
        (self.space)()
            .query(ravenrs_app::SECURITY_AUDIT_ENTRY, &Query::all())
            .expect("reads")
            .into_iter()
            .filter(|line| line.get("target_entity") == Value::Text(entity.to_owned()))
            .map(|line| {
                let change = match line.get("change") {
                    Value::Text(change) => change,
                    _ => String::new(),
                };
                (change, line.get("new_value"))
            })
            .collect()
    }
}

/// Every list of the navigation, with what it is there to show.
const DEMONSTRATIONS: [(&str, &str); 19] = [
    ("Regras", "ValidationRules"),
    ("Avisos e informações", "ValidationSoft"),
    ("Ao sair do campo", "ValidationOnLeave"),
    ("Regras de aparência", "AppearanceRules"),
    ("Tipos de Action", "ActionDemo"),
    ("Todos os tipos", "EditorsDemo"),
    ("Campos personalizados", "CustomFieldsDemo"),
    ("Uma lista, cinco visões", "ListEditorsDemo"),
    ("Variantes e filtros", "VariantsDemo"),
    ("Edição na lista", "EditableList"),
    ("Muitas linhas", "ManyRows"),
    ("Coleções e vínculos", "RelationOwner"),
    ("Máquina de estados", "StateDemo"),
    ("Lembretes", "ReminderDemo"),
    ("Documentos por papel", "SecuredDocument"),
    ("Completa", "AuditFull"),
    ("Leve", "AuditLight"),
    ("Só criação", "AuditCreation"),
    ("Coleções", "AuditOwner"),
];

fn every_demonstration_opens(center: &mut Center) {
    for (caption, _) in DEMONSTRATIONS {
        center.press(caption);
        assert!(center.session().error().is_none(), "{caption}: {:?}", center.session().error());
        assert!(center.says(caption), "{caption} shows its name");
    }
}

#[test]
fn the_same_center_answers_as_a_web_api_with_the_same_permissions() {
    use ravenrs_app_server::ApiRequest;
    use serde_json::json;
    let mut api = feature_center::api_service(None).expect("it starts");
    let mut token_of = |name: &str| {
        let body = json!({ "name": name, "password": PASSWORD });
        let answer = api.handle(ApiRequest::new("POST", "/api/auth/token").json(&body));
        assert_eq!(answer.status, 200, "{answer:?}");
        answer.body["token"].as_str().expect("a token").to_owned()
    };
    let (bia, caio) = (token_of("bia"), token_of("caio"));
    let documents = ApiRequest::new("GET", "/api/data/SecuredDocument").bearer(&bia);
    let answer = api.handle(documents);
    assert_eq!(answer.body["items"].as_array().expect("records").len(), 3);
    assert_eq!(answer.body["items"][0]["cost"], serde_json::Value::Null, "bia does not see the cost");
    let answer = api.handle(ApiRequest::new("GET", "/api/data/SecuredDocument").bearer(&caio));
    assert_eq!(answer.body["items"].as_array().expect("records").len(), 2, "nor caio what is of ana");
    let write =
        ApiRequest::new("PATCH", "/api/data/SecuredDocument/1").bearer(&bia).json(&json!({ "name": "x" }));
    assert_eq!(api.handle(write).status, 403, "it is of ana");
    let form = ApiRequest::new("POST", "/api/data/ValidationRules").bearer(&bia).json(&json!({ "age": 10 }));
    assert_eq!(api.handle(form).status, 422);
}

#[test]
fn each_person_finds_the_theme_they_left() {
    let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
    let space: Space = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
    let mut ana = Center::over(space.clone(), "ana");
    ana.ui.theme_state().set(Theme::raven_light());
    ana.settle();
    let mut again = Center::over(space.clone(), "ana");
    again.settle();
    assert_eq!(again.ui.theme().name, Theme::raven_light().name);
    let mut bia = Center::over(space, "bia");
    bia.settle();
    assert_eq!(bia.ui.theme().name, Theme::raven_dark().name);
}

#[test]
fn the_token_editor_changes_the_theme_of_the_whole_center() {
    let mut center = Center::memory("ana");
    center.press("Editor de tokens");
    assert!(center.says("Sem alterações"));
    center.ui.theme_state().update(|theme| theme.radii.md = 2.0);
    assert!(center.says("1 token alterado"));
    center.press("Restaurar");
    assert!(center.says("Sem alterações"));
}

#[test]
fn every_demonstration_opens_in_memory() {
    every_demonstration_opens(&mut Center::memory("ana"));
}

#[test]
fn every_demonstration_opens_in_sqlite_and_what_was_done_is_there_the_next_time() {
    let folder = std::env::temp_dir().join(format!("raven-centro-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder");
    let file = folder.join("centro.db");
    let _ = std::fs::remove_file(&file);
    let open = |file: std::path::PathBuf| -> Space {
        let database = feature_center::sqlite_database(&file).unwrap_or_else(|why| panic!("{why}"));
        Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>)
    };
    let mut center = Center::over(open(file.clone()), "ana");
    every_demonstration_opens(&mut center);
    center.open_record("AuditFull", 1.0);
    center.set("name", "Mudou");
    center.save();
    assert!(!center.lines_of_trail("AuditFull").is_empty(), "the trail is in the database");
    drop(center);

    let center = Center::over(open(file.clone()), "ana");
    assert_eq!(center.stored("AuditFull", 1.0).get("name"), Value::Text("Mudou".into()));
    assert_eq!(
        (center.space)().query("AuditFull", &Query::all()).expect("reads").len(),
        1,
        "opened again, it is not filled again"
    );
    drop(center);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn validation_refuses_warns_and_notes() {
    let mut c = Center::memory("ana");
    c.open_record("ValidationRules", 1.0);
    c.set("age", 10.0);
    c.set("email", "sem arroba");
    c.set("code", "B2");
    c.save();
    assert_eq!(c.stored("ValidationRules", 1.0).get("age"), Value::Number(30.0), "not kept");
    assert!(c.says("este e-mail não parece certo"), "a message that says what the record holds");

    c.run("Cancel", ActionInput::None);
    c.open_record("ValidationSoft", 1.0);
    c.set("amount", 50.0);
    c.save();
    assert_eq!(c.stored("ValidationSoft", 1.0).get("amount"), Value::Number(50.0), "a note does not stop it");
    assert!(c.says("Valor baixo"));
    c.set("discount", 90.0);
    c.save();
    assert!(c.says("Desconto alto"), "a warning asks");
    assert_eq!(c.stored("ValidationSoft", 1.0).get("discount"), Value::Number(10.0));
}

#[test]
fn appearance_follows_the_record() {
    let mut c = Center::memory("ana");
    c.open_record("AppearanceRules", 4.0);
    assert!(!c.says("Não deve aparecer"), "the field goes away");
    c.open_record("AppearanceRules", 1.0);
    assert!(c.says("Tudo certo"));
    c.open_record("AppearanceRules", 3.0);
    let delete = c.session().frame().action("Delete").expect("there");
    assert!(!delete.is_enabled(), "and the action");
}

#[test]
fn every_kind_of_action_runs() {
    let mut c = Center::memory("ana");
    c.open_list("ActionDemo");
    c.select("ActionDemo", 1.0);
    c.run("DemoSimple", ActionInput::None);
    assert!(c.says("Rodou a Action simples"));
    c.run("DemoValue", ActionInput::Parameter(Value::Number(77.0)));
    assert_eq!(c.stored("ActionDemo", 1.0).get("amount"), Value::Number(77.0));
    // The test picks by hand, and the grid has nothing of its own selected to keep.
    c.select("ActionDemo", 1.0);
    c.run("DemoChoice", ActionInput::Choice("high".into()));
    assert!(c.session().error().is_none(), "{:?}", c.session().error());
    assert_eq!(c.stored("ActionDemo", 1.0).get("priority"), Value::Text("high".into()));
    c.select("ActionDemo", 1.0);
    c.run("DemoConfirm", ActionInput::None);
    assert!(c.says("Somar 1"), "it asks first");
    c.press("Somar");
    assert_eq!(c.stored("ActionDemo", 1.0).get("amount"), Value::Number(78.0));
    c.select("ActionDemo", 1.0);
    let only_open = c.session().frame().action("DemoCriteria").expect("there");
    assert!(only_open.is_enabled());
    c.select("ActionDemo", 3.0);
    assert!(!only_open.is_enabled(), "by what is selected");
    c.open_record("ActionDemo", 1.0);
    assert!(
        c.ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some("Carimbar")),
        "inside the form"
    );
}

#[test]
fn every_editor_shows_and_a_field_from_the_model_is_kept() {
    let mut c = Center::memory("ana");
    c.open_record("EditorsDemo", 1.0);
    for shown in ["Um de cada", "1.234,56", "(81) 98888-7777", "Médio", "42"] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    assert!(matches!(c.stored("EditorsDemo", 1.0).get("code"), Value::Text(_)), "a GUID was made");
    c.open_record("CustomFieldsDemo", 1.0);
    assert!(c.says("Varejo"));
    assert!(c.says("42"), "and one the model calculates");
    c.set("segment", "Atacado");
    c.save();
    assert_eq!(c.stored("CustomFieldsDemo", 1.0).get("segment"), Value::Text("Atacado".into()));
}

#[test]
fn one_list_shows_every_way() {
    let mut c = Center::memory("ana");
    c.open_list("ListEditorsDemo");
    for (way, sign) in
        [("chart", "7.000"), ("month", "Fundação"), ("timeline", "Bia Rocha"), ("tree", "Telhado")]
    {
        c.run("ViewVariants", ActionInput::Choice(way.into()));
        assert!(c.says(sign), "{way}: {:?}", c.texts());
        assert_eq!(
            c.ui.access_tree().nodes.iter().filter(|n| n.role == Role::Cell).count(),
            0,
            "{way} has no grid"
        );
    }
    c.run("ViewVariants", ActionInput::Choice("grid".into()));
    assert!(c.ui.access_tree().nodes.iter().any(|n| n.role == Role::Cell));
}

#[test]
fn variants_filters_and_a_function_of_the_application() {
    let mut c = Center::memory("ana");
    c.open_list("VariantsDemo");
    c.run("ViewVariants", ActionInput::Choice("active".into()));
    assert!(c.says("Alfa") && !c.says("Gama"));
    c.run("ViewVariants", ActionInput::Choice("all".into()));
    c.run("SetFilter", ActionInput::Choice("even".into()));
    assert!(c.says("Beta"), "120 is even");
    assert!(!c.says("Delta"), "75 is not");
}

#[test]
fn many_rows_come_a_few_at_a_time() {
    let mut c = Center::memory("ana");
    c.open_list("ManyRows");
    assert!(c.says("Linha 1"));
    let cells = c.ui.access_tree().nodes.iter().filter(|n| n.role == Role::Cell).count();
    assert!(cells < 1500, "only what shows is built: {cells}");
}

#[test]
fn relations_list_link_and_clone() {
    let mut c = Center::memory("ana");
    c.open_record("RelationOwner", 1.0);
    for shown in ["Parafuso", "Só do tipo X"] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    let tab =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Tab && n.name.as_deref() == Some("Etiquetas"))
            .map(|n| n.id);
    c.ui.perform_action(tab.expect("a tab for the other list"), AccessAction::Click);
    assert!(c.says("Ligada"), "{:?}", c.texts());
    assert!(!c.says("Solta"), "only what points here");
    c.open_list("RelationOwner");
    c.select("RelationOwner", 1.0);
    c.run("CloneObject", ActionInput::None);
    c.save();
    let items = (c.space)().query("RelationItem", &Query::all()).expect("reads");
    assert_eq!(items.len(), 2, "the items that belong to it go along");
    let labels = (c.space)().query("RelationLabel", &Query::all()).expect("reads");
    assert_eq!(labels.len(), 2, "and what only points to it stays");
}

#[test]
fn a_form_of_what_no_storage_keeps_works_out_its_values() {
    let mut c = Center::memory("ana");
    c.press("Simulação de parcelas");
    c.set("amount", 1200.0);
    c.set("months", 12.0);
    assert!(c.says("100"), "{:?}", c.texts());
    c.save();
    assert!(c.session().error().is_none(), "{:?}", c.session().error());
}

#[test]
fn states_go_only_by_their_ways_with_or_without_a_screen() {
    let mut c = Center::memory("ana");
    c.open_list("StateDemo");
    c.select("StateDemo", 1.0);
    let review = c.session().frame().action("StateDemo.state.draft.review").expect("there");
    let approve = c.session().frame().action("StateDemo.state.review.done").expect("there");
    assert!(review.is_enabled() && !approve.is_enabled());
    c.run("StateDemo.state.draft.review", ActionInput::None);
    assert_eq!(c.stored("StateDemo", 1.0).get("state"), Value::Text("review".into()));
    c.select("StateDemo", 1.0);
    assert!(approve.is_enabled());
    c.run("StateDemo.state.review.done", ActionInput::None);
    assert_eq!(c.stored("StateDemo", 1.0).get("state"), Value::Text("done".into()));

    let mut space = (c.space)();
    let mut record = space.find("StateDemo", &Value::Number(1.0)).expect("reads").expect("there");
    record.set("state", "draft");
    space.save(record).expect("waits");
    assert!(matches!(space.commit(), Err(SpaceError::Refused(_))), "no way back, with no screen around");
}

#[test]
fn a_reminder_is_on_its_way_and_comes_up_when_it_is_due() {
    let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
    let source = database.clone();
    let mut c = Center::over(Rc::new(move || Box::new(source.space()) as Box<dyn ObjectSpace>), "ana");
    assert!(
        !c.says("Ligar para o cliente")
            || c.ui.access_tree().nodes.iter().all(|n| n.name.as_deref() != Some("Dispensar"))
    );
    assert!(c.ui.next_wake().is_some(), "it asks to be built again at the time due");
    let mut space = database.space();
    let mut record = space.find("ReminderDemo", &Value::Number(1.0)).expect("reads").expect("there");
    let now = ravenrs_app::local_now();
    record.set(
        "remind_at",
        Value::Date(ravenrs_criteria::DateTime { seconds: now.seconds.saturating_sub(60), ..now }),
    );
    space.save(record).expect("waits");
    space.commit().expect("keeps");
    c.session().records_changed();
    c.settle();
    c.press("Dispensar");
    assert_eq!(c.stored("ReminderDemo", 1.0).get("remind_at"), Value::Null);
}

#[test]
fn each_person_sees_and_does_what_their_role_says() {
    let mut ana = Center::memory("ana");
    assert!(ana.says("Usuários") && ana.says("Trilha de auditoria") && ana.says("Modelo da aplicação"));
    ana.open_list("SecuredDocument");
    assert!(ana.says("Da ana") && ana.says("R$ 900"));

    let mut bia = Center::memory("bia");
    assert!(!bia.says("Usuários"), "people are for administrators");
    assert!(!bia.says("Modelo da aplicação"));
    bia.open_list("SecuredDocument");
    assert!(bia.says("Da ana"), "she reads every document");
    assert!(!bia.says("R$ 900"), "and no cost");
    bia.open_record("SecuredDocument", 2.0);
    assert!(bia.says("Protegido"));
    bia.set("subject", "Mudei");
    bia.save();
    assert_eq!(bia.stored("SecuredDocument", 2.0).get("subject"), Value::Text("Mudei".into()));
    assert_eq!(bia.stored("SecuredDocument", 2.0).get("cost"), Value::Number(50_000.0), "the cost stays");
    bia.open_record("SecuredDocument", 1.0);
    bia.set("subject", "Da ana agora é meu");
    bia.save();
    assert_eq!(
        bia.stored("SecuredDocument", 1.0).get("subject"),
        Value::Text("Contrato".into()),
        "not hers to change"
    );
    assert!(bia.session().error().is_some() || bia.says("permissão"));

    let mut caio = Center::memory("caio");
    assert!(!caio.says("Completa"), "a group he has no way to");
    caio.open_list("SecuredDocument");
    assert!(!caio.says("Da ana"), "not there for him");
    assert!(caio.says("Da bia"));
    let new = caio.session().frame().action("New").expect("there");
    assert!(!new.is_enabled(), "he only reads");
    caio.open_list("ActionDemo");
    let simple = caio.session().frame().action("DemoSimple").expect("there");
    assert!(!simple.is_active(), "and that action is not there for him");
}

#[test]
fn each_mode_of_the_trail_writes_what_it_says() {
    let mut c = Center::memory("ana");
    for entity in ["AuditFull", "AuditLight", "AuditCreation", "AuditFields"] {
        c.open_record(entity, 1.0);
        c.set("name", "Novo nome");
        c.set("ignored", "Mudou");
        c.save();
    }
    let full = c.lines_of_trail("AuditFull");
    assert_eq!(full.len(), 2, "{full:?}");
    assert!(full.iter().any(|(_, new)| *new == Value::Text("Novo nome".into())));
    let light = c.lines_of_trail("AuditLight");
    assert_eq!(light.len(), 1, "{light:?}");
    assert_eq!(light[0].1, Value::Null);
    assert!(c.lines_of_trail("AuditCreation").is_empty());
    assert_eq!(c.lines_of_trail("AuditFields").len(), 1, "the field left out is not there");

    c.open_record("AuditPart", 1.0);
    c.set("name", "Item mudado");
    c.save();
    let owner = c.lines_of_trail("AuditOwner");
    assert!(owner.iter().any(|(change, _)| change == "collection_object_changed"), "{owner:?}");
    c.open_list("SecurityAuditEntry");
    assert!(c.says("Novo nome") || c.says("ana"));
}

#[test]
fn reports_and_panels_show_what_the_person_can_read() {
    let mut c = Center::memory("ana");
    c.open_list("ReportDefinition");
    let report =
        (c.space)().query(ravenrs_app_report::REPORT_DEFINITION, &Query::all()).expect("reads").remove(0);
    c.session().frame().set_selected(vec![report]);
    c.settle();
    c.run("ShowReport", ActionInput::None);
    assert!(c.says("Fundação"), "{:?}", c.texts());
    c.press("Fechar");
    c.open_list("ListEditorsDemo");
    c.run("ShowInReport", ActionInput::None);
    assert!(c.says("Telhado"));
    c.press("Fechar");
    c.open_list("AnalyticsDashboard");
    let panel =
        (c.space)().query(ravenrs_app_report::ANALYTICS_DASHBOARD, &Query::all()).expect("reads").remove(0);
    c.session().frame().set_selected(vec![panel]);
    c.settle();
    c.run("OpenPanel", ActionInput::None);
    assert!(c.says("10.000,00"), "{:?}", c.texts());
}

#[test]
fn the_model_editor_changes_a_caption_for_the_administrator_only() {
    let mut c = Center::memory("ana");
    c.session()
        .set_model_setting(&["NavigationItems", "Validação", "ValidationRules"], "Caption", "Regras mudadas")
        .expect("changed");
    c.settle();
    assert!(c.says("Regras mudadas"));
    c.session().restore_model_settings().expect("restored");
    let caio = Center::memory("caio");
    assert!(caio.session().set_model_setting(&["NavigationItems"], "Caption", "x").is_err());
}
