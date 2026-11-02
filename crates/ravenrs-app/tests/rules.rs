//! Validation and appearance, with no window.
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use ravenrs_app::*;

fn object(entity: &str, values: &[(&str, Value)]) -> Object {
    let values: BTreeMap<String, Value> = values.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect();
    Object::from_values(entity, values)
}

fn base() -> EntityInfo {
    EntityInfo::new("Bill")
        .field(FieldInfo::new("id", FieldKind::Integer).key())
        .field(FieldInfo::new("name", FieldKind::Text).caption("Nome"))
        .field(FieldInfo::new("code", FieldKind::Text).caption("Código"))
        .field(FieldInfo::new("amount", FieldKind::Money).caption("Valor"))
        .field(FieldInfo::new("limit", FieldKind::Money).caption("Limite"))
        .field(FieldInfo::new("paid", FieldKind::Boolean).caption("Paga"))
}

fn bank() -> EntityInfo {
    EntityInfo::new("Bank")
        .field(FieldInfo::new("code", FieldKind::Text).typed_key())
        .field(FieldInfo::new("bill", FieldKind::Integer))
}

/// What fails, for a record of an entity with this rule, with these records already kept.
fn failures(rule: Rule, record: &[(&str, Value)], kept: &[&[(&str, Value)]]) -> Vec<Violation> {
    let entity = base().rule(rule);
    let database = MemoryDatabase::new(vec![entity.clone(), bank()]).expect("valid");
    let mut space = database.space();
    for values in kept {
        let name = if values.iter().any(|(k, _)| *k == "bill") { "Bank" } else { "Bill" };
        space.save(object(name, values)).expect("saves");
    }
    space.commit().expect("commits");
    validate(&entity, &object("Bill", record), CONTEXT_SAVE, &space, &RuleWords::default())
}

fn fails(rule: Rule, record: &[(&str, Value)]) -> bool {
    !failures(rule, record, &[]).is_empty()
}

const ID: (&str, Value) = ("id", Value::Number(1.0));

fn text(field: &'static str, value: &str) -> (&'static str, Value) {
    (field, Value::Text(value.to_owned()))
}

fn number(field: &'static str, value: f64) -> (&'static str, Value) {
    (field, Value::Number(value))
}

#[test]
fn required_wants_something_in_the_field() {
    let rule = || Rule::required("name");
    assert!(fails(rule(), &[ID]));
    assert!(fails(rule(), &[ID, text("name", "   ")]), "blanks are nothing");
    assert!(!fails(rule(), &[ID, text("name", "a")]));
    let found = failures(rule(), &[ID], &[]);
    assert_eq!(found[0].field.as_deref(), Some("name"));
    assert_eq!(found[0].message, "Nome is required", "in the words of the caption");
}

#[test]
fn a_range_has_limits_on_both_sides_and_passes_what_is_empty() {
    let rule = || Rule::range("amount", Some(10.0), Some(20.0));
    assert!(!fails(rule(), &[ID]), "empty is for required to say");
    assert!(!fails(rule(), &[ID, number("amount", 10.0)]));
    assert!(!fails(rule(), &[ID, number("amount", 20.0)]));
    let low = failures(rule(), &[ID, number("amount", 9.0)], &[]);
    assert_eq!(low[0].message, "Valor cannot be below 10");
    let high = failures(rule(), &[ID, number("amount", 21.0)], &[]);
    assert_eq!(high[0].message, "Valor cannot be above 20");
}

#[test]
fn a_pattern_has_to_match() {
    let rule = || Rule::matches("code", "^[A-Z]{2}[0-9]+$");
    assert!(!fails(rule(), &[ID, text("code", "AB12")]));
    assert!(fails(rule(), &[ID, text("code", "ab12")]));
    assert!(!fails(rule(), &[ID]));
}

#[test]
fn text_compares_in_each_way() {
    let rule = |comparison, ignore_case| {
        Rule::on("code", RuleKind::Text { comparison, text: "Ab".to_owned(), ignore_case })
    };
    let cases = [
        (TextComparison::Equals, "Ab", false, true),
        (TextComparison::Equals, "ab", false, false),
        (TextComparison::Equals, "ab", true, true),
        (TextComparison::NotEquals, "Ab", false, false),
        (TextComparison::NotEquals, "x", false, true),
        (TextComparison::Contains, "xAby", false, true),
        (TextComparison::Contains, "xy", false, false),
        (TextComparison::StartsWith, "Abx", false, true),
        (TextComparison::StartsWith, "xAb", false, false),
        (TextComparison::EndsWith, "xAb", false, true),
        (TextComparison::EndsWith, "Abx", false, false),
    ];
    for (comparison, held, ignore, ok) in cases {
        assert_eq!(!fails(rule(comparison, ignore), &[ID, text("code", held)]), ok, "{comparison:?} {held}");
    }
}

#[test]
fn values_compare_with_a_value_or_another_field() {
    let with_value = |comparison| {
        Rule::on("amount", RuleKind::Compare { comparison, with: Operand::Value(Value::Number(5.0)) })
    };
    let at = |n| [ID, number("amount", n)];
    for (comparison, below, same, above) in [
        (Comparison::Equal, false, true, false),
        (Comparison::NotEqual, true, false, true),
        (Comparison::Less, true, false, false),
        (Comparison::LessOrEqual, true, true, false),
        (Comparison::Greater, false, false, true),
        (Comparison::GreaterOrEqual, false, true, true),
    ] {
        assert_eq!(!fails(with_value(comparison), &at(4.0)), below, "{comparison:?} below");
        assert_eq!(!fails(with_value(comparison), &at(5.0)), same, "{comparison:?} same");
        assert_eq!(!fails(with_value(comparison), &at(6.0)), above, "{comparison:?} above");
    }
    let with_field = || {
        Rule::on(
            "amount",
            RuleKind::Compare { comparison: Comparison::LessOrEqual, with: Operand::Field("limit".into()) },
        )
    };
    assert!(!fails(with_field(), &[ID, number("amount", 5.0), number("limit", 5.0)]));
    assert!(fails(with_field(), &[ID, number("amount", 6.0), number("limit", 5.0)]));
}

#[test]
fn an_expression_is_about_the_whole_record() {
    let rule = || Rule::criteria("sane", "[amount] <= [limit]");
    assert!(!fails(rule(), &[ID, number("amount", 1.0), number("limit", 2.0)]));
    let found = failures(rule(), &[ID, number("amount", 3.0), number("limit", 2.0)], &[]);
    assert_eq!(found[0].field, None, "it goes on the list, on no field");
    assert_eq!(found[0].message, "This record is not valid");
}

#[test]
fn true_means_checked() {
    let rule = || Rule::on("paid", RuleKind::IsTrue);
    assert!(fails(rule(), &[ID]));
    assert!(fails(rule(), &[ID, ("paid", Value::Bool(false))]));
    assert!(!fails(rule(), &[ID, ("paid", Value::Bool(true))]));
}

#[test]
fn unique_looks_at_the_other_records_and_not_at_itself() {
    let kept: &[(&str, Value)] = &[number("id", 7.0), text("code", "A")];
    assert_eq!(failures(Rule::unique("code"), &[ID, text("code", "A")], &[kept]).len(), 1);
    assert!(failures(Rule::unique("code"), &[ID, text("code", "B")], &[kept]).is_empty());
    assert!(failures(Rule::unique("code"), &[number("id", 7.0), text("code", "A")], &[kept]).is_empty());
}

#[test]
fn unique_together_needs_all_the_fields_to_repeat() {
    let rule = || Rule::new("pair", RuleKind::UniqueTogether(vec!["code".into(), "name".into()]));
    let kept: &[(&str, Value)] = &[number("id", 7.0), text("code", "A"), text("name", "x")];
    assert_eq!(failures(rule(), &[ID, text("code", "A"), text("name", "x")], &[kept]).len(), 1);
    assert!(failures(rule(), &[ID, text("code", "A"), text("name", "y")], &[kept]).is_empty());
}

#[test]
fn another_record_can_be_required_to_exist() {
    let rule =
        || Rule::new("bank", RuleKind::Exists { entity: "Bank".into(), criteria: Some("[bill] = 9".into()) });
    assert_eq!(failures(rule(), &[ID], &[]).len(), 1);
    let other: &[(&str, Value)] = &[text("code", "341"), number("bill", 8.0)];
    assert_eq!(failures(rule(), &[ID], &[other]).len(), 1, "not this one");
    let right: &[(&str, Value)] = &[text("code", "341"), number("bill", 9.0)];
    assert!(failures(rule(), &[ID], &[right]).is_empty());
}

#[test]
fn a_record_can_be_required_to_be_pointed_to() {
    let rule = || Rule::new("used", RuleKind::Referenced { entity: "Bank".into(), field: "bill".into() });
    assert_eq!(failures(rule(), &[ID], &[]).len(), 1);
    let pointing: &[(&str, Value)] = &[text("code", "341"), number("bill", 1.0)];
    assert!(failures(rule(), &[ID], &[pointing]).is_empty());
}

#[test]
fn a_rule_can_be_anything() {
    let rule = |answer: Result<(), String>| {
        Rule::new("mine", RuleKind::Custom(std::rc::Rc::new(move |_, _| answer.clone())))
    };
    assert!(!fails(rule(Ok(())), &[ID]));
    assert_eq!(failures(rule(Err("no".into())), &[ID], &[])[0].message, "no");
}

#[test]
fn a_rule_applies_in_its_contexts_only() {
    let entity = base()
        .rule(Rule::required("name"))
        .rule(Rule::required("code").contexts([CONTEXT_DELETE]))
        .rule(Rule::on("paid", RuleKind::IsTrue).contexts(["Pay"]));
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let space = database.space();
    let names = |context: &str| -> Vec<Option<String>> {
        validate(&entity, &object("Bill", &[ID]), context, &space, &RuleWords::default())
            .into_iter()
            .map(|v| v.field)
            .collect()
    };
    assert_eq!(names(CONTEXT_SAVE), [Some("name".to_owned())]);
    assert_eq!(names(CONTEXT_DELETE), [Some("code".to_owned())]);
    assert_eq!(names("Pay"), [Some("paid".to_owned())]);
    assert!(names("Other").is_empty());
}

#[test]
fn a_target_criteria_decides_who_the_rule_is_for() {
    let rule = || Rule::required("name").target_criteria("[amount] > 10");
    assert!(!fails(rule(), &[ID, number("amount", 5.0)]));
    assert!(fails(rule(), &[ID, number("amount", 50.0)]));
}

#[test]
fn a_rule_can_say_its_own_words_and_read_calculated_fields() {
    let found = failures(Rule::required("name").message("Dê um nome"), &[ID], &[]);
    assert_eq!(found[0].message, "Dê um nome");
    let entity = base()
        .field(FieldInfo::new("twice", FieldKind::Money).calculated("[amount] * 2"))
        .rule(Rule::range("twice", None, Some(10.0)));
    let database = MemoryDatabase::new(vec![entity.clone()]).expect("valid");
    let record = object("Bill", &[ID, number("amount", 6.0)]);
    let found = validate(&entity, &record, CONTEXT_SAVE, &database.space(), &RuleWords::pt_br());
    assert_eq!(found[0].message, "A twice não pode ser maior que 10".replace("A twice", "Twice"));
}

#[test]
fn a_bad_rule_stops_the_application_from_being_put_together() {
    let check = |rule: Rule| base().rule(rule).check().map_err(|e| e.0);
    assert!(check(Rule::required("nope")).is_err_and(|e| e.contains("nope")));
    assert!(check(Rule::matches("code", "(")).is_err());
    assert!(check(Rule::criteria("x", "[a] =")).is_err_and(|e| e.contains("the rule x")));
    assert!(check(Rule::required("name").target_criteria("[a] =")).is_err());
    assert!(check(Rule::new("p", RuleKind::UniqueTogether(vec!["nope".into()]))).is_err());
    let bad = RuleKind::Compare { comparison: Comparison::Equal, with: Operand::Field("nope".into()) };
    assert!(check(Rule::on("name", bad)).is_err());
    let appearance = base().appearance(AppearanceRule::new("a", ["*"]).criteria("[a] ="));
    assert!(appearance.check().is_err());
}

fn list() -> ViewInfo {
    ViewInfo::new("Bill_ListView", ViewKind::List, Some("Bill"))
}

fn form() -> ViewInfo {
    ViewInfo::new("Bill_DetailView", ViewKind::Detail, Some("Bill"))
}

#[test]
fn appearance_follows_the_record_and_the_name() {
    let entity = base().appearance(
        AppearanceRule::new("late", ["amount", "lim*"])
            .criteria("[amount] > 10")
            .font(Tint::Danger)
            .back(Tint::Warning)
            .bold(),
    );
    let high = object("Bill", &[number("amount", 11.0)]);
    let low = object("Bill", &[number("amount", 1.0)]);
    let on = |name: &str, record: &Object| {
        entity.appearance_of(AppearanceTarget::Item, name, &list(), Some(record))
    };
    let found = on("amount", &high);
    assert_eq!((found.font, found.back, found.bold), (Some(Tint::Danger), Some(Tint::Warning), true));
    assert_eq!(on("limit", &high).font, Some(Tint::Danger), "the star");
    assert_eq!(on("name", &high), Appearance::default(), "not that name");
    assert_eq!(on("amount", &low), Appearance::default(), "not that record");
}

#[test]
fn a_star_first_means_all_but_the_names_after_it() {
    let entity = base().appearance(AppearanceRule::new("most", ["*", "name", "lim*"]).font(Tint::Muted));
    let record = object("Bill", &[]);
    let font = |name: &str| entity.appearance_of(AppearanceTarget::Item, name, &list(), Some(&record)).font;
    assert_eq!(font("amount"), Some(Tint::Muted));
    assert_eq!(font("name"), None);
    assert_eq!(font("limit"), None);
}

#[test]
fn a_function_can_decide_too() {
    let entity = base().appearance(
        AppearanceRule::new("f", ["*"])
            .when(|object| object.get("paid") == Value::Bool(true))
            .font(Tint::Muted),
    );
    let paid = object("Bill", &[("paid", Value::Bool(true))]);
    let open = object("Bill", &[("paid", Value::Bool(false))]);
    assert_eq!(
        entity.appearance_of(AppearanceTarget::Item, "x", &list(), Some(&paid)).font,
        Some(Tint::Muted)
    );
    assert_eq!(entity.appearance_of(AppearanceTarget::Item, "x", &list(), Some(&open)).font, None);
}

#[test]
fn the_highest_priority_wins_what_both_change() {
    let entity = base()
        .appearance(AppearanceRule::new("high", ["*"]).priority(2).font(Tint::Danger))
        .appearance(AppearanceRule::new("low", ["*"]).priority(1).font(Tint::Success).back(Tint::Muted));
    let found = entity.appearance_of(AppearanceTarget::Item, "x", &list(), Some(&object("Bill", &[])));
    assert_eq!(found.font, Some(Tint::Danger));
    assert_eq!(found.back, Some(Tint::Muted), "what only one changes stays");
}

#[test]
fn a_rule_is_for_its_context_and_its_kind_of_target() {
    let entity = base()
        .appearance(AppearanceRule::new("a", ["*"]).context(AppearanceContext::List).font(Tint::Danger))
        .appearance(AppearanceRule::new("b", ["*"]).context(AppearanceContext::Detail).font(Tint::Success))
        .appearance(
            AppearanceRule::new("c", ["*"])
                .context(AppearanceContext::View("Other".into()))
                .font(Tint::Accent),
        )
        .appearance(AppearanceRule::new("d", ["Pay"]).target(AppearanceTarget::Action).enabled(false))
        .appearance(AppearanceRule::new("e", ["Main"]).target(AppearanceTarget::Layout).visible(false));
    let record = object("Bill", &[]);
    let item = |view: &ViewInfo| entity.appearance_of(AppearanceTarget::Item, "x", view, Some(&record));
    assert_eq!(item(&list()).font, Some(Tint::Danger));
    assert_eq!(item(&form()).font, Some(Tint::Success));
    let action = entity.appearance_of(AppearanceTarget::Action, "Pay", &list(), Some(&record));
    assert_eq!(action.enabled, Some(false));
    assert_eq!(entity.appearance_of(AppearanceTarget::Action, "Other", &list(), Some(&record)).enabled, None);
    let layout = entity.appearance_of(AppearanceTarget::Layout, "Main", &form(), Some(&record));
    assert_eq!(layout.visible, Some(false));
}

#[test]
fn with_no_record_only_rules_with_no_condition_apply() {
    let entity = base()
        .appearance(AppearanceRule::new("always", ["a"]).visible(false))
        .appearance(AppearanceRule::new("sometimes", ["b"]).criteria("[paid] = True").visible(false));
    assert_eq!(entity.appearance_of(AppearanceTarget::Item, "a", &list(), None).visible, Some(false));
    assert_eq!(
        entity.appearance_of(AppearanceTarget::Item, "b", &list(), None).visible,
        None,
        "a column is for all rows"
    );
}

#[test]
fn appearance_turns_actions_off_by_what_is_selected() {
    let entity = base().appearance(
        AppearanceRule::new("paid", ["Pay"])
            .target(AppearanceTarget::Action)
            .criteria("[paid] = True")
            .enabled(false),
    );
    let entity2 = base().appearance(
        AppearanceRule::new("hide", ["Pay"])
            .target(AppearanceTarget::Action)
            .criteria("[paid] = True")
            .visible(false),
    );
    struct Bills;
    impl ViewController for Bills {
        fn id(&self) -> &str {
            "Bills"
        }
        fn actions(&self) -> Vec<Action> {
            vec![Action::simple("Pay", "Pagar")]
        }
    }
    struct Part;
    impl Module for Part {
        fn name(&self) -> &str {
            "Part"
        }
        fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
            vec![Box::new(Bills)]
        }
    }
    for (rule_entity, disables) in [(entity, true), (entity2, false)] {
        let database = MemoryDatabase::new(vec![rule_entity.clone()]).expect("valid");
        let application = Application::builder("A").module(Part).entity(rule_entity).build().expect("valid");
        let frame = application
            .new_frame(FrameContext::Main, std::rc::Rc::new(std::cell::RefCell::new(database.space())));
        frame.set_view(Some(ViewInfo::new("Bill_ListView", ViewKind::List, Some("Bill"))));
        let pay = frame.action("Pay").expect("the action");
        assert!(pay.is_enabled() && pay.is_active());
        frame.set_selected(vec![object("Bill", &[("paid", Value::Bool(false))])]);
        assert!(pay.is_enabled() && pay.is_active());
        frame.set_selected(vec![
            object("Bill", &[("paid", Value::Bool(false))]),
            object("Bill", &[("paid", Value::Bool(true))]),
        ]);
        assert!(!pay.is_enabled(), "one that does not fit is enough");
        assert!(pay.is_active(), "still there: on a list, hiding only fades it");
        if !disables {
            frame.set_view(Some(ViewInfo::new("Bill_DetailView", ViewKind::Detail, Some("Bill"))));
            frame.set_selected(vec![object("Bill", &[("paid", Value::Bool(true))])]);
            assert!(!pay.is_active(), "on a form, it goes away");
            frame.set_view(Some(ViewInfo::new("Bill_ListView", ViewKind::List, Some("Bill"))));
        }
        frame.set_selected(Vec::new());
        assert!(pay.is_enabled() && pay.is_active());
    }
}

#[derive(BusinessObject)]
#[rule(criteria = "[amount] <= [limit]", message = "Acima do limite")]
#[rule(unique = "code,name")]
#[rule(criteria = "[amount] > 0", context = "Pay")]
#[appearance(names = "amount", criteria = "[amount] > 10", font = "danger", bold)]
#[appearance(names = "Pay", action, criteria = "[paid] = True", enabled = false)]
#[appearance(names = "Main", layout, context = "detail", visible = false, priority = 3, back = "muted")]
struct Declared {
    #[field(key)]
    id: i64,
    #[field(required, min = 1, max = 10.5, message = "Nome curto")]
    name: String,
    #[field(unique, matches = "^[A-Z]+$", required)]
    code: String,
    #[field(money)]
    amount: i64,
    #[field(money)]
    limit: i64,
    paid: bool,
}

#[test]
fn the_derive_declares_rules_and_appearance() {
    let entity = Declared::entity_info();
    entity.check().expect("valid");
    let kinds: Vec<(Option<&str>, String)> =
        entity.rules.iter().map(|r| (r.field.as_deref(), format!("{:?}", r.kind))).collect();
    assert!(kinds.contains(&(Some("name"), "Required".to_owned())));
    assert!(kinds.contains(&(Some("name"), "Range".to_owned())));
    assert!(kinds.contains(&(Some("code"), "Unique".to_owned())));
    assert!(kinds.contains(&(Some("code"), "Matches".to_owned())));
    assert!(kinds.contains(&(None, "Criteria".to_owned())));
    assert!(kinds.contains(&(None, "UniqueTogether".to_owned())));
    let named = entity.rules.iter().find(|r| r.field.as_deref() == Some("name")).expect("a rule");
    assert_eq!(named.message.as_deref(), Some("Nome curto"));
    let pay = entity.rules.iter().rfind(|r| r.field.is_none() && r.contexts == ["Pay"]);
    assert!(pay.is_some());
    assert_eq!(entity.appearance.len(), 3);
    assert_eq!(entity.appearance[0].font, Some(Tint::Danger));
    assert_eq!(entity.appearance[0].bold, Some(true));
    assert_eq!(entity.appearance[1].target, AppearanceTarget::Action);
    assert_eq!(entity.appearance[1].enabled, Some(false));
    assert_eq!(entity.appearance[2].target, AppearanceTarget::Layout);
    assert_eq!(entity.appearance[2].context, AppearanceContext::Detail);
    assert_eq!(entity.appearance[2].priority, 3);
    assert_eq!(entity.appearance[2].back, Some(Tint::Muted));
    let _ = (Declared { id: 0, name: String::new(), code: String::new(), amount: 0, limit: 0, paid: false })
        .to_object();
}
