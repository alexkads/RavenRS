//! Goes through every demonstration of the center, as a person would. If something in the framework
//! stops working, it shows here.
#![allow(clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{ActionInput, Object, ObjectSpace, Query, SpaceError, Value};
use ravenrs_app_ui::Session;
use ravenrs_core::{AccessAction, Primitive, Quad, Role, Size, Toggled};
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::Ui;

use raven_feature_center::seed::PASSWORD;
use raven_feature_center::{memory_database, watched};

type Space = Rc<dyn Fn() -> Box<dyn ObjectSpace>>;

struct Center {
    ui: Ui,
    session: Rc<RefCell<Option<Session>>>,
    space: Space,
}

impl Center {
    fn over(space: Space, name: &str) -> Self {
        let mut center = Self::at_the_door(space);
        center.sign_in(name);
        center
    }

    /// The center on its sign-in screen, with nobody in yet.
    fn at_the_door(space: Space) -> Self {
        let session = Rc::new(RefCell::new(None));
        let (keep, making) = (session.clone(), space.clone());
        let app = watched(move || making(), move |seen| *keep.borrow_mut() = Some(seen.clone()));
        let mut ui = Ui::new(app.build().expect("it puts together"), Theme::raven_dark());
        ui.set_viewport(Size::new(1600.0, 1200.0), 1.0);
        Self { ui, session, space }
    }

    /// Whether `name` gets in with `password`.
    fn gets_in(&mut self, name: &str, password: &str) -> bool {
        self.type_into("Nome", name);
        self.type_into("Senha", password);
        self.press("Entrar");
        self.session.borrow().is_some()
    }

    fn key(&mut self, key: ravenrs_core::Key) {
        self.ui.handle_event(&ravenrs_core::Event::KeyDown {
            key,
            modifiers: ravenrs_core::Modifiers::default(),
        });
        self.settle();
    }

    /// The name of what has the keyboard focus.
    fn focused(&mut self) -> Option<String> {
        self.settle();
        let tree = self.ui.access_tree();
        tree.focus.and_then(|id| tree.nodes.iter().find(|n| n.id == id)).and_then(|n| n.name.clone())
    }

    fn count(&self, entity: &str) -> usize {
        (self.space)().query(entity, &Query::all()).expect("reads").len()
    }

    fn memory(name: &str) -> Self {
        let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
        Self::over(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>), name)
    }

    fn sign_in(&mut self, name: &str) {
        let inside = self.gets_in(name, PASSWORD);
        assert!(inside, "{name} got in: {:?}", self.texts());
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
const DEMONSTRATIONS: [(&str, &str); 21] = [
    ("Regras", "ValidationRules"),
    ("Avisos e informações", "ValidationSoft"),
    ("Ao sair do campo", "ValidationOnLeave"),
    ("Regras de aparência", "AppearanceRules"),
    ("Tipos de Action", "ActionDemo"),
    ("Todos os tipos", "EditorsDemo"),
    ("Campos personalizados", "CustomFieldsDemo"),
    ("Textos por chave", "TextsDemo"),
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
    ("Dados da loja", "StoreSettings"),
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
    let mut api = raven_feature_center::api_service(None).expect("it starts");
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
        let database = raven_feature_center::sqlite_database(&file).unwrap_or_else(|why| panic!("{why}"));
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
    c.run("DemoBackground", ActionInput::None);
    let start = std::time::Instant::now();
    while !c.session().frame().jobs().is_empty() {
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "the work never ended");
        std::thread::sleep(std::time::Duration::from_millis(5));
        c.settle();
    }
    assert_eq!(c.stored("ActionDemo", 1.0).get("amount"), Value::Number(88.0), "kept from its own thread");
    assert!(c.says("Somou 10 em segundo plano."));
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
fn a_declared_action_asks_sets_and_keeps_with_no_code() {
    let mut c = Center::memory("ana");
    c.open_list("ActionDemo");
    c.select("ActionDemo", 2.0);
    c.run("DemoDeclared", ActionInput::None);
    assert!(c.says("Fechar e dobrar o valor dos selecionados?"), "it asks first");
    // #160: it can be changed again, so the question is plain, not the one of deleting.
    assert!(c.says("Confira antes de continuar."), "{:?}", c.texts());
    assert!(!c.says("Não será possível desfazer"), "{:?}", c.texts());
    c.press("Fechar e dobrar");
    assert!(c.session().error().is_none(), "{:?}", c.session().error());
    assert_eq!(c.stored("ActionDemo", 2.0).get("state"), Value::Text("closed".into()));
    assert_eq!(c.stored("ActionDemo", 2.0).get("amount"), Value::Number(400.0));
    c.select("ActionDemo", 2.0);
    let declared = c.session().frame().action("DemoDeclared").expect("there");
    assert!(!declared.is_enabled(), "only for what is open");
}

#[test]
fn texts_by_key_replace_words_and_add_the_center_s_own() {
    let mut c = Center::memory("ana");
    c.open_list("TextsDemo");
    assert!(c.says("Recarregar"), "a word of the screens the center replaced: {:?}", c.texts());
    assert!(!c.texts().iter().any(|text| text == "Atualizar"), "and the one it replaced is gone");
    c.select("TextsDemo", 1.0);
    c.run("DemoHello", ActionInput::None);
    assert!(c.says("Olá, Ana! Este texto veio do catálogo."), "a key of its own: {:?}", c.texts());
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
    let (report, _) = stored_report(&c, "Custos por tarefa");
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

/// Draws a list in each theme, to look at a change on the real screens:
/// `FC_SCREENSHOTS=folder cargo test -p raven-feature-center --test tour draw_the_lists`.
#[test]
fn draw_the_lists() {
    use ravenrs_render::{GpuContext, Renderer, wgpu};
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
    let Some(folder) = std::env::var_os("FC_SCREENSHOTS") else { return };
    let folder = std::path::Path::new(&folder);
    std::fs::create_dir_all(folder).expect("folder");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(gpu) = pollster::block_on(GpuContext::new(&instance, None)) else { return };
    let (width, height, scale) = (1360_u32, 860_u32, 2.0_f32);
    let mut c = Center::memory("ana");
    c.ui.set_viewport(Size::new(width as f32, height as f32), scale);
    c.open_list("ManyRows");
    let (w, h) = (width * 2, height * 2);
    for (name, theme) in
        [("claro", Theme::raven_light()), ("escuro", Theme::raven_dark()), ("vidro", Theme::raven_glass())]
    {
        c.ui.set_theme(theme);
        c.settle();
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screen"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Renderer::new(&gpu.device, &gpu.queue, FORMAT).render(c.ui.scene(), &target, (w, h), scale);
        let padded =
            (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = slice.get_mapped_range().expect("mapped range");
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        let file = folder.join(format!("lista-{name}.png"));
        image::RgbaImage::from_raw(w, h, pixels).expect("image").save(file).expect("save");
    }
}

#[test]
fn a_record_that_is_one_of_a_kind_opens_on_its_form() {
    let mut c = Center::memory("ana");
    c.press("Dados da loja");
    assert!(c.session().form_record().is_some(), "a new one, while there is none");
    c.set("name", "Loja de exemplo");
    c.save();
    c.open_list("ReportDefinition");
    c.press("Dados da loja");
    let record = c.session().form_record().expect("the form again, with no list");
    assert_eq!(record.get("name"), Value::Text("Loja de exemplo".into()));
}

#[test]
fn the_gtin_editor_checks_the_digit_while_typing() {
    let mut c = Center::memory("ana");
    c.open_record("EditorsDemo", 1.0);
    c.set("gtin", "7891000100103");
    assert!(c.says("EAN-13"), "{:?}", c.texts());
    c.set("gtin", "7891000100104");
    assert!(c.says("\u{2717} GTIN"), "{:?}", c.texts());
}

/// The counter window by itself, with its session and no sign-in.
struct Counter {
    ui: Ui,
    session: Rc<RefCell<Option<Session>>>,
}

impl Counter {
    fn open(change: impl FnOnce(ravenrs_app_ui::App) -> ravenrs_app_ui::App) -> Self {
        let session = Rc::new(RefCell::new(None));
        let keep = session.clone();
        let app = change(raven_feature_center::actions::counter_app())
            .with_session(move |seen| *keep.borrow_mut() = Some(seen.clone()));
        let mut ui = Ui::new(app.build().expect("it puts together"), Theme::raven_dark());
        ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
        let mut counter = Self { ui, session };
        counter.settle();
        counter
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.ui.update();
        }
    }

    fn says(&mut self, text: &str) -> bool {
        self.settle();
        self.ui.access_tree().nodes.iter().any(|node| {
            [&node.name, &node.value, &node.description]
                .iter()
                .any(|found| found.as_deref().is_some_and(|found| found.contains(text)))
        })
    }

    fn key(&mut self, key: ravenrs_core::Key) {
        self.ui.handle_event(&ravenrs_core::Event::KeyDown {
            key,
            modifiers: ravenrs_core::Modifiers::default(),
        });
        self.settle();
    }

    fn function(&mut self, number: u8) {
        self.key(ravenrs_core::Key::Function(number));
    }

    fn type_in(&mut self, text: &str) {
        for letter in text.chars() {
            self.key(ravenrs_core::Key::Character(letter.to_string()));
        }
    }

    fn session(&self) -> Session {
        self.session.borrow().clone().expect("open")
    }
}

#[test]
fn the_counter_window_shows_its_notice_and_keys_inside_the_center_and_nowhere_else() {
    let mut c = Center::memory("ana");
    assert!(!c.says("Este aviso vem do módulo"), "the center has no notice of its own");
    c.press("Caixa de exemplo");
    assert!(c.says("Este aviso vem do módulo"), "{:?}", c.texts());
    assert!(c.says("Próximo número"), "the keys are in the bar: {:?}", c.texts());
    c.press("Dados da loja");
    assert!(!c.says("Este aviso vem do módulo"), "it goes when the window goes");
}

#[test]
fn a_notice_stays_and_cannot_be_dismissed() {
    let mut counter = Counter::open(|app| app);
    assert_eq!(counter.session().notices().len(), 1);
    assert!(counter.says("Demonstração"));
    assert!(!counter.says("Fechar"), "there is no way to close it");
    counter.function(12);
    assert!(counter.says("Demonstração"), "not even a locked window hides it");
}

#[test]
fn the_counter_gives_numbers_in_order_from_the_keyboard() {
    let mut counter = Counter::open(|app| app);
    counter.function(9);
    assert!(counter.says("Venda número 1."));
    counter.function(9);
    assert!(counter.says("Venda número 2."));
}

#[test]
fn a_draft_is_kept_and_comes_back_once() {
    let mut counter = Counter::open(|app| app);
    counter.function(11);
    assert!(counter.says("Nenhuma venda guardada."));
    counter.function(10);
    assert!(counter.says("Venda guardada"));
    counter.function(11);
    assert!(counter.says("Retomou: Venda pela metade"));
    counter.function(11);
    assert!(counter.says("Nenhuma venda guardada."), "it was taken");
}

#[test]
fn alt_and_a_letter_adds_up_in_the_background_with_no_cent_lost() {
    let mut counter = Counter::open(|app| app);
    counter.ui.handle_event(&ravenrs_core::Event::KeyDown {
        key: ravenrs_core::Key::Character("s".into()),
        modifiers: ravenrs_core::Modifiers { alt: true, ..ravenrs_core::Modifiers::default() },
    });
    counter.session().finish_background();
    assert!(counter.says("Total exato: 36.99"), "27.90 + 8.79 + 0.10 + 0.20");
    assert!(counter.session().background_jobs().is_empty());
}

#[test]
fn a_decimal_field_keeps_what_it_was_given() {
    use ravenrs_app::{BusinessObject, FieldValue};
    let info = raven_feature_center::model::CounterProduct::entity_info();
    let price = info.fields.iter().find(|field| field.name == "price").expect("the field");
    assert!(matches!(price.kind, ravenrs_app::FieldKind::Decimal(_)));
    let exact: rust_decimal::Decimal = "0.30".parse().expect("a number");
    assert_eq!(rust_decimal::Decimal::from_value(exact.to_value()), exact.normalize());
}

#[test]
fn the_keys_can_be_listed_and_the_window_locked() {
    let mut counter = Counter::open(|app| app);
    counter.function(1);
    assert!(counter.says("Consulta de atalhos"));
    assert!(counter.says("Bloquear"));
    counter.session().close_presented();
    counter.settle();
    counter.function(12);
    assert!(counter.says("Caixa bloqueado"));
    assert!(!counter.says("Próximo número"), "nothing else shows");
    counter.function(9);
    counter.type_in("0000");
    counter.key(ravenrs_core::Key::Enter);
    assert!(counter.says("Caixa bloqueado"), "a wrong one stays locked");
    counter.type_in(raven_feature_center::actions::COUNTER_PIN);
    counter.key(ravenrs_core::Key::Enter);
    assert!(counter.says("Próximo número"), "the right one gets back");
}

#[test]
fn an_application_can_start_on_a_theme_of_its_own() {
    let mut counter = Counter::open(|app| app.theme(Theme::raven_light()));
    counter.settle();
    assert_eq!(counter.ui.theme().palette.ink, Theme::raven_light().palette.ink);
}

fn stored_report(c: &Center, name: &str) -> (Object, ravenrs_report::Report) {
    let records = (c.space)().query(ravenrs_app_report::REPORT_DEFINITION, &Query::all()).expect("reads");
    let wanted = Value::Text(name.to_owned());
    let record = records.into_iter().find(|record| record.get("name") == wanted).expect("seeded");
    let report = match record.get("layout") {
        Value::Text(layout) if !layout.is_empty() => {
            ravenrs_report::Report::from_json(&layout).expect("reads back")
        }
        _ => ravenrs_report::Report::default(),
    };
    (record, report)
}

#[test]
fn labels_go_in_columns_with_a_mark_for_each_code_and_turn_into_printer_language() {
    let c = Center::memory("ana");
    let (record, report) = stored_report(&c, raven_feature_center::seed::LABELS_REPORT);
    assert!(report.columns.as_ref().is_some_and(|columns| columns.count > 1), "columns on a sheet");
    let mut product = (c.space)().find("EditorsDemo", &Value::Number(1.0)).expect("reads").expect("there");
    product.set("gtin", "7891000100103".to_owned());
    let words = ravenrs_app_report::ReportWords::pt_br();
    let document = ravenrs_app_report::render_stored_report(&c.session(), &record, &[(product, 4)], &words)
        .unwrap_or_else(|why| panic!("{why}"));
    let marks = document.pages[0]
        .items
        .iter()
        .filter(|item| matches!(item, ravenrs_report::Item::BarcodeMark { .. }))
        .count();
    assert_eq!(marks, 4, "each label has its code, one beside the other");
    let job = ravenrs_report_label::LabelJob::new(ravenrs_report_label::Language::Zpl, 203);
    let bytes = ravenrs_report_label::compile(&document, &job).unwrap_or_else(|why| panic!("{why}"));
    assert!(bytes.starts_with(b"^XA"), "ZPL");
}

#[test]
fn a_receipt_is_one_long_page_with_its_printer_and_commands_in_place() {
    let mut c = Center::memory("ana");
    let (record, report) = stored_report(&c, raven_feature_center::seed::RECEIPT_REPORT);
    assert!(report.continuous);
    assert_eq!(report.printer.as_ref().map(|printer| printer.language.as_str()), Some("ESC/POS"));
    let rows = (c.space)().query("EditableList", &Query::all()).expect("reads");
    let many: Vec<(Object, u32)> = rows.into_iter().map(|row| (row, 3)).collect();
    let words = ravenrs_app_report::ReportWords::pt_br();
    let document = ravenrs_app_report::render_stored_report(&c.session(), &record, &many, &words)
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(document.pages.len(), 1, "a roll has one page however long");
    let job = ravenrs_report_label::LabelJob::new(ravenrs_report_label::Language::EscPos, 203);
    let bytes = ravenrs_report_label::compile(&document, &job).unwrap_or_else(|why| panic!("{why}"));
    assert!(bytes.windows(2).any(|pair| pair == [0x1d, b'V']), "the paper is cut");
    assert!(bytes.windows(2).any(|pair| pair == [0x1b, b'p']), "the drawer opens");

    ravenrs_app_report::design_stored_report(&c.session(), &record, &words)
        .unwrap_or_else(|why| panic!("{why}"));
    // Printing and export live on the ribbon's own page, "Saída".
    c.settle();
    let page = c.ui.access_tree().nodes.iter().find(|n| n.name.as_deref() == Some("Saída")).map(|n| n.id);
    c.ui.perform_action(page.expect("the ribbon's page of output"), AccessAction::Click);
    assert!(c.says("Impressora direta"), "the designer offers printing straight: {:?}", c.texts());
}

#[test]
fn leaving_a_field_checks_it_before_saving() {
    let mut c = Center::memory("ana");
    c.press("Ao sair do campo");
    c.open_record("ValidationOnLeave", 1.0);
    c.type_into("Nome", "");
    for _ in "Recife".chars() {
        c.key(ravenrs_core::Key::Backspace);
    }
    assert!(!c.says("obrigatório"), "nothing to say while the field still has the focus: {:?}", c.texts());
    c.type_into("Cidade", "");
    assert!(c.says("obrigatório"), "the field says it as soon as the focus leaves it: {:?}", c.texts());
    assert_eq!(c.stored("ValidationOnLeave", 1.0).get("name"), Value::Text("Recife".into()), "nothing saved");
}

#[test]
fn the_code_panel_counts_what_the_seed_put_there() {
    let mut c = Center::memory("ana");
    c.press("Painel em código");
    for shown in [
        "R$ 10.000,00",
        "Ana Lima: R$ 7.000,00, Bia Rocha: R$ 3.000,00",
        "Aberta: R$ 8.000,00, Atrasada: R$ 2.000,00",
    ] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    let caio = &mut Center::memory("caio");
    caio.press("Painel em código");
    assert!(caio.session().error().is_none(), "{:?}", caio.session().error());
}

#[test]
fn screen_readers_hear_the_model_and_the_check_action_announces_what_it_did() {
    let mut c = Center::memory("ana");
    c.press("Leitores de tela e teclado");
    c.open_record("AccessibilityDemo", 1.0);
    let tree = c.ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Início do período")), "a name of its own");
    assert!(
        tree.nodes.iter().any(|n| n.description.as_deref() == Some("O último dia que entra na conta")),
        "and what it means"
    );
    // The form draws De and Até first, and the model asks for Nome and Valor first.
    c.type_into("Nome", "");
    c.key(ravenrs_core::Key::Tab);
    let second = c.focused().unwrap_or_default();
    c.key(ravenrs_core::Key::Tab);
    let third = c.focused().unwrap_or_default();
    assert!(
        second.starts_with("Valor") && third.starts_with("Início do período"),
        "the order of the model: Nome, {second}, {third}"
    );
    c.open_list("AccessibilityDemo");
    c.select("AccessibilityDemo", 1.0);
    let check = c.session().frame().action("AccessibilityCheck").expect("there");
    assert!(check.hotkey_text().is_some_and(|keys| !keys.is_empty()), "it says its keys");
    c.run("AccessibilityCheck", ActionInput::None);
    assert_eq!(c.stored("AccessibilityDemo", 1.0).get("checked"), Value::Bool(true));
    assert!(c.says("Conferido: Aluguel"), "and says what it did: {:?}", c.texts());
}

#[test]
fn the_ready_made_records_take_new_ones_with_their_own_rules() {
    let mut c = Center::memory("ana");
    c.press("Pessoas");
    assert!(c.says("Ana Lima"), "the full name is worked out: {:?}", c.texts());
    c.run("New", ActionInput::None);
    c.set("first_name", "Caio");
    c.set("last_name", "Dias");
    c.set("email", "sem arroba");
    c.save();
    assert_eq!(c.count("Person"), 2, "an e-mail with no @ is refused");
    c.set("email", "caio@exemplo.com");
    c.save();
    assert_eq!(c.count("Person"), 3, "{:?}", c.session().error());

    for (caption, entity, fields) in [
        ("Países", "Country", &[("name", "Brasil"), ("code", "BR")][..]),
        ("Endereços", "Address", &[("street", "Rua da Aurora"), ("city", "Recife")][..]),
    ] {
        c.press(caption);
        let before = c.count(entity);
        c.run("New", ActionInput::None);
        for (field, value) in fields {
            c.set(field, *value);
        }
        c.save();
        assert_eq!(c.count(entity), before + 1, "{caption}: {:?}", c.session().error());
    }
    c.press("Eventos");
    c.run("New", ActionInput::None);
    c.set("subject", "Reunião de obra");
    let at = |day: i64| {
        let now = ravenrs_app::local_now();
        Value::Date(ravenrs_criteria::DateTime { days: now.days + day, ..now })
    };
    c.set("starts", at(2));
    c.set("ends", at(1));
    c.save();
    assert_eq!(c.count("Event"), 0, "it cannot end before it starts");
    c.set("ends", at(3));
    c.save();
    assert_eq!(c.count("Event"), 1, "{:?}", c.session().error());
    c.press("Países");
    c.run("New", ActionInput::None);
    c.set("name", "Brasil");
    c.save();
    assert_eq!(c.count("Country"), 1, "the name of a country is unique");
}

#[test]
fn the_lists_of_reports_panels_and_roles_open_from_the_navigation() {
    let mut c = Center::memory("ana");
    c.press("Relatórios");
    assert!(c.says("Custos por tarefa"), "{:?}", c.texts());
    c.press("Painéis");
    let panels = (c.space)().query(ravenrs_app_report::ANALYTICS_DASHBOARD, &Query::all()).expect("reads");
    let Value::Text(name) = panels[0].get("name") else { panic!("a panel with a name") };
    assert!(c.says(&name), "{name}: {:?}", c.texts());
    c.press("Papéis");
    for role in ["Administradores", "Vendas", "Leitura"] {
        assert!(c.says(role), "{role}: {:?}", c.texts());
    }
    c.press("Campos de fora");
    assert!(c.says("Fora da auditoria"), "{:?}", c.texts());
}

#[test]
fn a_new_password_lets_in_and_the_old_one_no_longer_does() {
    let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
    let source = database.clone();
    let space: Space = Rc::new(move || Box::new(source.space()) as Box<dyn ObjectSpace>);
    let mut c = Center::over(space.clone(), "bia");
    c.press("Trocar senha");
    c.type_into("Senha atual", PASSWORD);
    c.type_into("Nova senha", "uma-senha-bem-nova");
    c.type_into("Repita a senha", "uma-senha-bem-nova");
    c.press("Salvar senha");
    assert!(c.session().error().is_none(), "{:?}", c.session().error());

    let mut old = Center::at_the_door(space.clone());
    assert!(!old.gets_in("bia", PASSWORD), "the old password is refused");
    let mut new = Center::at_the_door(space);
    assert!(new.gets_in("bia", "uma-senha-bem-nova"), "{:?}", new.texts());
}

/// Hands each request of the window to the center's own web API, at once, as `cargo run --bin api`
/// would over the network.
struct AtOnce(Rc<RefCell<ravenrs_app_server::ApiService>>);

impl ravenrs_app_remote::Transport for AtOnce {
    fn send(
        &self,
        request: ravenrs_app_remote::RemoteRequest,
        done: Box<dyn FnOnce(Result<ravenrs_app_remote::RemoteResponse, String>)>,
    ) {
        let address = request.url.split_once("://").map_or(request.url.as_str(), |(_, rest)| rest);
        let target = address.split_once('/').map_or("", |(_, rest)| rest);
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let mut asked = ravenrs_app_server::ApiRequest::new(&request.method, path);
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            asked = asked.query(name, value);
        }
        if let Some(token) = &request.bearer {
            asked = asked.bearer(token);
        }
        if let Some(body) = &request.body {
            asked.body = body.clone().into_bytes();
        }
        let answer = self.0.borrow_mut().handle(asked);
        done(Ok(ravenrs_app_remote::RemoteResponse::new(answer.status, answer.body.to_string())));
    }
}

#[test]
fn over_a_server_the_counter_window_has_its_control_too() {
    let service = Rc::new(RefCell::new(raven_feature_center::api_service(None).expect("it starts")));
    let client = ravenrs_app_remote::RemoteClient::new("http://centro", AtOnce(service));
    let mut ui = Ui::new(raven_feature_center::remote_over(client).expect("it opens"), Theme::raven_dark());
    ui.set_viewport(Size::new(1600.0, 1200.0), 1.0);
    let mut c = Window(ui);
    c.type_into("Nome", "ana");
    c.type_into("Senha", PASSWORD);
    c.press("Entrar");
    c.press("Caixa de exemplo");
    assert!(c.says("Próximo número"), "the counter is drawn, not an empty panel: {:?}", c.texts());
    c.press("Editor de tokens");
    assert!(c.says("Tokens") || c.says("tokens"), "{:?}", c.texts());
}

/// The window over a server, read and pressed the same way as `Center`.
struct Window(Ui);

impl Window {
    fn texts(&mut self) -> Vec<String> {
        for _ in 0..4 {
            self.0.update();
        }
        self.0
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
        let _ = self.texts();
        let tree = self.0.access_tree();
        let found = tree.nodes.iter().find(|n| n.role == Role::Button && n.name.as_deref() == Some(name));
        let id = found.unwrap_or_else(|| panic!("no button {name}: {:?}", self.texts())).id;
        self.0.perform_action(id, AccessAction::Click);
        let _ = self.texts();
    }

    fn type_into(&mut self, field: &str, text: &str) {
        let _ = self.texts();
        let tree = self.0.access_tree();
        let id = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref().is_some_and(|t| t.starts_with(field)))
            .unwrap_or_else(|| panic!("no field {field}: {:?}", self.texts()))
            .id;
        self.0.perform_action(id, AccessAction::Focus);
        for character in text.chars() {
            self.0.handle_event(&ravenrs_core::Event::KeyDown {
                key: ravenrs_core::Key::Character(character.to_string()),
                modifiers: ravenrs_core::Modifiers::default(),
            });
        }
        let _ = self.texts();
    }
}

impl Center {
    /// Clicks whatever has this name: a switch, a tab, a button.
    fn activate(&mut self, name: &str) {
        self.settle();
        let tree = self.ui.access_tree();
        let found = tree.nodes.iter().find(|n| n.name.as_deref() == Some(name) && n.role != Role::Group);
        let id = found.unwrap_or_else(|| panic!("nothing called {name}: {:?}", self.texts())).id;
        self.ui.perform_action(id, AccessAction::Click);
        self.settle();
    }
}

#[test]
fn the_flow_diagram_shows_every_feature_and_its_connections_hang_as_cables() {
    let mut c = Center::memory("ana");
    c.press("Diagrama de fluxo");
    assert!(c.says("Diagrama de exemplo"), "the label: {:?}", c.texts());
    assert!(c.says("Nós no diagrama: 4"));
    assert_eq!(c.ui.cables().ids().len(), 2, "both connections hang as cables");
    // A node arrives from outside, as from a toolbox.
    c.press("Chegar um nó");
    assert!(c.says("Nós no diagrama: 5"), "{:?}", c.texts());
    // Every option is a switch that the canvas follows.
    for option in [
        "Travado",
        "Mapa",
        "Controles",
        "Encaixar na grade",
        "Portas ao passar",
        "Criação rápida",
        "Redimensionar",
        "Girar",
        "Títulos editáveis",
        "Roda move o diagrama",
    ] {
        c.activate(option);
        assert!(c.says("Diagrama de exemplo"), "{option} keeps the canvas");
        c.activate(option);
    }
    // Enter twice on a port opens the search of what a connection from it can lead to; a start
    // takes nothing in, so it is not offered.
    let before = c.texts().iter().filter(|t| t.starts_with("Nós no diagrama")).cloned().collect::<Vec<_>>();
    let port =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("out"))
            .map(|n| n.id)
            .expect("a port");
    c.ui.perform_action(port, AccessAction::Focus);
    c.key(ravenrs_core::Key::Enter);
    c.key(ravenrs_core::Key::Enter);
    assert!(c.says("Procurar o próximo nó"), "the search opens: {:?}", c.texts());
    assert!(c.says("Etapa de conferência") && !c.says("Outro início"), "only what the port takes");
    c.press("Etapa de conferência");
    assert!(!c.says("Procurar o próximo nó"), "and closes, the node created connected");
    assert!(!c.says(&before[0]), "one node more");
    // A run told from outside: the canvas says how each node did and counts the items.
    c.activate("Execução de fora");
    assert!(c.says("3 itens"), "the count on the line: {:?}", c.texts());
    assert!(c.says("Nós no diagrama: 6"), "the diagram itself is the same");
    c.activate("Execução de fora");
    assert!(!c.says("3 itens"));
    // height: a strip 200 pt tall, as wide as the page.
    c.activate("Faixa de 200 pt");
    assert!(c.says("Nós no diagrama: 6"), "the diagram is the same in the strip");
    // And in a row beside a text, which does not stretch it, it takes the width of its stages.
    let tree = c.ui.access_tree();
    let row = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Estágios do build 12"));
    let wide = row.expect("the strip in a row").bounds.width();
    assert!(wide > 400.0, "a strip in a row is as wide as its stages, not 0: {wide}");
    c.activate("Faixa de 200 pt");
    // Read only: the lock is not offered.
    c.activate("Só leitura");
    assert!(!c.says("Destravar edição"), "no way out of the lock");
    c.activate("Só leitura");
    c.activate("Grade 48");
    c.activate("Mapa mental");
    // Without cables, the lines are plain again.
    c.activate("Ligações como cabos");
    assert!(c.ui.cables().ids().is_empty(), "plain lines");
    c.activate("Ligações como cabos");
    assert_eq!(c.ui.cables().ids().len(), 3, "the two of the sample and the one the search created");
}

#[test]
fn the_data_inspector_shows_the_orders_four_ways_and_keeps_the_field_chosen() {
    let mut c = Center::memory("ana");
    c.press("Inspetor de dados");
    assert!(c.says("1234567890123456789"), "the 19-digit number exact: {:?}", c.texts());
    assert!(c.says("null"), "a null discount");
    assert!(c.says("—"), "and an absent one, told apart");
    c.activate("Árvore");
    assert!(c.says("Item 1 de 3"));
    c.press("Próximo item");
    assert!(c.says("Modo 1 · item 2"), "the item moved");
    c.activate("pedido: 102");
    assert!(c.says("Escolheu pedido"), "the pick is heard as a path");
    assert!(c.says("Arraste pedido para um parâmetro"), "the field chosen can be carried to an expression");
    c.press("Copiar como expressão");
    assert_eq!(c.ui.clipboard().read().as_deref(), Some("[pedido]"), "or copied as one from the keyboard");
    c.activate("Esquema");
    assert!(c.says("→ pedido"), "the field chosen stays chosen");
    assert!(c.says("cliente.cidade"), "a field one order lacks");
    c.activate("JSON");
    assert!(c.says("Campo escolhido: pedido"));
    c.activate("Tabela");
    c.activate("Cinco mil linhas");
    assert!(c.says("Mostrando 100 de 5000 itens"), "a long list is cut with a notice");
}

#[test]
fn the_expression_editor_previews_the_result_and_tells_a_failure_apart() {
    let mut c = Center::memory("ana");
    c.press("Editor de expressões");
    assert!(c.says("Resultado: 300"), "the preview over the sample order: {:?}", c.texts());
    c.press("Usar um exemplo que dá certo");
    assert!(c.says("Resultado: 140"), "with the parameter Desconto");
    c.press("Usar um exemplo que falha");
    assert!(c.says("Não deu para calcular"), "a failure says so");
    assert!(!c.says("Resultado:"), "and is not a result");
    c.press("Limpar");
    assert!(!c.says("Não deu para calcular"), "nothing written, nothing to preview");
}

#[test]
fn the_parameter_form_builds_from_a_schema_and_gives_back_only_on_apply() {
    let mut c = Center::memory("ana");
    c.press("Formulário de parâmetros");
    assert!(c.says("Endereço"), "the fields of the schema: {:?}", c.texts());
    assert!(!c.says("Corpo"), "the body only shows for POST");
    assert!(c.says("Pediu as opções de account"), "the account's options are asked for");
    c.press("Adicionar");
    assert!(c.says("Remover a linha 1"), "a row of headers");
    c.press("Aplicar");
    assert!(c.says("Endereço é obrigatório"), "nothing goes back without the address");
    c.activate("Expressão");
    c.press("Descartar");
    assert!(c.says("Descartado."));
    c.activate("Carregando");
    assert!(c.says("Carregando as opções"));
    c.activate("Falharam");
    assert!(c.says("Não deu para carregar as opções"));
    c.press("Tentar de novo");
    assert!(c.says("Pediu as opções de account"));

    // live: the form of the dialog gives back each change at once, with no buttons of its own.
    assert!(c.says("Branch do disparo é obrigatório"), "the empty required field is marked: {:?}", c.texts());
    c.type_into("Branch do disparo", "develop");
    assert!(c.says("\"branch\":\"develop\""), "{:?}", c.texts());
    assert!(!c.says("Branch do disparo é obrigatório"), "{:?}", c.texts());
    let applies = c.ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some("Aplicar")).count();
    assert_eq!(applies, 1, "only the first form has Aplicar");
}

#[test]
fn the_cables_show_eight_uses_and_plug_in_each() {
    let mut c = Center::memory("ana");
    c.press("Cabos");
    for (name, cables) in
        raven_feature_center::widgets::graficos::cabos::USES.iter().zip([2, 3, 2, 1, 4, 3, 0, 1, 1])
    {
        c.activate(name);
        assert_eq!(c.ui.cables().ids().len(), cables, "{name}: {:?}", c.texts());
    }
}

#[test]
fn the_patch_panel_turns_around_retracts_pairs_and_checks() {
    let mut c = Center::memory("ana");
    c.press("Painel de tomadas");
    for (name, cables) in raven_feature_center::widgets::graficos::painel::USES.iter().zip([1, 0, 0]) {
        c.activate(name);
        assert_eq!(c.ui.cables().ids().len(), cables, "{name}: {:?}", c.texts());
    }
    c.activate("Rack");
    assert!(c.says("Entrada 1"), "the back: {:?}", c.texts());
    c.activate("Virar");
    assert!(c.says("Amplificador A-40"), "the front: {:?}", c.texts());
    c.activate("Ver a traseira");
    assert!(c.says("Saída L"));
}

#[test]
fn the_dock_refuses_to_close_unsaved_work_keeps_its_arrangement_and_takes_three_uses() {
    use raven_feature_center::widgets::doca::area::{UNSAVED, USES};
    let mut c = Center::memory("ana");
    c.press("Doca");
    assert!(c.says("Editor de código"), "the label: {:?}", c.texts());
    // A document with changes not saved does not close, and the area says why.
    c.press(&format!("Fechar {UNSAVED}"));
    assert!(c.says("tem mudanças não salvas"), "refused: {:?}", c.texts());
    assert!(c.ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(UNSAVED)), "still open");
    // The arrangement kept as text comes back after a tool closes.
    c.press("Guardar o arranjo");
    assert!(c.says("Arranjo guardado"));
    c.press("Fechar Busca");
    assert!(c.says("Arranjo salvo: 6 abertos, 1 fechados"), "on_end: {:?}", c.texts());
    c.press("Restaurar o guardado");
    assert!(c.says("Arranjo restaurado"));
    assert!(c.has_name("Busca"), "the tool is back: {:?}", c.texts());
    // Once saved, it closes.
    c.press(&format!("Salvar {UNSAVED}"));
    c.press(&format!("Fechar {UNSAVED}"));
    assert!(!c.has_name(UNSAVED), "it closed: {:?}", c.texts());
    // The other uses: a floating navigator, and columns the app opens and closes.
    c.activate(USES[1]);
    assert!(c.says("Mesa de fotos"));
    assert!(c.says("Navegador, flutuando"), "floating: {:?}", c.texts());
    c.activate(USES[2]);
    assert!(c.says("Página fixa"));
    c.press("Índice");
    assert!(c.says("Índice: fechado"));
    c.press("Índice");
    assert!(c.says("Índice: aberto"));
}

#[test]
fn the_ribbon_opens_pages_searches_and_shows_the_table_tools_with_a_table() {
    let mut c = Center::memory("ana");
    c.press("Faixa de opções");
    assert!(c.says("Página aberta: Início"), "{:?}", c.texts());
    c.press("Recortar");
    assert!(c.says("Recortado"));
    c.activate("Exibir");
    assert!(c.says("Página aberta: Exibir"));
    assert!(!c.has_name("Layout"), "no table, no contextual page");
    c.activate("Tabela selecionada");
    assert!(c.has_name("Layout"), "the table tools: {:?}", c.texts());
    c.press("Aproximar");
    assert!(c.says("Zoom: 110%"));
    c.type_into("Diga o que você deseja fazer", "tabela");
    assert!(c.says("Procurando: tabela"), "{:?}", c.texts());
}

impl Center {
    /// Whether anything on screen is called exactly this.
    fn has_name(&mut self, name: &str) -> bool {
        self.settle();
        self.ui.access_tree().nodes.iter().any(|n| n.name.as_deref() == Some(name))
    }
}

#[test]
fn the_progress_bar_says_its_stage_and_puts_the_detail_beside_the_label_or_under_the_bar() {
    let mut c = Center::memory("ana");
    c.press("Progresso");
    let height = |c: &mut Center, name: &str| {
        let tree = c.ui.access_tree();
        tree.find(Role::ProgressIndicator, name).unwrap_or_else(|| panic!("{name}")).bounds.height()
    };
    // detail_inline: one line of words over the bar; screen readers hear the detail all the same.
    assert!(c.says("60%") && c.says("816 de 1.360 testes"), "{:?}", c.texts());
    let inline = height(&mut c, "Estágio 3 de 5 · Testes");
    c.press("Avançar um estágio");
    assert!(c.says("Estágio 4 de 5 · Testes") && c.says("80%"), "{:?}", c.texts());
    // Off: the detail goes under the bar, a line more.
    c.activate("Detalhe na linha do rótulo");
    let under = height(&mut c, "Estágio 4 de 5 · Testes");
    assert!(inline + 10.0 < under, "inline {inline}, under {under}");
    // tone, color, height, label, percent and detail of the others.
    assert!(c.says("Disco") && c.says("Cor própria, 10 pt") && c.says("12 de 30 fotos"), "{:?}", c.texts());
    // marker and marker_label: the value is read against the limit, above and below it.
    let spoken = |c: &mut Center, name: &str| {
        c.settle();
        let tree = c.ui.access_tree();
        tree.find(Role::ProgressIndicator, name).and_then(|n| n.value.clone()).unwrap_or_default()
    };
    assert_eq!(spoken(&mut c, "Confiança do palpite"), "89%, acima do limite 0,85");
    assert!(c.says("Com 89%, o bot responde."), "{:?}", c.texts());
    c.press("Palpite de 60%");
    assert_eq!(spoken(&mut c, "Confiança do palpite"), "60%, abaixo do limite 0,85");
    assert!(c.says("Com 60%, o bot diz \"não entendi\"."), "{:?}", c.texts());
    // Two unnamed markers, read by where they are.
    assert_eq!(spoken(&mut c, "Meta e piso do mês"), "55%, acima do ponto de 30%, abaixo do ponto de 80%");
}

#[test]
fn the_scheduler_shows_each_resource_compact_or_on_two_lines_and_tells_its_events() {
    let mut c = Center::memory("ana");
    c.press("Agenda por recurso");
    // resources, resources_title, compact and legend: the title of the column, the legend, a row
    // per resource; the one away says why.
    assert!(c.says("AGENTES") && c.says("A linha vertical é agora"), "{:?}", c.texts());
    assert!(c.has_name("oficina-1") && c.has_name("vps-backup"), "{:?}", c.texts());
    assert!(
        c.says("Desconectado há 3 h · o build que estava nele foi retomado no oficina-1"),
        "{:?}",
        c.texts()
    );
    // on_open: opening an event says which.
    let tree = c.ui.access_tree();
    let event = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref().is_some_and(|t| t.starts_with("site #56")))
        .map(|n| n.id);
    if let Some(id) = event {
        c.ui.perform_action(id, ravenrs::core::AccessAction::Click);
    }
    // The toggles change the look and the page stays whole.
    for name in
        ["Compacto", "Barra", "Cartão", "Avatares", "Linhas que preenchem", "Só leitura", "Nomes estreitos"]
    {
        c.activate(name);
    }
    assert!(c.has_name("oficina-1"), "{:?}", c.texts());
}

#[test]
fn toggles_switch_with_a_label_compact_or_only_an_accessible_name() {
    let mut c = Center::memory("ana");
    c.press("Chaves");
    // new and label: on to begin with, and clicking the label switches it off.
    assert!(c.says("Avisos no celular: ligada"), "{:?}", c.texts());
    c.activate("Avisos no celular");
    assert!(c.says("Avisos no celular: desligada"), "{:?}", c.texts());
    // small: the compact switch is shorter and switches the same.
    c.activate("Resumo por e-mail");
    assert!(c.says("Resumo por e-mail: ligada"), "{:?}", c.texts());
    let tree = c.ui.access_tree();
    let small = tree.find(Role::Switch, "Resumo por e-mail").expect("the small switch").bounds.height();
    let large = tree.find(Role::Switch, "Avisos no celular").expect("the switch").bounds.height();
    assert!(small < large, "small {small}, large {large}");
    // accessible_name: no text on the screen, and still found and switched by its name.
    let tree = c.ui.access_tree();
    let named: Vec<_> = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Ligar docker")).collect();
    assert_eq!(named.len(), 1, "only the switch has the name, no text shows it: {named:?}");
    assert_eq!(named[0].role, Role::Switch);
    c.activate("Ligar docker");
    assert!(c.says("Plugin docker: ligada"), "{:?}", c.texts());
    // disabled and read_only: neither changes.
    let tree = c.ui.access_tree();
    assert!(tree.find(Role::Switch, "Indisponível").expect("disabled").disabled);
    c.activate("Só leitura");
    assert!(c.ui.access_tree().find(Role::Switch, "Só leitura").expect("read only").read_only);
}

#[test]
fn the_code_completes_once_and_locks_while_it_is_verified() {
    let mut c = Center::memory("ana");
    c.press("Código de verificação");
    let field = |c: &mut Center| {
        c.settle();
        let tree = c.ui.access_tree();
        let found = tree
            .nodes
            .iter()
            .find(|n| n.role == Role::TextInput && n.name.as_deref() == Some("Código de verificação"));
        found.cloned().expect("the code")
    };
    // new: digits only, one per box.
    c.type_into("Código de verificação", "48a29");
    assert!(c.says("Digitado: 4829"), "{:?}", c.texts());
    // on_complete: the sixth digit sends it, once.
    c.type_into("Código de verificação", "13");
    assert!(c.says("Verificando 482913…") && c.says("Conclusões: 1"), "{:?}", c.texts());
    // disabled: unavailable to screen readers, and typing changes nothing nor completes again.
    assert!(field(&mut c).disabled);
    c.type_into("Código de verificação", "7");
    assert_eq!(field(&mut c).value.as_deref(), Some("482913"));
    assert!(c.says("Conclusões: 1"), "{:?}", c.texts());
    // Back on, what was typed is kept.
    c.press("Resposta do servidor");
    let back = field(&mut c);
    assert!(!back.disabled && back.value.as_deref() == Some("482913"), "{back:?}");
    assert!(c.says("Digitado: 482913"), "{:?}", c.texts());
}

#[test]
fn avatars_take_a_size_a_picture_a_ring_and_a_circle_or_a_rounded_square() {
    let mut c = Center::memory("ana");
    c.press("Avatar");
    // new, size, image and shape: every avatar is named by the person, picture or initials.
    for name in ["Ana Lima", "Conta Redonda", "Conta Quadrada", "Foto Quadrada"] {
        c.settle();
        assert!(c.ui.access_tree().find(Role::Image, name).is_some(), "{name}: {:?}", c.texts());
    }
    let md = Theme::raven_dark().radii.md;
    let primitives = c.ui.scene().primitives().to_vec();
    assert!(
        primitives.iter().any(|p| matches!(p, Primitive::Image(image) if image.corner_radii == [md; 4])),
        "a picture cropped to the rounded square"
    );
    assert!(
        primitives.iter().any(|p| matches!(p, Primitive::Quad(q) if q.corner_radii == [md; 4])),
        "initials in it"
    );
    // border: the chosen one of the row wears the ring, and choosing another moves it.
    assert!(c.says("Escolhida: Ana Lima"), "{:?}", c.texts());
    c.press("Carla Dias");
    assert!(c.says("Escolhida: Carla Dias"), "{:?}", c.texts());
    let accent = Theme::raven_dark().color(Semantic::Accent);
    let rings = c
        .ui
        .scene()
        .primitives()
        .iter()
        .filter(|p| matches!(p, Primitive::Quad(q) if q.border.is_some_and(|b| b.width == 2.0 && b.color == accent)))
        .count();
    assert_eq!(rings, 1, "one ring, on the chosen one");
}

#[test]
fn cards_are_clicked_chosen_and_toned_and_take_every_shape() {
    let mut c = Center::memory("ana");
    c.press("Cartões");
    let card_named = |c: &mut Center, start: &str| {
        c.settle();
        let tree = c.ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.role == Role::Button && n.name.as_deref().is_some_and(|name| name.starts_with(start)))
            .unwrap_or_else(|| panic!("no card {start}"))
            .clone()
    };
    // on_click: the card is a button, and a click opens it.
    let report = card_named(&mut c, "Relatório do mês").id;
    c.ui.perform_action(report, AccessAction::Click);
    c.settle();
    assert!(c.says("Aberto 1 vezes"), "{:?}", c.texts());
    // selected: one plan of three is chosen, and choosing another moves the mark.
    assert_eq!(card_named(&mut c, "Estúdio").selected, Some(true));
    let network = card_named(&mut c, "Rede").id;
    c.ui.perform_action(network, AccessAction::Click);
    c.settle();
    assert!(c.says("Plano escolhido: Rede"), "{:?}", c.texts());
    assert_eq!(card_named(&mut c, "Rede").selected, Some(true));
    assert_eq!(card_named(&mut c, "Estúdio").selected, Some(false));
    // tone, fill, border, corner_radius, padding, shadow and no_shadow, as painted.
    let theme = Theme::raven_dark();
    let quads: Vec<Quad> =
        c.ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
            .collect();
    for tone in [Semantic::Success, Semantic::Warning, Semantic::Danger] {
        let hairline = theme.color(tone).with_alpha(0.30);
        assert!(quads.iter().any(|q| q.border.is_some_and(|b| b.color == hairline)), "a {tone:?} card");
    }
    assert!(quads.iter().any(|q| q.corner_radii == [10.0; 4] && q.border.is_some()), "the compact card");
    assert!(
        quads.iter().any(|q| q.border.is_none() && q.fill == theme.color(Semantic::Surface).into()),
        "no hairline"
    );
    assert!(quads.iter().any(|q| q.shadow == Some(theme.elevation.raised)), "the raised card");
    assert!(quads.iter().any(|q| q.shadow.is_none() && q.border.is_some()), "the flat card");
    for said in [
        "Padrão",
        "Compacto",
        "Plano",
        "Elevado",
        "Sem filete",
        "Neutro",
        "Destaque",
        "Pós-venda",
        "A conferir",
        "Estornado",
    ] {
        assert!(c.says(said), "{said}: {:?}", c.texts());
    }
}

#[test]
fn check_boxes_are_mixed_by_their_children_or_by_an_unknown_answer() {
    let mut c = Center::memory("ana");
    c.press("Caixas de seleção");
    let state = |c: &mut Center, name: &str| {
        c.settle();
        c.ui.access_tree().find(Role::CheckBox, name).unwrap_or_else(|| panic!("{name}")).clone()
    };
    // new: a click checks it.
    c.activate("Aceito os termos");
    assert!(c.says("Termos: aceitos"), "{:?}", c.texts());
    // mixed: two of three extras, so the select all is partly checked; a click checks them all,
    // another unchecks them all; a child alone makes it mixed again.
    assert_eq!(state(&mut c, "Todos os adicionais").toggled, Some(Toggled::Mixed));
    c.activate("Todos os adicionais");
    assert!(c.says("Adicionais: Seguro, Bagagem extra, Assento na janela"), "{:?}", c.texts());
    assert_eq!(state(&mut c, "Todos os adicionais").toggled, Some(Toggled::True));
    c.activate("Todos os adicionais");
    assert!(c.says("Adicionais: nenhum"), "{:?}", c.texts());
    c.activate("Seguro");
    assert_eq!(state(&mut c, "Todos os adicionais").toggled, Some(Toggled::Mixed));
    // leading: the children's labels line up past the width of a toggle.
    let (child, parent) = (state(&mut c, "Seguro"), state(&mut c, "Todos os adicionais"));
    assert!(child.bounds.width() > 0.0 && parent.bounds.width() > 0.0);
    // optional: unknown, yes, no, and unknown again.
    assert_eq!(state(&mut c, "Tem carro").toggled, Some(Toggled::Mixed));
    for said in ["sim", "não", "não informado"] {
        c.activate("Tem carro");
        assert!(c.says(&format!("Tem carro: {said}")), "{said}: {:?}", c.texts());
    }
    // disabled and read_only: neither changes.
    assert!(state(&mut c, "Indisponível").disabled);
    c.activate("Só leitura");
    let read_only = state(&mut c, "Só leitura");
    assert!(read_only.read_only && read_only.toggled == Some(Toggled::True), "{read_only:?}");
}

#[test]
fn buttons_show_every_variant_and_activate_with_the_mouse_enter_and_space() {
    let mut c = Center::memory("ana");
    c.press("Botões");
    for name in [
        "Salvar",
        "Revisar",
        "Ver detalhes",
        "Excluir",
        "Pequeno",
        "Médio",
        "Grande",
        "Mini",
        "Enviar",
        "Continuar",
        "Ocupar a largura",
    ] {
        c.press(name);
        assert!(c.says(&format!("Acionado: {name}")), "{name}: {:?}", c.texts());
    }
    let tree = c.ui.access_tree();
    let button = |name| tree.find(Role::Button, name).expect("the demonstrated button");
    for (name, height) in [("Pequeno", 32.0), ("Médio", 40.0), ("Grande", 48.0), ("Mini", 22.0)] {
        assert_eq!(button(name).bounds.height(), height, "{name}");
    }
    assert!(button("Ocupar a largura").bounds.width() > button("Médio").bounds.width());
    assert_eq!(button("Pesquisar documentos").description.as_deref(), Some("Abre a pesquisa de documentos"));
    assert!(button("Indisponível").disabled);
    assert!(button("Processando").disabled);
    c.press("Pesquisar documentos");
    assert!(c.says("Pesquisa aberta"));
    c.press("Indisponível");
    c.press("Processando");
    assert!(c.says("Pesquisa aberta"), "inert buttons did not invoke their handlers");
    c.press("Fixar");
    assert!(c.says("Fixado"));
    assert_eq!(
        c.ui.access_tree().find(Role::Button, "Fixar").expect("toggle").toggled,
        Some(ravenrs_core::Toggled::True)
    );
    c.press("Fixar");
    assert!(c.says("Não fixado"));
    c.press("Contar ativações");
    let id = c.ui.access_tree().find(Role::Button, "Contar ativações").expect("counter").id;
    c.ui.perform_action(id, AccessAction::Focus);
    c.key(ravenrs_core::Key::Enter);
    c.key(ravenrs_core::Key::Space);
    assert!(c.says("Ativações: 3"), "{:?}", c.texts());
}

#[test]
fn text_shows_the_type_scale_and_keeps_truncated_and_reactive_content_accessible() {
    let mut c = Center::memory("ana");
    c.press("Texto");
    for name in [
        "Métrica",
        "Destaque",
        "Título",
        "Cabeçalho",
        "Corpo",
        "Chamada",
        "Legenda",
        "Tamanho, peso e cor",
        "À esquerda",
        "Ao centro",
        "À direita",
    ] {
        assert!(c.has_name(name), "{name}: {:?}", c.texts());
    }
    assert!(
        c.has_name(raven_feature_center::widgets::basicos::texto::LONG),
        "truncation preserves the accessible name"
    );
    let tree = c.ui.access_tree();
    let bounds =
        |name| tree.nodes.iter().find(|node| node.name.as_deref() == Some(name)).expect("the text").bounds;
    let glyphs = |name| {
        let area = bounds(name);
        c.ui.scene()
            .primitives()
            .iter()
            .filter_map(|primitive| {
                let ravenrs_core::Primitive::Glyphs(run) = primitive else {
                    return None;
                };
                run.glyphs
                    .first()
                    .filter(|glyph| {
                        glyph.x >= area.x()
                            && glyph.x <= area.x() + area.width()
                            && glyph.y >= area.y()
                            && glyph.y <= area.y() + area.height()
                    })
                    .map(|_| run)
            })
            .collect::<Vec<_>>()
    };
    assert!(bounds("Métrica").height() > bounds("Legenda").height());
    assert!(glyphs("Tamanho, peso e cor").iter().any(|run| run.size == 20.0 && run.color == c.ui.theme().color(ravenrs_style::Semantic::InkMuted)));
    let left = glyphs("À esquerda")[0].glyphs[0].x;
    let center = glyphs("Ao centro")[0].glyphs[0].x;
    let right = glyphs("À direita")[0].glyphs[0].x;
    assert!(left < center && center < right, "the three alignments move the painted text");
    let visible: usize =
        glyphs(raven_feature_center::widgets::basicos::texto::LONG).iter().map(|run| run.glyphs.len()).sum();
    assert!(
        visible > 0 && visible < raven_feature_center::widgets::basicos::texto::LONG.chars().count() / 2,
        "truncation limits the painted line, not the accessible text"
    );
    assert!(c.says("Vinculado: 0") && c.says("Calculado: 0"));
    c.press("Alterar conteúdo");
    assert!(c.says("Vinculado: 1") && c.says("Calculado: 2"));
    c.press("Alterar conteúdo");
    assert!(c.says("Vinculado: 2") && c.says("Calculado: 4"));
}

#[test]
fn the_news_of_each_version_open_on_the_version_running_the_newest_and_a_failure() {
    let mut c = Center::memory("ana");
    c.press("Novidades da versão");
    assert!(c.says("Novidades fechadas"), "{:?}", c.texts());
    // The version in the status bar opens the news of the version running. A modal dialog is
    // all the tree shows, so its own count says which version is open.
    c.press("Raven Exemplo · versão 0.2.0");
    for said in [
        "2 de 3",
        "Instalada",
        "Busca em todas as colunas",
        "Quem contribuiu",
        "Alex S S Fonseca, Matheus",
        "#729",
        "#731",
    ] {
        assert!(c.says(said), "{said}: {:?}", c.texts());
    }
    c.key(ravenrs_core::Key::Escape);
    assert!(c.says("Novidades fechadas"), "Esc closes it: {:?}", c.texts());
    c.press("Mais recente");
    for said in ["1 de 3", "Nova", "Por que atualizar", "Como atualizar", "Relatórios agendados"] {
        assert!(c.says(said), "{said}: {:?}", c.texts());
    }
    c.key(ravenrs_core::Key::Escape);
    c.press("Versão em uso");
    assert!(c.says("2 de 3"), "{:?}", c.texts());
    c.key(ravenrs_core::Key::Escape);
    c.press("Detalhes de uma falha");
    assert!(c.says("Detalhes da atualização"), "{:?}", c.texts());
    c.press("Abrir o registro");
    c.key(ravenrs_core::Key::Escape);
    assert!(c.says("detalhes fechados"), "{:?}", c.texts());
    assert!(c.says(".raven-exemplo-do-centro"), "the log is asked of the system: {:?}", c.texts());
}

#[test]
fn the_service_page_writes_the_file_of_each_system_and_plans_with_no_command_run() {
    let mut c = Center::memory("ana");
    c.press("Serviço do sistema");
    assert!(c.says("Gerenciador deste computador:"), "{:?}", c.texts());
    // systemd, with the limit: the unit of the app's own name, its command and MemoryMax.
    for said in [
        "Conferido: o serviço pode ser instalado",
        "raven-exemplo.service",
        "MemoryMax=1G",
        "Restart=on-failure",
        "\"servir\"",
    ] {
        assert!(c.says(said), "{said}: {:?}", c.texts());
    }
    c.press("Limite de 1G: sim");
    assert!(!c.says("MemoryMax"), "with no limit, no MemoryMax: {:?}", c.texts());
    c.press("Do sistema todo: não");
    // For the whole system; the file stays in the folder of the dry run, never in /etc.
    assert!(
        c.says("WantedBy=multi-user.target") && c.says("raven-feature-center-servico"),
        "{:?}",
        c.texts()
    );
    c.press("Planejar a instalação");
    assert!(c.says("systemctl enable --now raven-exemplo"), "{:?}", c.texts());
    c.press("Ver o estado");
    assert!(c.says("Estado: instalado") && c.says("systemctl is-active raven-exemplo"), "{:?}", c.texts());
    c.press("Remover");
    assert!(c.says("Removido · systemctl disable --now raven-exemplo"), "{:?}", c.texts());
    c.press("Do sistema todo: sim");
    c.press("Limite de 1G: não");
    // launchd: the plist by the bundle id, kept alive, and the limit macOS does not enforce.
    c.press("launchd");
    for said in
        ["dev.ravenrs.raven-exemplo.plist", "<key>RunAtLoad</key><true/>", "O macOS não o aplica", "RUST_LOG"]
    {
        assert!(c.says(said), "{said}: {:?}", c.texts());
    }
    c.press("Planejar a instalação");
    assert!(c.says("launchctl bootstrap gui/UID") && c.says("aviso: o macOS não aplica"), "{:?}", c.texts());
    // Windows: a task at logon, again when it falls.
    c.press("Windows");
    assert!(c.says("<LogonTrigger>") && c.says("<RestartOnFailure>"), "{:?}", c.texts());
    c.press("Planejar a instalação");
    assert!(c.says("schtasks /Create /TN raven-exemplo"), "{:?}", c.texts());
    c.press("Remover");
    // A name by hand, with ServiceSpec::new.
    c.press("Nome à mão: não");
    assert!(c.says("raven-exemplo-servidor-tarefa.xml"), "{:?}", c.texts());
}

#[test]
fn the_web_api_page_builds_the_options_the_server_gets() {
    let mut c = Center::memory("ana");
    c.press("API web");
    assert!(c.says("Nenhuma origem permitida."), "{:?}", c.texts());
    assert!(c.says("workers: 1"), "one thread to begin with: {:?}", c.texts());

    // allow_origin: each origin named, and the server gets it.
    c.type_into("Origem", "https://exemplo.com");
    c.press("Permitir origem");
    assert!(c.says("Permitidas: https://exemplo.com."), "{:?}", c.texts());
    assert!(c.says("origins: [\"https://exemplo.com\"]"), "{:?}", c.texts());

    // workers: more threads, and zero counts as one.
    c.press("Aumentar");
    c.press("Aumentar");
    c.press("Aumentar");
    assert!(c.says("Pedido: 4."), "{:?}", c.texts());
    assert!(c.says("workers: 4"), "{:?}", c.texts());
    for _ in 0..4 {
        c.press("Diminuir");
    }
    assert!(c.says("Pedido: 0. Conta como 1"), "{:?}", c.texts());
    assert!(c.says("workers: 1"), "zero is one: {:?}", c.texts());

    // live: the channel goes to the server, and what is published gets its id.
    assert!(c.says("live: None"), "{:?}", c.texts());
    c.activate("Ligar o canal ao vivo");
    assert!(c.says("topics: [\"avisos\"]"), "{:?}", c.texts());
    assert!(c.says("Nenhum evento publicado em avisos."), "{:?}", c.texts());
    c.press("Publicar evento");
    c.press("Publicar evento");
    assert!(c.says("Último evento publicado em avisos: #2."), "{:?}", c.texts());

    // open and pass_header: the webhooks of other services come in with no token, with their headers.
    assert!(c.says("open: []"), "{:?}", c.texts());
    c.activate("Receber avisos de outros serviços");
    assert!(c.says("open: [\"/hooks/\"]"), "{:?}", c.texts());
    assert!(c.says("headers: [\"x-hub-\"]"), "{:?}", c.texts());
}

#[test]
fn the_page_of_the_api_service_asks_each_feature_through_handle() {
    let mut c = Center::memory("ana");
    c.press("Serviço da API");
    assert!(c.says("Nenhum pedido ainda."), "{:?}", c.texts());

    // new, handle and session_lifetime: ana signs in, and the session lasts 30 minutes.
    c.press("Entrar como ana");
    assert!(c.says("Entrar: 200: a sessão dura 1800 s"), "{:?}", c.texts());
    // setting: a preference of her own, kept and read back.
    c.press("Guardar o tema escuro");
    assert!(c.says("Tema: 200: tema \"escuro\""), "{:?}", c.texts());
    // endpoint: a command of the center's, run as whoever called it.
    c.press("Somar 2 e 3");
    assert!(c.says("Somar: 200: soma 5.0"), "{:?}", c.texts());
    // title and hide: the description has its name and leaves the entity out.
    c.press("Ler a descrição");
    assert!(c.says("API do Centro de Recursos; AuditOwner fica de fora: sim"), "{:?}", c.texts());
    // webhook: a signed notice is taken, a forged one refused.
    c.press("Receber aviso assinado");
    assert!(c.says("assinado: 2"), "{:?}", c.texts());
    assert!(!c.says("com assinatura falsa: 2"), "{:?}", c.texts());
    // fallback: the route answers only while it is on, as text (ApiResponse::raw).
    c.press("Pedir /api/hora");
    assert!(c.says("Hora: 404"), "off, no one knows it: {:?}", c.texts());
    c.activate("Ligar a rota /api/hora");
    c.press("Pedir /api/hora");
    assert!(c.says("Hora: 200 text/plain: 12:00 para ana"), "{:?}", c.texts());
    // authenticator: a guest comes in by an invitation.
    c.press("Entrar com convite");
    assert!(c.says("Convite: 200: visitante entrou por convite"), "{:?}", c.texts());
    // sign_ins_a_minute: after 3 tries a minute, the address waits.
    c.press("Errar a senha 4 vezes");
    assert!(c.says("429"), "the fourth try waits: {:?}", c.texts());
    // role_merge, rule_words and audit: how the roles add up changes the service.
    assert!(c.says("Papéis: um que permita basta."), "{:?}", c.texts());
    c.activate("Todos os papéis");
    assert!(c.says("Papéis: todos precisam permitir."), "{:?}", c.texts());
    assert!(c.says("as regras falam português"), "{:?}", c.texts());
}

#[test]
fn the_build_log_shows_colors_folds_its_stages_searches_and_points_at_a_line() {
    let mut c = Center::memory("ana");
    c.press("Log de build");
    assert!(c.says("Checkout"), "the stages: {:?}", c.texts());
    assert!(c.says("PASS") && c.says("FAIL"), "the lines: {:?}", c.texts());
    assert!(!c.texts().iter().any(|text| text.contains('\u{1b}')), "no ANSI code shows");
    assert!(c.says("Log do build 42") || c.has_name("Log do build 42"), "its label: {:?}", c.texts());

    // A long line keeps the height of one line and does not run into the next (#899): the list
    // scrolls across to it instead of breaking it.
    let tree = c.ui.access_tree();
    let item = |number: usize| {
        let start = format!("Apontar a linha {number} ");
        tree.nodes
            .iter()
            .find(|n| n.role == Role::ListItem && n.name.as_deref().is_some_and(|t| t.starts_with(&start)))
    };
    let (long, next) = (item(10), item(9));
    let long = long.expect("the long line").bounds;
    assert!(long.height() <= 20.5, "one line high: {long:?}");
    assert!(next.expect("the line before").bounds.max_y() <= long.origin.y + 0.5, "no overlap");

    // The search marks the lines that hold it.
    c.type_into("Buscar no log", "fail");
    assert!(c.says("1 linhas com o termo"), "{:?}", c.texts());

    // Each stage says how it ran, and a line with no colors takes them from its first word.
    assert!(c.says("· git · 8 s"), "{:?}", c.texts());
    assert!(c.says("SLOW") && c.says("Baixar log"), "{:?}", c.texts());

    // A stage folds and says how much it hides.
    c.press("Checkout · git · 8 s");
    assert!(c.says("· git · 8 s · 2 linhas dobradas"), "{:?}", c.texts());
    assert!(!c.says("HEAD is now at"), "its lines are hidden");
    c.press("Checkout · git · 8 s · 2 linhas dobradas");
    assert!(c.says("HEAD is now at"), "and come back");

    // A line is pointed at by its number.
    c.press("Apontar a linha 2");
    assert!(c.says("Linha apontada: 2."), "{:?}", c.texts());

    // A new line comes, and the settings change what shows.
    c.press("Chegar linha nova");
    assert!(c.says("nova::linha_1"), "{:?}", c.texts());
    assert!(c.says("11 linhas no log."), "{:?}", c.texts());

    // One stage alone, and back to every line.
    c.activate("Só o estágio Testes");
    assert!(c.says("Só Testes") && !c.says("HEAD is now at"), "{:?}", c.texts());
    c.press("Mostrar tudo");
    assert!(c.says("HEAD is now at"), "{:?}", c.texts());
    c.activate("Números das linhas");
    assert!(!c.has_name("Apontar a linha 2"), "no numbers");
    c.activate("Busca");
    assert!(!c.has_name("Buscar no log") && !c.says("linhas com o termo"), "no search field");
    c.activate("Seguir o fim");
    assert!(c.has_name("Seguir o fim"), "the switch stays, to follow again");
}

#[test]
fn the_stats_say_their_number_with_a_caption_and_the_change_since_before() {
    let mut c = Center::memory("ana");
    c.press("Indicadores");
    // new and caption: the label in capitals, the number and the line under it.
    assert!(c.says("NA FILA") && c.says("O mais antigo espera há 40 s"), "{:?}", c.texts());
    c.press("Chegou um build");
    assert!(c.says("3") && c.says("O mais antigo espera há 60 s"), "{:?}", c.texts());
    c.press("Saiu um build");
    c.press("Saiu um build");
    c.press("Saiu um build");
    assert!(c.says("Ninguém espera"), "{:?}", c.texts());
    // large: the same stat, in the size of the number a screen is about.
    c.activate("Grande");
    assert!(c.says("NA FILA"), "{:?}", c.texts());
    // delta and delta_toned: the change, up or down, good or bad.
    for change in ["+12,4%", "-3,1%", "+8%", "-0,4%"] {
        assert!(c.says(change), "{change}: {:?}", c.texts());
    }
    assert!(c.says("Lida do processo, ao vivo; meta ≤ 60 MB"), "{:?}", c.texts());
    // value_tone: the totals painted in their tone, taken toward ink as a badge's words are.
    assert!(c.says("PÓS-VENDA") && c.says("R$ 1.240"), "{:?}", c.texts());
    let theme = Theme::raven_dark();
    let ink = theme.color(Semantic::Ink);
    let colors: Vec<_> =
        c.ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| {
                if let Primitive::Glyphs(run) = p { (run.size == 30.0).then_some(run.color) } else { None }
            })
            .collect();
    for tone in [Semantic::Success, Semantic::Warning, Semantic::Danger] {
        let toned = theme.color(tone).lerp(ink, 0.3);
        assert!(colors.contains(&toned), "a value in {tone:?}: {colors:?}");
    }
    assert!(colors.contains(&ink), "and the plain value stays in ink");
}

#[test]
fn the_list_packs_its_rows_tells_their_state_and_runs_their_action() {
    let mut c = Center::memory("ana");
    c.press("Lista");
    let height = |c: &mut Center, name: &str| {
        c.settle();
        let tree = c.ui.access_tree();
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{name}"))
            .bounds
            .height()
    };
    // compact: rows of 48, the subtitle beside the title; the title names the row and the rest
    // describes it.
    let compact = height(&mut c, "Checkout");
    assert!((compact - 48.0).abs() < 0.5, "{compact}");
    let tree = c.ui.access_tree();
    let lint = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Lint")).expect("Lint");
    assert_eq!(lint.description.as_deref(), Some("cargo clippy · 1 min 40 s"));
    c.activate("Compacta");
    assert!(height(&mut c, "Checkout") > compact, "two lines when not compact");
    // on_click and selected: a click opens the row.
    c.press("Lint");
    assert!(c.says("Aberto: Lint"), "{:?}", c.texts());
    // action: the button at the end of the row.
    c.press("Ligar o plugin");
    assert!(c.says("Ação: ligar o plugin"), "{:?}", c.texts());
    // avatar and trailing.
    assert!(c.has_name("Ana Lima") && c.has_name("Bia Souza"), "{:?}", c.texts());
}

#[test]
fn the_title_bar_draws_the_window_buttons_and_the_bar_of_a_phone() {
    let mut c = Center::memory("ana");
    c.press("Barra de título");
    assert!(c.says("Relatório.pdf") && c.says("Raven CI"), "{:?}", c.texts());
    // on_minimize, on_close: a button shows when it has a handler, and runs it.
    c.press("Minimizar");
    assert!(c.says("Minimizar"), "{:?}", c.texts());
    c.press("Fechar");
    assert!(c.says("Fechar"), "{:?}", c.texts());
    // on_maximize and maximized: the middle button says it restores once maximized.
    c.press("Maximizar");
    assert!(c.has_name("Restaurar"), "{:?}", c.texts());
    // mobile: the menu before the title and the search after it.
    c.press("Abrir o menu");
    assert!(c.says("Menu aberto"), "{:?}", c.texts());
    c.press("Buscar comandos");
    assert!(c.says("Busca aberta"), "{:?}", c.texts());
    // active: without the keyboard the bar goes quiet, and still shows.
    c.activate("Com o teclado");
    assert!(c.says("Relatório.pdf"), "{:?}", c.texts());
}

#[test]
fn selects_pick_by_position_or_value_in_four_sizes_with_search_an_icon_and_their_states() {
    let mut c = Center::memory("ana");
    c.press("Listas de escolha");
    c.settle();
    // new, of, label, icon, size, searchable, width, fill, disabled and read_only: each select is named.
    for name in [
        "Estúdio",
        "Qualidade",
        "Prazo",
        "Responsável",
        "Idioma",
        "Tamanho Mini",
        "Tamanho Small",
        "Tamanho Medium",
        "Tamanho Large",
        "Moeda",
        "Largura fixa",
        "Largura da coluna",
        "Desligada",
        "Só leitura",
    ] {
        assert!(c.ui.access_tree().find(Role::ComboBox, name).is_some(), "{name}: {:?}", c.texts());
    }
    assert!(c.says("Estúdio escolhido: Todos os estúdios"), "{:?}", c.texts());
    // Choosing writes the binding: open the list and pick another studio.
    let field = c.ui.access_tree().find(Role::ComboBox, "Estúdio").map(|n| n.id).expect("the field");
    c.ui.perform_action(field, AccessAction::Expand);
    c.settle();
    let option =
        c.ui.access_tree().find(Role::ListBoxOption, "Externo").map(|n| n.id).expect("the open list");
    c.ui.perform_action(option, AccessAction::Click);
    c.settle();
    assert!(c.says("Estúdio escolhido: Externo"), "{:?}", c.texts());
}

#[test]
fn menus_open_from_a_button_or_any_view_with_a_header_titled_sections_and_every_kind_of_item() {
    let mut c = Center::memory("ana");
    c.press("Menus");
    c.settle();
    // trigger: the account is a button named by the label, beside the menus of every style and size.
    for name in [
        "Ações",
        "Conta",
        "Novo",
        "Mais",
        "Apagar",
        "Pequeno",
        "Mini",
        "Grande",
        "Opções",
        "Abrir pelo atalho",
    ] {
        assert!(c.ui.access_tree().find(Role::Button, name).is_some(), "{name}: {:?}", c.texts());
    }
    c.press("Conta");
    c.settle();
    // header and label: who is signed in above the items, and the themes as a group named "Tema".
    assert!(c.says("ana@exemplo.com"), "{:?}", c.texts());
    assert!(c.ui.access_tree().find(Role::Group, "Tema").is_some(), "{:?}", c.texts());
    let escuro = c.ui.access_tree().find(Role::MenuItem, "Escuro").map(|n| n.id).expect("the themes");
    c.ui.perform_action(escuro, AccessAction::Click);
    c.settle();
    assert!(c.says("Tema: Escuro"), "{:?}", c.texts());
}

/// Picks a view in the switch of the calendar: "Ano" is also the name of a page.
fn pick_view(c: &mut Center, view: &str) {
    let id =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::RadioButton && n.name.as_deref() == Some(view))
            .map(|n| n.id)
            .unwrap_or_else(|| panic!("no view {view}: {:?}", c.texts()));
    c.ui.perform_action(id, AccessAction::Click);
    c.settle();
}

#[test]
fn the_event_calendar_shows_six_views_and_names_the_seasons_from_the_hemisphere() {
    let mut c = Center::memory("ana");
    c.press("Calendário de eventos");
    c.settle();
    // views, view and hemisphere: the season of the 15th of November is spring in the south.
    assert!(c.says("Primavera de 2026"), "{:?}", c.texts());
    c.activate("Estações do sul");
    assert!(c.says("Outono de 2026"), "{:?}", c.texts());
    // the agenda: the events from the 15th on, in order.
    pick_view(&mut c, "Agenda");
    assert!(c.says("Café com cliente"), "{:?}", c.texts());
    // on_event: an event opens.
    c.activate("Café com cliente");
    assert!(c.says("Aberto o evento 2"), "{:?}", c.texts());
    // the year, then a day of it opens the month with events, events_per_day and today.
    pick_view(&mut c, "Ano");
    assert!(c.says("2026"), "{:?}", c.texts());
    // the month, with two events a day before the rest are counted, and the week and the day.
    pick_view(&mut c, "Mês");
    assert!(c.says("Ensaio de gestante"), "{:?}", c.texts());
    pick_view(&mut c, "Semana");
    assert!(c.says("Entrega de álbum"), "{:?}", c.texts());
    // the day chosen in the agenda goes along: the 18th.
    pick_view(&mut c, "Dia");
    assert!(c.says("Café com cliente"), "{:?}", c.texts());
}

#[test]
fn the_year_view_marks_days_opens_months_and_shows_a_season_without_its_header() {
    let mut c = Center::memory("ana");
    c.press("Ano");
    c.settle();
    // new, today, marks, columns and selected: a marked day says what it has.
    let tree = c.ui.access_tree();
    assert!(tree.nodes.iter().any(|n| n.description.as_deref() == Some("Ensaio")), "{:?}", c.texts());
    // on_month: the name of a month is a button.
    c.activate("março");
    assert!(c.says("Mês aberto: 3/2026"), "{:?}", c.texts());
    // months and header: three months from December, the year before, without arrows of their own.
    let tree = c.ui.access_tree();
    let calendars = tree.nodes.iter().filter(|n| n.role == Role::Calendar).count();
    assert_eq!(calendars, 12 + 3, "{:?}", c.texts());
    let crossing = tree
        .nodes
        .iter()
        .filter(|n| n.role == Role::Calendar && n.name.as_deref().is_some_and(|name| name.contains("2025")))
        .count();
    assert_eq!(crossing, 1, "December says its year: {:?}", c.texts());
}

#[test]
fn tabs_switch_views_in_two_looks_and_three_sizes_and_send_what_does_not_fit_to_a_menu() {
    let mut c = Center::memory("ana");
    c.press("Abas");
    c.settle();
    // new and tab: the open tab builds its view.
    assert!(c.says("O que aconteceu hoje."), "{:?}", c.texts());
    c.activate("Ajustes");
    c.settle();
    assert!(c.says("Preferências da conta."), "{:?}", c.texts());
    // item, variant and size: six channels in 360, with the last ones in the menu of each bar.
    let tree = c.ui.access_tree();
    let menus = tree
        .nodes
        .iter()
        .filter(|n| n.role == Role::Button && n.name.as_deref() == Some("Mais abas"))
        .count();
    assert_eq!(menus, 2, "one menu in each look: {:?}", c.texts());
    assert!(c.says("Canal: Todas"), "{:?}", c.texts());
    let more = tree
        .nodes
        .iter()
        .find(|n| n.role == Role::Button && n.name.as_deref() == Some("Mais abas"))
        .map(|n| n.id)
        .expect("the menu of the first bar");
    c.ui.perform_action(more, AccessAction::Click);
    c.settle();
    let site = c.ui.access_tree().find(Role::MenuItem, "Site").map(|n| n.id).expect("Site in the menu");
    c.ui.perform_action(site, AccessAction::Click);
    c.settle();
    assert!(c.says("Canal: Site"), "{:?}", c.texts());
    // size: three bars, and disabled: the bar that waits keeps its tab.
    for name in ["Hoje", "Semana", "Mês", "Rascunho", "Enviando"] {
        assert!(c.ui.access_tree().find(Role::Tab, name).is_some(), "{name}: {:?}", c.texts());
    }
    c.activate("Rascunho");
    c.settle();
    let tree = c.ui.access_tree();
    assert_eq!(tree.find(Role::Tab, "Enviando").and_then(|n| n.selected), Some(true), "it did not change");
}

#[test]
fn split_panes_fold_a_side_keep_their_sizes_and_hide_a_pane() {
    let mut c = Center::memory("ana");
    c.press("Painéis divididos");
    c.settle();
    // horizontal, pane, size, range, label and collapsed: the first column folds by its arrow.
    assert!(c.says("Sessões: aberta"), "{:?}", c.texts());
    assert!(c.says("Larguras guardadas: 220, o resto, 220"), "{:?}", c.texts());
    c.activate("Recolher Sessões");
    c.settle();
    assert!(c.says("Sessões: recolhida"), "{:?}", c.texts());
    assert!(c.ui.access_tree().find(Role::Button, "Mostrar Sessões").is_some(), "{:?}", c.texts());
    c.activate("Mostrar Sessões");
    c.settle();
    assert!(c.says("Sessões: aberta"), "{:?}", c.texts());
    // collapsible: the last column folds with its own arrow, kept by the pane.
    c.activate("Recolher Movimento");
    c.settle();
    assert!(c.ui.access_tree().find(Role::Button, "Mostrar Movimento").is_some(), "{:?}", c.texts());
    c.activate("Mostrar Movimento");
    c.settle();
    // sizes and on_end: a key on the first bar writes the width the app keeps, and asks to save.
    let bar =
        c.ui.access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Slider && n.name.as_deref() == Some("Redimensionar"))
            .map(|n| n.id)
            .expect("the first bar");
    c.ui.perform_action(bar, AccessAction::Increment);
    c.settle();
    assert!(c.says("Tamanhos salvos: 1"), "{:?}", c.texts());
    assert!(!c.says("Larguras guardadas: 220, o resto, 220"), "{:?}", c.texts());
    // vertical, height and visible: the lower pane goes and comes back.
    assert!(c.says("Detalhes"), "{:?}", c.texts());
    c.activate("Esconder detalhes");
    c.settle();
    assert!(!c.says("Detalhes"), "{:?}", c.texts());
    c.activate("Mostrar detalhes");
    c.settle();
    assert!(c.says("Detalhes"), "{:?}", c.texts());
}

#[test]
fn monospaced_text_lines_up_the_lines_of_a_receipt() {
    let mut c = Center::memory("ana");
    c.press("Texto");
    c.settle();
    let [subtotal, total] = raven_feature_center::widgets::basicos::texto::CUPOM;
    let tree = c.ui.access_tree();
    // Screen readers may hear a run of spaces as one.
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let width = |name: &str| {
        tree.nodes
            .iter()
            .find(|n| n.name.as_deref().is_some_and(|said| words(said) == words(name)))
            .map(|n| n.bounds.width())
            .expect("the line")
    };
    assert_eq!(width(subtotal), width(total), "the same number of characters ends at the same place");
}

#[test]
fn a_receipt_starts_empty_takes_lines_and_shows_the_total_in_its_band() {
    let mut c = Center::memory("ana");
    c.press("Cupom");
    c.settle();
    // new, title, subtitle, empty_text, summary and total: the sale starts empty.
    assert!(c.says("Passe o primeiro produto."), "{:?}", c.texts());
    assert!(c.says("Caixa 1 · venda 128"), "{:?}", c.texts());
    let total = |c: &mut Center| {
        let tree = c.ui.access_tree();
        tree.nodes
            .iter()
            .filter(|n| n.role == Role::Status && n.name.as_deref() == Some("TOTAL A RECEBER"))
            .filter_map(|n| n.value.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(total(&mut c), ["R$ 0,00", "R$ 519,90"], "the sale and the one with a discount");
    // line: each photo passed is a line, and the total follows.
    c.activate("Passar foto 10x15");
    c.activate("Passar foto 10x15");
    assert!(!c.says("Passe o primeiro produto."), "{:?}", c.texts());
    let line =
        c.ui.access_tree()
            .find(Role::ListItem, "FOTO 10X15 BRILHO")
            .map(|n| (n.value.clone(), n.description.clone()));
    assert_eq!(line, Some((Some("30,00".to_owned()), Some("2 UN x 15,00".to_owned()))));
    assert_eq!(total(&mut c)[0], "R$ 30,00");
    assert!(c.says("DESCONTO"), "{:?}", c.texts());
    c.activate("Nova venda");
    assert!(c.says("Passe o primeiro produto."), "{:?}", c.texts());
}
