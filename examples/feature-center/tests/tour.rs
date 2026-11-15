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

// The steps of each family of widgets, one file each.
#[path = "tour/agenda.rs"]
mod agenda;
#[path = "tour/basicos_avisos.rs"]
mod basicos_avisos;
#[path = "tour/basicos_controles.rs"]
mod basicos_controles;
#[path = "tour/basicos_estrutura.rs"]
mod basicos_estrutura;
#[path = "tour/conversa.rs"]
mod conversa;
#[path = "tour/doca.rs"]
mod doca;
#[path = "tour/grade.rs"]
mod grade;
#[path = "tour/graficos.rs"]
mod graficos;
#[path = "tour/imagem.rs"]
mod imagem;
#[path = "tour/kit.rs"]
mod kit;
#[path = "tour/kit_campos.rs"]
mod kit_campos;
#[path = "tour/kit_navegacao.rs"]
mod kit_navegacao;
#[path = "tour/listas.rs"]
mod listas;

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
    // #160: it can be changed again, so the question is plain, not the one of deleting, and it
    // says what its model says besides.
    assert!(c.says("O valor dobra e a situação passa a Fechado"), "{:?}", c.texts());
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
/// `FC_SCREENSHOTS=folder cargo test -p feature-center --test tour draw_the_lists`.
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
        let app = change(feature_center::actions::counter_app())
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
    let info = feature_center::model::CounterProduct::entity_info();
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
    counter.type_in(feature_center::actions::COUNTER_PIN);
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
    let (record, report) = stored_report(&c, feature_center::seed::LABELS_REPORT);
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
    let (record, report) = stored_report(&c, feature_center::seed::RECEIPT_REPORT);
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
    let service = Rc::new(RefCell::new(feature_center::api_service(None).expect("it starts")));
    let client = ravenrs_app_remote::RemoteClient::new("http://centro", AtOnce(service));
    let mut ui = Ui::new(feature_center::remote_over(client).expect("it opens"), Theme::raven_dark());
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
    assert!(c.says("▸ pedido"), "the field chosen stays chosen");
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
}

#[test]
fn the_cables_show_eight_uses_and_plug_in_each() {
    let mut c = Center::memory("ana");
    c.press("Cabos");
    for (name, cables) in
        feature_center::widgets::graficos::cabos::USES.iter().zip([2, 3, 2, 1, 4, 3, 0, 1, 1])
    {
        c.activate(name);
        assert_eq!(c.ui.cables().ids().len(), cables, "{name}: {:?}", c.texts());
    }
}

#[test]
fn the_patch_panel_turns_around_retracts_pairs_and_checks() {
    let mut c = Center::memory("ana");
    c.press("Painel de tomadas");
    for (name, cables) in feature_center::widgets::graficos::painel::USES.iter().zip([1, 0, 0]) {
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
    use feature_center::widgets::doca::area::{UNSAVED, USES};
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

// Framework: storage and server (fork A).

#[test]
fn a_database_in_memory_comes_back_from_its_storage_and_goes_out_as_json() {
    let mut c = Center::memory("ana");
    c.press("Banco em memória");
    c.press("Acrescentar anotação");
    assert!(c.says("Anotação 1 gravada no armazenamento"), "{:?}", c.texts());
    c.press("Abrir de novo");
    assert!(c.says("Reaberto: 1 anotações voltaram do armazenamento"), "{:?}", c.texts());
    c.press("Exportar JSON");
    assert!(c.says("JSON exportado"), "{:?}", c.texts());
    c.press("Trocar pela cópia de fora");
    assert!(c.says("Anotações: Da matriz, Da filial"), "{:?}", c.texts());
    c.press("Importar o exportado");
    assert!(c.says("Importou 1 registros"), "{:?}", c.texts());
    assert!(c.says("Anotações: Anotação 1"), "back as it was exported: {:?}", c.texts());
    c.press("Gravar num disco cheio");
    assert!(c.says("O armazenamento recusou: o disco está cheio"), "{:?}", c.texts());
}

#[test]
fn a_backup_is_made_checked_restored_and_refused_by_another_tenant() {
    let mut c = Center::memory("ana");
    c.press("Cópias de segurança");
    c.press("Acrescentar contato");
    c.press("Fazer backup");
    assert!(c.says("Backup 1 feito e conferido: contatos: 1"), "{:?}", c.texts());
    c.press("Conferir o backup");
    assert!(c.says("Backup de Raven Centro de Recursos, filial matriz, versão"), "{:?}", c.texts());
    c.press("Acrescentar contato");
    assert!(c.says("Contatos: 2"));
    c.press("Restaurar o backup");
    assert!(c.says("Restaurado (contatos: 1)"), "{:?}", c.texts());
    assert!(c.says("Contatos: 1"), "the one added after the backup is gone");
    c.press("Guardar uma cópia");
    c.press("Guardar uma cópia");
    c.press("Guardar uma cópia");
    c.press("Guardar uma cópia");
    assert!(c.says("Cópias guardadas: 3"), "no more than three stay: {:?}", c.texts());
    c.press("Restaurar na filial");
    assert!(c.says("A filial recusou"), "{:?}", c.texts());
    let _ = std::fs::remove_dir_all(feature_center::framework::infra::copias::folder());
}

#[test]
fn a_table_of_an_older_system_is_read_and_written_through_the_model() {
    let mut c = Center::memory("ana");
    c.press("Mapeamento SQL");
    assert!(
        c.says("Ana Lima (PE, 8 letras)"),
        "trimmed after reading, counted by the database: {:?}",
        c.texts()
    );
    assert!(c.says("o nome do primeiro tem 12 caracteres"), "kept as it was in the table: {:?}", c.texts());
    c.press("Gravar cliente com uf minúscula");
    assert!(c.says("Gravou Caio Dias; na tabela, uf = PE"), "{:?}", c.texts());
    c.press("Repetir uma etiqueta");
    assert!(c.says("O banco recusou a segunda"), "{:?}", c.texts());
}

#[test]
fn postgres_checks_before_the_network_and_postgrest_reads_page_by_page() {
    let mut c = Center::memory("ana");
    c.press("Postgres e PostgREST");
    assert!(c.says("sem RAVEN_POSTGRES_URL"), "{:?}", c.texts());
    c.press("Conferir um endereço errado");
    assert!(c.says("Recusado antes da rede: the database address could not be read"), "{:?}", c.texts());
    c.press("Conferir um certificado errado");
    assert!(c.says("a certificate authority could not be read"), "{:?}", c.texts());
    c.press("Ler pela API");
    assert!(c.says("Leu 3 em 3 pedidos de 2: Café, Açúcar, Leite"), "{:?}", c.texts());
    assert!(c.says("Esquema: loja"), "{:?}", c.texts());
    c.press("Ler com token fixo");
    assert!(c.says("Authorization"), "{:?}", c.texts());
    c.press("Gravar nome repetido");
    assert!(c.says("O servidor recusou"), "{:?}", c.texts());
}

#[test]
fn the_api_signs_in_with_a_code_takes_a_signed_payment_and_sends_its_event_once() {
    let mut c = Center::memory("ana");
    c.press("Integrações");
    c.press("Pedidos pendentes");
    assert!(c.says("Entre antes"));
    c.press("Entrar com o código");
    assert!(c.says("Entrou (200)"), "{:?}", c.texts());
    c.press("Pedidos pendentes");
    assert!(c.says("200: 2 pedidos pendentes, para integra"), "{:?}", c.texts());
    c.press("Ler os eventos pela API");
    assert!(c.says("404: os eventos ficam fora da API"), "{:?}", c.texts());
    for _ in 0..3 {
        c.press("Entrar com código errado");
        assert!(c.says("401: o código não confere"), "{:?}", c.texts());
    }
    c.press("Entrar com código errado");
    assert!(c.says("429"), "a fourth try in a minute is refused: {:?}", c.texts());
    c.press("Receber aviso falso");
    assert!(c.says("401: a assinatura não confere"), "{:?}", c.texts());
    c.press("Receber pagamento assinado");
    assert!(c.says("200: pedido 1 pago"), "{:?}", c.texts());
    assert!(c.says("Pedidos: 1 de 2 pagos"));
    c.press("Pedidos pagos");
    assert!(c.says("200: 1 pagos (Mercado Boa Vista)"), "filtered by the query string: {:?}", c.texts());
    assert!(c.says("Evento pedido.pago versão 2: pending"), "{:?}", c.texts());
    c.press("Receber pagamento assinado");
    assert!(c.says("aviso repetido, nada mudou"), "{:?}", c.texts());

    c.press("ERP fora do ar");
    c.press("Enviar eventos");
    assert!(c.says("0 confirmados, 1 para tentar de novo"), "{:?}", c.texts());
    c.press("ERP fora do ar");
    c.press("Enviar como integra");
    assert!(c.says("1 esperando"), "not again before the backoff: {:?}", c.texts());
    c.press("Enviar daqui a um minuto");
    assert!(c.says("1 confirmados"), "{:?}", c.texts());
    assert!(c.says("O ERP recebeu: matriz/pedido-1"), "{:?}", c.texts());
    c.press("Enviar daqui a um minuto");
    assert!(c.says("0 confirmados, 0 para tentar de novo, 0 esperando"), "never twice: {:?}", c.texts());
}

#[test]
fn the_center_s_own_api_has_an_endpoint_run_as_whoever_calls() {
    use ravenrs_app_server::ApiRequest;
    use serde_json::json;
    let mut api = feature_center::api_service(None).expect("it starts");
    let mut token_of = |name: &str| {
        let body = json!({ "name": name, "password": PASSWORD });
        let answer = api.handle(ApiRequest::new("POST", "/api/auth/token").json(&body));
        answer.body["token"].as_str().expect("a token").to_owned()
    };
    let (bia, caio) = (token_of("bia"), token_of("caio"));
    let answer = api.handle(ApiRequest::new("POST", "/api/run/contagem").bearer(&bia));
    assert_eq!(answer.body["documentos"], 3, "{answer:?}");
    let answer = api.handle(ApiRequest::new("POST", "/api/run/contagem").bearer(&caio));
    assert_eq!(answer.body["documentos"], 2, "caio does not see what is of ana");
    assert_eq!(answer.body["quem"], "caio");
}

// Framework: ready-made modules (fork B).

impl Center {
    /// Waits for work that runs out of the way of the screens, such as a note at the sandbox.
    fn wait_for(&mut self, text: &str) -> bool {
        for _ in 0..400 {
            if self.says(text) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }
}

#[test]
fn the_point_of_sale_scans_keeps_charges_issues_prints_and_swaps_the_note() {
    let mut c = Center::memory("ana");
    c.press("PDV de exemplo");
    c.press("Passar arroz");
    c.press("Passar 3 leites");
    assert!(c.says("Itens: 2 · Total: R$ 44,37"), "27,90 + 3 x 5,49: {:?}", c.texts());
    c.press("Passar código errado");
    assert!(c.says("Problema: Produto 999 não encontrado."), "{:?}", c.texts());
    c.press("Cancelar último");
    assert!(c.says("Itens: 1"));
    c.press("Guardar venda");
    assert!(c.says("Vendas guardadas: 1") && c.says("Itens: 0"), "{:?}", c.texts());
    c.press("Retomar a guardada");
    assert!(c.says("Retomou: 1 itens") && c.says("Vendas guardadas: 0"), "{:?}", c.texts());
    c.press("Avançar (F9)");
    assert!(c.says("Tela: Pagando"));
    c.press("Voltar (Esc)");
    assert!(c.says("Tela: Vendendo"));
    c.press("Voltar (Esc)");
    assert!(c.says("Nada para voltar."));
    c.press("Cancelar venda");
    assert!(c.says("Itens: 0"));

    c.press("Passar arroz");
    c.press("Cobrar no débito com pergunta");
    assert!(c.says("o provedor pergunta"), "{:?}", c.texts());
    c.press("Responder a primeira opção");
    assert!(c.wait_for("Nota 1 autorizada"), "{:?}", c.texts());
    c.press("Imprimir o cupom");
    assert!(c.says("TOTAL R$ 27,90") && c.says("Cartão de débito"), "the receipt itself: {:?}", c.texts());
    c.press("Fechar");
    assert!(c.says("Cupom na tela"), "{:?}", c.texts());
    c.press("Trocar por nota grande");
    assert!(c.wait_for("Ver o DANFE"), "the NFC-e canceled and a NF-e issued: {:?}", c.texts());
    assert!(c.says("Nota grande (NF-e, A4)") && c.says("Consumidor 11444777000161"), "{:?}", c.texts());
    c.press("Conferir pagamentos pendentes");
    assert!(c.says("Pagamento pendente resolvido"), "the one left by an earlier session: {:?}", c.texts());
}

#[test]
fn a_request_goes_step_by_step_once_and_a_lent_role_decides_only_for_its_hour() {
    let mut c = Center::memory("ana");
    c.press("Solicitações");
    assert!(
        c.says("Etapas: Gerência aprova (Gerência, prazo de 24 h) → Diretoria aprova"),
        "{:?}",
        c.texts()
    );
    assert!(c.says("Entidades do pacote: Request, Decision, RequestTask, Delegation"));
    c.press("Ana pede um notebook");
    assert!(c.says("Solicitação 1: Notebook para o caixa (manager)"), "{:?}", c.texts());
    assert!(c.says("Caixa de entrada de bia: 1"));
    c.press("Ana tenta aprovar");
    assert!(c.says("ana foi recusado"), "{:?}", c.texts());
    c.press("Bia aprova");
    assert!(c.says("bia aprovou; agora: board"), "{:?}", c.texts());
    c.press("Bia aprova de novo");
    assert!(c.says("já estava registrada"), "{:?}", c.texts());
    assert!(c.says("Eventos na caixa de saída: 1"), "the event of the decision, once");
    assert!(c.says("Caixa de entrada de caio: 1"));
    c.press("Caio aprova como diretoria");
    assert!(c.says("Solicitação 1: Notebook para o caixa (approved)"), "{:?}", c.texts());

    c.press("Bia empresta a Gerência a Caio");
    assert!(c.says("Caio decide como: Diretoria, Gerência; emprestado: Gerência (lent)"), "{:?}", c.texts());
    c.press("Ana pede um notebook");
    c.press("Caio aprova como gerência");
    assert!(c.says("caio aprovou; agora: board"), "{:?}", c.texts());
    c.press("Ana pede um notebook");
    c.press("Passar duas horas");
    assert!(c.says("Caio decide como: Diretoria; emprestado: nada"), "{:?}", c.texts());
    c.press("Caio aprova como gerência");
    assert!(c.says("caio foi recusado"), "the loan is over: {:?}", c.texts());
}

#[test]
fn the_catalog_of_texts_tells_its_own_from_english_and_takes_texts_on_top() {
    let mut c = Center::memory("ana");
    c.press("Catálogo de textos");
    assert!(c.says("Idiomas: ") && c.says("pt-BR"), "{:?}", c.texts());
    assert!(c.says("Faltam em pt-BR: "));
    assert!(c.says("center.hello: próprio do pt-BR nenhum; com o inglês por trás nenhum"), "{:?}", c.texts());
    c.press("Juntar o JSON do centro");
    assert!(c.says("ui.refresh: próprio do pt-BR Recarregar"), "{:?}", c.texts());
    assert!(c.says("center.hello: próprio do pt-BR Olá, {0}!"), "{:?}", c.texts());
    c.press("Juntar o JSON errado");
    assert!(c.says("Recusado, nada mudou"), "{:?}", c.texts());
    c.press("Juntar o catálogo próprio");
    assert!(c.says("center.bye: próprio do pt-BR Até logo!"));
    assert!(
        c.says("center.only_english: próprio do pt-BR nenhum; com o inglês por trás Only in English"),
        "{:?}",
        c.texts()
    );
    c.press("Pôr um texto");
    assert!(c.says("ui.refresh: próprio do pt-BR Ler de novo"));
    c.press("Recomeçar");
    assert!(c.says("center.bye: próprio do pt-BR nenhum"));
}

#[test]
fn the_accessibility_audit_runs_from_the_center_over_a_shop_put_together_in_code() {
    let mut c = Center::memory("ana");
    c.press("Auditoria de acessibilidade");
    c.press("Listar os itens");
    assert!(c.says("Itens da navegação: ") && c.says("Produtos do caixa"), "{:?}", c.texts());
    let left_out = c.texts().into_iter().find(|t| t.starts_with("Deixando de fora")).expect("the line");
    assert!(!left_out.trim_start_matches("Deixando de fora Produtos do caixa").contains("Produtos do caixa"));
    c.press("Só a tela de abertura");
    assert!(c.says("A tela de abertura: ") && c.says("em 1 telas (abertura)"), "{:?}", c.texts());
    c.press("Sem passar o Tab");
    assert!(c.says("A tela de abertura, sem o Tab: "), "{:?}", c.texts());
    c.press("Auditar um item");
    assert!(c.says("Produtos do caixa: "), "{:?}", c.texts());
    c.press("Auditar a loja inteira");
    assert!(c.says("A loja inteira, com o tema claro e o Novo de cada tela: "), "{:?}", c.texts());
    assert!(c.says("Vendas da loja > Novo"), "it pressed Novo on each screen: {:?}", c.texts());
}

#[test]
fn modules_are_put_together_by_their_manifests_or_refused_saying_why() {
    let mut c = Center::memory("ana");
    c.press("Módulos e manifestos");
    assert!(c.says("Montada: "), "{:?}", c.texts());
    assert!(c.says("Vendas da loja"), "an entity of a Rust struct");
    assert!(c.says("Estoque 2.1.0 · roda em [Desktop, Server]"), "{:?}", c.texts());
    assert!(c.says("papel Estoquistas · migração estoque-001-saldos · tarefa estoque.recontagem"));
    assert!(c.says("Etiquetas 1.0.0 · aceita Estoque"), "{:?}", c.texts());
    assert!(c.says("Serviço dado às Actions: margem de 30 %"));
    for refused in ["Rodar na Web", "Usar Postgres", "Tirar events.status", "Estoque na versão 3"] {
        c.press(refused);
        assert!(c.says("Recusada ao montar"), "{refused}: {:?}", c.texts());
        c.press(refused);
        assert!(c.says("Montada: "), "{refused} undone");
    }
    c.press("Desligar Etiquetas");
    assert!(c.says("Desligado, só para ler: ShelfLabel"), "{:?}", c.texts());
}

#[test]
fn the_ready_made_organizations_tasks_notes_and_files_keep_their_rules() {
    let mut c = Center::memory("ana");
    c.press("Organizações");
    c.run("New", ActionInput::None);
    c.save();
    assert_eq!(c.count("Organization"), 0, "a name is required");
    c.set("name", "Raven Ltda");
    c.save();
    assert_eq!(c.count("Organization"), 1, "{:?}", c.session().error());
    c.press("Tarefas");
    c.run("New", ActionInput::None);
    c.set("subject", "Conferir o estoque");
    c.set("percent_done", 150.0);
    c.save();
    assert_eq!(c.count("Task"), 0, "no more than 100 %");
    c.set("percent_done", 50.0);
    c.save();
    assert_eq!(c.count("Task"), 1, "{:?}", c.session().error());
    c.press("Anotações");
    c.run("New", ActionInput::None);
    c.set("title", "Ligar para o fornecedor");
    c.save();
    assert_eq!(c.count("Note"), 1, "{:?}", c.session().error());
    c.press("Arquivos");
    c.run("New", ActionInput::None);
    c.set("name", "contrato.pdf");
    c.save();
    assert_eq!(c.count("FileRecord"), 0, "a file record needs its file");
}

#[test]
fn a_panel_declared_over_a_struct_adds_up_its_sales() {
    let mut c = Center::memory("ana");
    c.press("Vendas da loja");
    assert!(c.says("Sabão em pó"), "the records the module put in place: {:?}", c.texts());
    c.press("Vendas por mês");
    for shown in [
        "Total vendido",
        "Mercearia: 64.59, Limpeza: 22.39, Bebidas: 18.49",
        "2026-09: 36.69, 2026-10: 31.39, 2026-11: 37.39",
        "Arroz 5 kg: 55.8",
    ] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
}

// Framework: the model in code, security and screens put together in code.

#[test]
fn the_model_inside_says_what_code_reads_of_it() {
    let mut c = Center::memory("ana");
    c.press("O modelo por dentro");
    for shown in [
        "ValidationRules, conferida: sem problemas",
        "O campo age se chama Idade, de 18 a 65",
        "idade",
        "Uma entidade sem chave, conferida: SemChave",
    ] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    c.press("Calculados");
    for shown in [
        "parcela 100",
        "Sem o que veio por caminho: amount, installment, months",
        "Como fica guardado, sem o calculado: amount, months",
        "Só com o valor: amount",
    ] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    c.press("Aparência");
    assert!(
        c.says("Na lista, o nome do atrasado: texto Accent, fundo Warning, negrito sim"),
        "{:?}",
        c.texts()
    );
    assert!(
        c.says("No formulário, o nome do atrasado: texto Danger, fundo nenhuma, negrito sim"),
        "{:?}",
        c.texts()
    );
    c.press("Actions declaradas");
    for shown in [
        "tem o id FieldPlacesDemo.Arquivar e vira a Action Arquivar, que pergunta Arquivar os selecionados?",
        "Alcance só nas listas de Tipos de Action, na lista: sim",
        "Alcance só nas listas de Tipos de Action, no formulário: não",
        "Alcance só nas listas de Tipos de Action, dentro de outro: não",
        "Alcance só na lista de Variantes, nela: sim",
        "Alcance só na lista de Variantes, em Regras: não",
    ] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    c.press("Nós do modelo");
    assert!(
        c.says("Validação, na ordem da navegação: Regras, Avisos e informações, Ao sair do campo"),
        "{:?}",
        c.texts()
    );
    assert!(c.says("índice 3, visível não, índice como texto 3"), "{:?}", c.texts());
    c.press("Consulta");
    assert!(c.says("Tarefas da mais cara para a mais barata: "), "{:?}", c.texts());
    assert!(!c.says("(7 campos lidos)"), "only what was asked for: {:?}", c.texts());
}

#[test]
fn a_rule_of_a_context_stops_only_the_action_that_asks_for_it() {
    let mut c = Center::memory("ana");
    c.press("Regras");
    c.open_list("ValidationRules");
    c.select("ValidationRules", 2.0);
    c.run("DemoSend", ActionInput::None);
    assert!(c.says("Para enviar, Outro precisa de um e-mail"), "{:?}", c.texts());
    assert!(!c.says("Enviado para"), "it did not run");
    c.select("ValidationRules", 1.0);
    c.run("DemoSend", ActionInput::None);
    assert!(c.says("Enviado para ana@exemplo.com."), "{:?}", c.texts());
    c.open_record("ValidationRules", 2.0);
    c.set("code", "");
    c.save();
    assert!(
        c.says("O código é uma letra maiúscula e números"),
        "an empty code is refused too: {:?}",
        c.texts()
    );
    assert_eq!(c.stored("ValidationRules", 2.0).get("code"), Value::Text("B2".into()));
}

#[test]
fn each_field_goes_where_the_model_says() {
    let mut c = Center::memory("ana");
    c.press("Onde cada campo aparece");
    assert!(c.says("Clínica Boa Vista") && c.says("Laboratório Sul"), "{:?}", c.texts());
    assert!(!c.says("Filial fechada"), "the list leaves out what is archived");
    assert!(
        c.has_name("Visitas") && !c.has_name("Contato interno"),
        "a column only on the list: {:?}",
        c.texts()
    );
    assert!(!c.says("chave-que-ninguem-ve"), "a secret shows nowhere");
    c.session().open_detail("FieldPlacesDemo", &Value::Text("C-01".into()));
    c.settle();
    assert!(c.says("Contato interno: Bia"), "on the form: {:?}", c.texts());
    assert!(!c.has_name("Chave de integração") && !c.says("chave-que-ninguem-ve"), "{:?}", c.texts());
    assert!(!c.has_name("Visitas"), "and one only on the list is not on the form");
    assert_eq!(
        c.focused().as_deref().map(|name| name.starts_with("Nome")),
        Some(true),
        "the name takes the keyboard"
    );
    let places = c.session().entity("FieldPlacesDemo").expect("there");
    assert_eq!(places.form_columns, Some(2));
    let editors = c.session().entity("EditorsDemo").expect("there");
    let document = editors.field_named("document").expect("there");
    assert_eq!(
        (document.accepted.clone(), document.most_bytes),
        (vec!["pdf".to_owned(), "txt".to_owned()], Some(1_000_000))
    );

    c.open_list("RelationOwner");
    c.select("RelationOwner", 1.0);
    c.run("CloneObject", ActionInput::None);
    c.save();
    let owners = (c.space)().query("RelationOwner", &Query::all()).expect("reads");
    let numbers: Vec<Value> = owners.iter().map(|owner| owner.get("number")).collect();
    assert!(
        numbers.contains(&Value::Text("PED-0001".into()))
            && numbers.iter().any(|number| *number != Value::Text("PED-0001".into())),
        "the number stays behind: {numbers:?}"
    );
}

#[test]
fn the_appearance_and_the_report_that_come_with_the_application() {
    let mut c = Center::memory("ana");
    c.press("Regras de aparência");
    assert!(c.says("Atrasado"));
    c.press("Relatórios");
    assert!(c.says(feature_center::seed::PEOPLE_REPORT), "the module put it there: {:?}", c.texts());
    let bia = &mut Center::memory("bia");
    bia.open_list("SecuredDocument");
    bia.select("SecuredDocument", 3.0);
    let delete = bia.session().frame().action("Delete").expect("there");
    assert!(!delete.is_enabled(), "Vendas never deletes a document, even her own");
}

#[test]
fn a_company_login_lets_lia_in_and_her_session_answers_in_code() {
    let database = memory_database().unwrap_or_else(|why| panic!("{why}"));
    let mut door = Center::at_the_door(Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>));
    assert!(
        door.says("Quem pode entrar"),
        "the center's door has who can sign in beside it: {:?}",
        door.texts()
    );

    let mut c = Center::memory("ana");
    c.press("Entrada pela empresa");
    assert!(c.says("Empresa Exemplo"), "the portal's own aside: {:?}", c.texts());
    c.press("Voltar à escolha da empresa");
    assert!(c.says("Voltou à escolha da empresa"), "{:?}", c.texts());
    c.type_into("Nome", "rui@outra");
    c.type_into("Senha", "x");
    c.press("Entrar");
    assert!(!c.says("Manutenção do prédio"), "another company's issuer is not taken");
    c.press("Preencher a Lia");
    c.press("Entrar");
    assert!(c.says("Manutenção do prédio"), "in, by the company's login: {:?}", c.texts());

    c.press("Entrar pela empresa");
    assert!(c.says("Entrou lia pela empresa, com Gerência e Contratos"), "{:?}", c.texts());
    c.press("O que ela pode");
    assert!(c.says("Gerência (não administra)"), "{:?}", c.texts());
    assert!(c.says("Muda o modelo: sim. Lê contratos: sim. Apaga contratos: não."), "{:?}", c.texts());
    c.press("Apagar um contrato");
    assert!(c.says("Recusado: A Gerência não apaga contratos."), "{:?}", c.texts());
    c.press("Trocar a própria senha");
    assert!(c.says("Senha trocada; a trilha tem 1 anotações."), "{:?}", c.texts());
    c.press("Trocar a própria senha");
    assert!(c.says("Não trocou"), "the current one changed: {:?}", c.texts());
    c.press("Dar senha provisória ao Rui");
    assert!(c.says("Só quem administra dá senha"), "{:?}", c.texts());
    c.press("Desligar o login da empresa");
    c.press("Recarregar a sessão");
    assert!(c.says("Papéis: nenhum"), "unlinked, she keeps nothing: {:?}", c.texts());
}

#[test]
fn an_application_in_code_has_its_brand_counts_controls_and_records_from_elsewhere() {
    let mut c = Center::memory("ana");
    c.press("Aplicação em código");
    for shown in ["Clínica Vida", "Unidade Centro", "3 consultas hoje"] {
        assert!(c.says(shown), "{shown}: {:?}", c.texts());
    }
    c.press("Consultas de hoje");
    assert!(c.says("Paula Nunes"), "records no storage keeps: {:?}", c.texts());
    c.press("Pacientes");
    assert!(c.says("Marta Reis"), "{:?}", c.texts());
    c.activate("Visão");
    c.activate("Fichas");
    assert!(c.says("Ficha de Marta Reis"), "the list editor of the variant: {:?}", c.texts());
    c.press("Abrir");
    assert!(c.says("Exame de rotina"), "visits from elsewhere: {:?}", c.texts());
    assert!(c.has_name("4 de 5"), "the stars control: {:?}", c.texts());
    c.press("Banco feito do modelo");
    assert!(c.says("banco feito das entidades do modelo"), "{:?}", c.texts());
    c.press("Pacientes");
    assert!(c.says("Otávio Lima"), "the same clinic, kept another way: {:?}", c.texts());
}

#[test]
fn a_window_by_hand_hands_over_files_and_shows_only_what_can_be_done() {
    let mut c = Center::memory("ana");
    c.press("Janela montada à mão");
    assert!(c.says("Luvas") && c.says("Almoxarifado"), "{:?}", c.texts());
    assert!(c.says("1 item sem estoque"), "its count: {:?}", c.texts());
    let buttons = |c: &mut Center, name: &str| {
        c.settle();
        c.ui.access_tree()
            .nodes
            .iter()
            .filter(|n| n.role == Role::Button && n.name.as_deref() == Some(name))
            .count()
    };
    assert_eq!(buttons(&mut c, "Retirar do estoque"), 1, "nothing selected: only the faded one of the bar");
    c.activate("Selecionar Luvas");
    assert_eq!(buttons(&mut c, "Retirar do estoque"), 2, "and the button of its own once it can be used");
    c.press("Abrir no fornecedor");
    assert!(c.says("entregou o endereço https://fornecedor.exemplo/?item=Luvas"), "{:?}", c.texts());
    c.press("Exportar os itens");
    assert!(c.says("A janela entregou itens.csv"), "{:?}", c.texts());
    c.press("Abrir ou fechar a navegação");
    assert!(c.says("Navegação fechada."), "{:?}", c.texts());
}

#[test]
fn the_pieces_of_a_screen_show_a_list_its_own_way_and_a_pane_keeps_contacts() {
    let mut c = Center::memory("ana");
    c.press("Peças de uma tela");
    assert!(c.says("120 un."), "the cell in the screen's words: {:?}", c.texts());
    assert!(c.says("Aventais, chegando hoje"), "a list from elsewhere: {:?}", c.texts());
    let columns = |c: &mut Center, name: &str| {
        c.settle();
        c.ui.access_tree().nodes.iter().filter(|n| n.name.as_deref() == Some(name)).count()
    };
    assert_eq!(columns(&mut c, "Unidade"), 1, "the same on every row, so only the deliveries have it");
    c.press("Cadastrar");
    assert!(c.says("O painel recusou"), "{:?}", c.texts());
    c.type_into("Nome", "Zeca");
    c.type_into("Telefone", "81 9999");
    c.press("Cadastrar");
    assert!(c.says("O painel avisou"), "{:?}", c.texts());
    assert!(c.says("\u{260e} 81 9999"), "{:?}", c.texts());
}
