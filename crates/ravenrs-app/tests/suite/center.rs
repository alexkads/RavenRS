//! Rules that only warn, messages with what the record holds, changes that meet, fields added in
//! the model, made-up keys and captions that go out for translating.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ravenrs_app::*;

fn product() -> EntityInfo {
    EntityInfo::new("Product")
        .caption("Product")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Name"))
        .field(FieldInfo::new("price", FieldKind::Money).caption("Price"))
        .field(FieldInfo::new("maker", FieldKind::Reference("Maker".into())).caption("Maker"))
}

fn maker() -> EntityInfo {
    EntityInfo::new("Maker")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text))
}

fn database() -> MemoryDatabase {
    MemoryDatabase::new(vec![product(), maker()]).expect("valid")
}

fn record(space: &mut MemoryObjectSpace, name: &str, price: f64) -> Object {
    let mut object = space.new_object("Product").expect("known");
    object.set("name", name);
    object.set("price", price);
    object
}

#[test]
fn a_rule_says_how_much_it_matters() {
    let entity = product()
        .rule(Rule::required("name"))
        .rule(Rule::range("price", Some(100.0), None).warning())
        .rule(Rule::criteria("cheap", "[price] > 1000").information());
    let db = database();
    let mut space = db.space();
    let found =
        validate(&entity, &record(&mut space, "Mesa", 50.0), CONTEXT_SAVE, &space, &RuleWords::default());
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[1].severity, Severity::Information);
    assert!(found.iter().all(|violation| !violation.is_error()));
    let empty = record(&mut space, "", 500.0);
    let found = validate(&entity, &empty, CONTEXT_SAVE, &space, &RuleWords::default());
    assert!(found[0].is_error(), "the usual is an error: {found:?}");
}

#[test]
fn a_severity_can_be_set_in_the_model() {
    let application = Application::builder("Test")
        .entity(product())
        .entity(maker())
        .module({
            struct Rules;
            impl Module for Rules {
                fn name(&self) -> &str {
                    "Rules"
                }
                fn model_difference(&self) -> Option<&'static str> {
                    Some(
                        r#"{ "Entities": { "Product": { "Rules": { "low": { "$new": true, "Kind": "Range",
                        "Field": "price", "Min": 100, "Severity": "Warning" } } } } }"#,
                    )
                }
            }
            Rules
        })
        .build()
        .expect("valid");
    let entity = application.entities().iter().find(|e| e.name == "Product").expect("there");
    assert_eq!(entity.rules[0].severity, Severity::Warning);
}

#[test]
fn a_message_can_say_what_the_record_holds() {
    let entity =
        product().rule(Rule::range("price", Some(100.0), Some(900.0)).message(
            "{$field} of {name} from {maker.name} is {price}, not {$min} to {$max}: {$entity}. {nope}",
        ));
    let db = database();
    let mut space = db.space();
    let mut acme = space.new_object("Maker").expect("known");
    acme.set("name", "Acme");
    let key = acme.get("id");
    space.save(acme).expect("saves");
    let mut object = record(&mut space, "Mesa", 12345.0);
    object.set("maker", key);
    let found = validate(&entity, &object, CONTEXT_SAVE, &space, &RuleWords::default());
    assert_eq!(
        found[0].message, "Price of Mesa from Acme is 123.45, not 100 to 900: Product. {nope}",
        "what it does not know stays as written"
    );
    let found = validate(&entity, &object, CONTEXT_SAVE, &space, &RuleWords::pt_br());
    assert!(found[0].message.contains("123,45"), "money as people write it: {}", found[0].message);
}

#[test]
fn checking_in_place_leaves_out_what_reads_other_records() {
    let entity = product().rule(Rule::unique("name")).rule(Rule::required("name"));
    let db = database();
    let mut space = db.space();
    let first = record(&mut space, "Mesa", 1.0);
    space.save(first).expect("saves");
    let twin = record(&mut space, "Mesa", 1.0);
    let words = RuleWords::default();
    assert_eq!(validate(&entity, &twin, CONTEXT_SAVE, &space, &words).len(), 1);
    assert!(validate_in_place(&entity, &twin, CONTEXT_SAVE, &space, &words).is_empty());
    let empty = record(&mut space, "", 1.0);
    assert_eq!(validate_in_place(&entity, &empty, CONTEXT_SAVE, &space, &words).len(), 1);
}

#[test]
fn changes_to_different_fields_both_stand() {
    let entity = product();
    let db = database();
    let mut space = db.space();
    let original = record(&mut space, "Mesa", 100.0);
    let mut mine = original.clone();
    mine.set("name", "Mesa grande");
    let mut stored = original.clone();
    stored.set("price", 200.0);
    let merged = merge_changes(&entity, &original, &mine, &stored);
    assert!(merged.conflicts.is_empty());
    assert_eq!(merged.record.get("name"), Value::Text("Mesa grande".into()));
    assert_eq!(merged.record.get("price"), Value::Number(200.0));
}

#[test]
fn changes_to_the_same_field_take_a_decision_unless_they_agree() {
    let entity = product();
    let db = database();
    let mut space = db.space();
    let original = record(&mut space, "Mesa", 100.0);
    let (mut mine, mut stored) = (original.clone(), original.clone());
    mine.set("name", "Minha");
    stored.set("name", "Dela");
    let merged = merge_changes(&entity, &original, &mine, &stored);
    assert_eq!(merged.conflicts, ["name"]);
    assert_eq!(merged.record.get("name"), Value::Text("Minha".into()), "this person's, if kept");
    stored.set("name", "Minha");
    assert!(merge_changes(&entity, &original, &mine, &stored).conflicts.is_empty());
}

fn with_city() -> Application {
    let mut application =
        Application::builder("Test").entity(product()).entity(maker()).build().expect("valid");
    let difference = Diff::parse(
        r#"{ "Entities": { "Product": { "Members": {
            "city": { "$new": true, "Kind": "Text", "Caption": "City" },
            "taxed": { "$new": true, "Kind": "Money", "Caption": "Taxed", "Expression": "[price] * 2" },
            "size": { "$new": true, "Kind": "Enum", "Options": { "$new": true,
                "s": { "$new": true, "Caption": "Small" }, "b": { "$new": true, "Caption": "Big" } } }
        } } } }"#,
    )
    .expect("reads");
    application.layers_mut().put(&ModelLayer::Administrator, difference);
    application.refresh_model().expect("valid");
    application
}

#[test]
fn a_field_added_in_the_model_is_a_field_with_a_column_and_a_place_on_the_form() {
    let application = with_city();
    let entity = application.entities().iter().find(|e| e.name == "Product").expect("there").clone();
    assert_eq!(entity.field_named("city").map(|f| f.caption.as_str()), Some("City"));
    assert_eq!(entity.field_named("taxed").and_then(|f| f.calculated.as_deref()), Some("[price] * 2"));
    let Some(FieldKind::Enum(options)) = entity.field_named("size").map(|f| f.kind.clone()) else {
        panic!("options");
    };
    assert!(options.iter().any(|option| option.code == "b" && option.caption == "Big"));
    let model = application.model();
    let columns = model.at(&["Views", "Product_ListView", "Columns"]).expect("columns");
    let shown: Vec<&str> = columns.in_order().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(shown[shown.len() - 3..], ["city", "size", "taxed"], "by name, unless they say an index");
    let main = model.at(&["Views", "Product_DetailView", "Layout", "Main"]).expect("the group");
    assert!(main.get("city").is_some());
    assert!(model.at(&["Views", "Product_DetailView", "Items", "taxed"]).is_some());
    let lookup = model.at(&["Views", "Product_LookupListView", "Columns", "city"]).expect("known");
    assert_eq!(lookup.integer("Index"), Some(-1), "out of sight when picking");

    let db = MemoryDatabase::new(application.entities().to_vec()).expect("valid");
    let mut space = db.space();
    let mut object = space.new_object("Product").expect("known");
    object.set("price", 100.0);
    object.set("city", "Recife");
    let key = object.get("id");
    space.save(object).expect("saves");
    space.commit().expect("keeps");
    let found = db.space().find("Product", &key).expect("reads").expect("there");
    assert_eq!(found.get("taxed"), Value::Number(200.0));
    assert_eq!(found.get("city"), Value::Text("Recife".into()));
}

#[test]
fn a_field_of_a_kind_the_model_cannot_add_says_so() {
    let mut application = Application::builder("Test").entity(maker()).build().expect("valid");
    let difference = Diff::parse(
        r#"{ "Entities": { "Maker": { "Members": { "x": { "$new": true, "Kind": "Reference" } } } } }"#,
    )
    .expect("reads");
    application.layers_mut().put(&ModelLayer::Administrator, difference);
    let error = application.refresh_model().expect_err("refused");
    assert!(error.0.contains("Maker") && error.0.contains("Reference"), "{}", error.0);
}

#[test]
fn a_guid_key_is_made_up_for_each_record() {
    let entity = EntityInfo::new("Note")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("text", FieldKind::Text));
    let db = MemoryDatabase::new(vec![entity]).expect("valid");
    let mut space = db.space();
    let (first, second) =
        (space.new_object("Note").expect("known"), space.new_object("Note").expect("known"));
    assert_ne!(first.get("id"), second.get("id"));
    let key = first.get("id");
    space.save(first).expect("saves");
    space.commit().expect("keeps");
    assert!(db.space().find("Note", &key).expect("reads").is_some());
}

#[test]
fn a_query_can_ask_for_only_some_fields() {
    let db = database();
    let mut space = db.space();
    let object = record(&mut space, "Mesa", 1.0);
    space.save(object).expect("saves");
    let found = space.query("Product", &Query::all().only(["name"])).expect("reads");
    assert_eq!(found[0].values().len(), 2, "{:?}", found[0].values());
    assert_eq!(found[0].get("price"), Value::Null);
}

#[test]
fn captions_go_out_for_translating_and_come_back() {
    let entity = product().navigation("Sales").variant(ViewVariant::new("all", "All, every one"));
    let mut application = Application::builder("Test")
        .entity(entity)
        .entity(maker())
        .dashboard(DashboardInfo::new("Home", "Home page").navigation("Sales"))
        .build()
        .expect("valid");
    let csv = application.export_captions();
    assert!(csv.starts_with("path,caption,translation\n"));
    assert!(csv.contains("Entities/Product/Members/name,Name,\n"), "{csv}");
    assert!(csv.contains("Views/Home_DashboardView,Home page,\n"), "{csv}");
    assert!(csv.contains("Views/Product_ListView/Variants/all,\"All, every one\",\n"), "{csv}");
    assert!(csv.contains("NavigationItems/Sales,Sales,\n"), "{csv}");
    assert!(!csv.contains("Views/Product_ListView/Columns"), "they follow the entity: {csv}");
    assert!(!csv.contains("NavigationItems/Sales/Product"), "and so does its place: {csv}");

    let filled = csv
        .replace("Entities/Product/Members/name,Name,", "Entities/Product/Members/name,Name,Nome")
        .replace("NavigationItems/Sales,Sales,", "NavigationItems/Sales,Sales,Vendas");
    application.import_captions("pt-BR", &filled).expect("reads");
    application.set_language("pt-BR").expect("valid");
    let entity = application.entities().iter().find(|e| e.name == "Product").expect("there");
    assert_eq!(entity.field_named("name").map(|f| f.caption.as_str()), Some("Nome"));
    let model = application.model();
    assert_eq!(model.at(&["NavigationItems", "Sales"]).and_then(|n| n.text("Caption")), Some("Vendas"));
    assert_eq!(
        model.at(&["Views", "Product_ListView", "Columns", "name"]).and_then(|n| n.text("Caption")),
        Some("Nome"),
        "what follows from it comes along"
    );
    assert!(application.unusable().is_empty(), "{:?}", application.unusable());
    application.set_language("en").expect("valid");
    let entity = application.entities().iter().find(|e| e.name == "Product").expect("there");
    assert_eq!(entity.field_named("name").map(|f| f.caption.as_str()), Some("Name"));
}

#[test]
fn a_form_can_have_a_place_for_actions_and_groups_with_titles() {
    let entity = product()
        .form_actions("Tools")
        .field(FieldInfo::new("street", FieldKind::Text).group("Address"))
        .field(FieldInfo::new("number", FieldKind::Integer).group("Address"));
    let model = generate_model(&[entity, maker()]).expect("valid");
    let layout = model.at(&["Views", "Product_DetailView", "Layout"]).expect("there");
    let address = layout.get("Address").expect("a group");
    assert_eq!(address.text("Caption"), Some("Address"));
    assert!(address.get("street").is_some() && address.get("number").is_some());
    assert!(layout.at(&["Main", "street"]).is_none(), "not with the others");
    let tools = layout.at(&["Actions_Tools", "Tools"]).expect("a place");
    assert_eq!(tools.text("Kind"), Some("ActionContainer"));
}

#[test]
fn a_dashboard_can_change_to_another_and_says_if_it_does_not_exist() {
    let first = DashboardInfo::new("A", "A").variant("a", "A", "A").variant("b", "B", "B");
    let good = Application::builder("T")
        .entity(maker())
        .dashboard(first.clone())
        .dashboard(DashboardInfo::new("B", "B"))
        .build()
        .expect("valid");
    let model = good.model();
    let variant = model.at(&["Views", "A_DashboardView", "Variants", "b"]).expect("there");
    assert_eq!(variant.text("View"), Some("B_DashboardView"));
    let error =
        Application::builder("T").entity(maker()).dashboard(first).build().map(|_| ()).expect_err("refused");
    assert!(error.0.contains("B_DashboardView"), "{}", error.0);
}

#[test]
fn an_action_can_tell_people_something() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let told: Rc<RefCell<Vec<(String, MessageKind)>>> = Rc::default();
    struct Teller;
    impl ViewController for Teller {
        fn id(&self) -> &str {
            "Teller"
        }
        fn actions(&self) -> Vec<Action> {
            vec![Action::simple("Tell", "Tell").on_execute(|cx| {
                cx.message(Message::new("Done", MessageKind::Success).seconds(2.0));
                Ok(())
            })]
        }
    }
    struct Telling;
    impl Module for Telling {
        fn name(&self) -> &str {
            "Telling"
        }
        fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
            vec![Box::new(Teller)]
        }
    }
    let application = Application::builder("T").entity(maker()).module(Telling).build().expect("valid");
    let db = MemoryDatabase::new(application.entities().to_vec()).expect("valid");
    let space: Rc<RefCell<dyn ObjectSpace>> = Rc::new(RefCell::new(db.space()));
    let frame = application.new_frame(FrameContext::Main, space);
    frame.show_view("Maker_ListView").expect("shows");
    // Nobody listening is not a failure.
    frame.execute("Tell", ActionInput::None).expect("runs");
    let store = told.clone();
    frame.set_messenger(move |message| store.borrow_mut().push((message.text.clone(), message.kind)));
    frame.execute("Tell", ActionInput::None).expect("runs");
    assert_eq!(*told.borrow(), [("Done".to_owned(), MessageKind::Success)]);
}
