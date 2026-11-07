//! The actions the center shows, one of each kind.

use ravenrs_app::{
    Action, ActionCategory, ActionInput, ChoiceItem, CriteriaMode, FieldKind, Message, MessageKind, Module,
    Object, Selection, Value, ViewController,
};

const ENTITY: &str = "ActionDemo";

/// Changes these records, keeps them, and has the screens read again.
fn change(
    cx: &ravenrs_app::ActionContext,
    field: &str,
    value: impl Fn(&Object) -> Value,
) -> Result<(), String> {
    let mut space = cx.space.borrow_mut();
    let result = (|| {
        for record in &cx.selected {
            let Some(mut stored) = space.find(ENTITY, &record.get("id"))? else { continue };
            let to = value(&stored);
            stored.set(field, to);
            space.save(stored)?;
        }
        space.commit()
    })();
    if let Err(error) = result {
        space.rollback();
        return Err(error.to_string());
    }
    drop(space);
    cx.records_changed();
    Ok(())
}

struct Showcase;

impl ViewController for Showcase {
    fn id(&self) -> &str {
        "ActionShowcase"
    }

    fn actions(&self) -> Vec<Action> {
        let on_entity = |action: Action| action.target_entity(ENTITY).category(ActionCategory::Edit);
        vec![
            on_entity(Action::simple("DemoSimple", "Simples"))
                .selection(Selection::RequireSingle)
                .on_execute(|cx| {
                    cx.message(Message::new("Rodou a Action simples.", MessageKind::Success));
                    Ok(())
                }),
            on_entity(Action::simple("DemoConfirm", "Com confirmação"))
                .selection(Selection::RequireMultiple)
                .confirmation("Somar 1 ao valor do que está selecionado?")
                .confirmation_words("Dá para desfazer pela Action com valor.", "Somar")
                .on_execute(|cx| {
                    change(cx, "amount", |record| match record.get("amount") {
                        Value::Number(amount) => Value::Number(amount + 1.0),
                        _ => Value::Number(1.0),
                    })?;
                    cx.message(
                        Message::new("Valor somado.", MessageKind::Success)
                            .seconds(8.0)
                            .action("Entendi", || {}),
                    );
                    Ok(())
                }),
            on_entity(Action::parametrized("DemoValue", "Com valor", FieldKind::Integer))
                .selection(Selection::RequireMultiple)
                .on_execute(|cx| {
                    let ActionInput::Parameter(value) = cx.input.clone() else { return Ok(()) };
                    change(cx, "amount", move |_| value.clone())
                }),
            on_entity(Action::single_choice(
                "DemoChoice",
                "Com escolha",
                vec![ChoiceItem::new("low", "Prioridade baixa"), ChoiceItem::new("high", "Prioridade alta")],
            ))
            .selection(Selection::RequireMultiple)
            .on_execute(|cx| {
                let ActionInput::Choice(choice) = cx.input.clone() else { return Ok(()) };
                change(cx, "priority", move |_| Value::Text(choice.clone()))
            }),
            on_entity(Action::popup("DemoPopup", "Com janela", "VariantsDemo_ListView"))
                .selection(Selection::RequireSingle)
                .on_execute(|cx| {
                    let picked = matches!(cx.input, ActionInput::Parameter(_));
                    cx.message(Message::new(
                        if picked { "Escolheu um registro na janela." } else { "Fechou sem escolher." },
                        MessageKind::Information,
                    ));
                    Ok(())
                }),
            on_entity(Action::url("DemoUrl", "Abrir endereço", "https://example.com/?nome={name}"))
                .selection(Selection::RequireSingle),
            on_entity(Action::simple("DemoCriteria", "Só para abertos"))
                .selection(Selection::RequireMultiple)
                .target_criteria("[state] = 'open'", CriteriaMode::TrueForAll)
                .on_execute(|cx| change(cx, "state", |_| Value::Text("closed".into()))),
            // The work runs on a thread of its own: the window shows how far it got and a way to
            // stop it, and the records it hands back are kept here.
            on_entity(Action::background("DemoBackground", "Em segundo plano", |job| {
                let total = job.selected().len().max(1) as f32;
                for (done, record) in job.selected().iter().enumerate() {
                    if job.cancelled() {
                        job.tell("Parou no meio; o que já foi feito ficou.", MessageKind::Warning);
                        return Ok(());
                    }
                    job.progress(
                        done as f32 / total,
                        format!("Somando 10 em {}", record.get("name").to_text()),
                    );
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let mut record = record.clone();
                    let amount = record.get("amount").as_number().unwrap_or(0.0);
                    record.set("amount", amount + 10.0);
                    job.save(record);
                }
                job.tell("Somou 10 em segundo plano.", MessageKind::Success);
                Ok(())
            }))
            .selection(Selection::RequireMultiple),
            Action::simple("DemoInForm", "Carimbar")
                .target_entity(ENTITY)
                .category(ActionCategory::InForm("Tools".into()))
                .on_execute(|cx| {
                    cx.message(Message::new(
                        "Esta Action fica no layout do formulário.",
                        MessageKind::Information,
                    ));
                    Ok(())
                }),
        ]
    }
}

/// Everything of the center's own that is not an entity.
#[derive(Debug)]
pub struct CenterModule;

impl Module for CenterModule {
    fn name(&self) -> &str {
        "Center"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(Showcase)]
    }

    fn model_difference(&self) -> Option<&'static str> {
        Some(
            r#"{ "NavigationItems": { "Sem armazenamento": {
                "$new": true, "Caption": "Sem armazenamento",
                "Simulation": { "$new": true, "Caption": "Simulação de parcelas", "View": "Simulation_DetailView" }
            } } }"#,
        )
    }
}
