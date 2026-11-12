//! The actions the center shows, one of each kind.

use ravenrs::prelude::{AnyView, BuildCx, NodeId, View};
use ravenrs_app::{
    Action, ActionCategory, ActionContext, ActionInput, AppRecordsModule, ChoiceItem, CriteriaMode,
    FieldKind, FieldValue, Message, MessageKind, Module, Notice, Object, ObjectSpace, Query, Selection,
    Value, ViewController, WindowController, drafts, next_number, save_draft, take_draft,
};
use ravenrs_app_ui::{App, Language, Session};
use rust_decimal::Decimal;

use crate::model::CounterProduct;

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
            Action::simple("DemoHello", "Dizer olá")
                .target_entity("TextsDemo")
                .category(ActionCategory::Edit)
                .selection(Selection::RequireSingle)
                .on_execute(|cx| {
                    let session =
                        Session::of(cx).ok_or_else(|| "esta Action precisa das telas".to_owned())?;
                    let name = match cx.selected.first().map(|record| record.get("name")) {
                        Some(Value::Text(name)) => name,
                        _ => String::new(),
                    };
                    cx.message(Message::new(
                        session.texts().format("center.hello", &[&name]),
                        MessageKind::Success,
                    ));
                    Ok(())
                }),
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

/// The action of the accessibility demonstration: words of its own for screen readers, keys, and
/// an announcement of what it did.
struct Accessible;

impl ViewController for Accessible {
    fn id(&self) -> &str {
        "AccessibilityShowcase"
    }

    fn actions(&self) -> Vec<Action> {
        vec![
            Action::simple("AccessibilityCheck", "Conferir")
                .target_entity(crate::model::ACCESSIBILITY_DEMO)
                .category(ActionCategory::Edit)
                .selection(Selection::RequireSingle)
                .icon("check")
                .hotkey("command+shift+k")
                .accessible_description("Marca o selecionado como conferido")
                .on_execute(|cx| {
                    let mut space = cx.space.borrow_mut();
                    let mut names = Vec::new();
                    for record in &cx.selected {
                        let found = space.find(crate::model::ACCESSIBILITY_DEMO, &record.get("id"));
                        let Some(mut stored) = found.map_err(|e| e.to_string())? else { continue };
                        stored.set("checked", true);
                        if let Value::Text(name) = stored.get("name") {
                            names.push(name);
                        }
                        space.save(stored).map_err(|e| e.to_string())?;
                    }
                    space.commit().map_err(|e| e.to_string())?;
                    drop(space);
                    cx.records_changed();
                    if let Some(session) = ravenrs_app_ui::Session::of(cx) {
                        session.announcer().polite(format!("Conferido: {}", names.join(", ")));
                    }
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
        vec![Box::new(Showcase), Box::new(Accessible)]
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

/// The kind of work the counter window keeps to pick up later.
const DRAFT_KIND: &str = "venda";
/// What gets back to the locked counter window.
pub const COUNTER_PIN: &str = "1234";

fn session_of(cx: &ActionContext) -> Result<Session, String> {
    Session::of(cx).ok_or_else(|| "esta ação precisa das telas".to_owned())
}

fn in_a_message(cx: &ActionContext, text: String) {
    cx.message(Message::new(text, MessageKind::Success));
}

/// Hands out the next number of a sequence, with no gaps once committed.
fn take_next_number(cx: &ActionContext) -> Result<(), String> {
    let number = {
        let mut space = cx.space.borrow_mut();
        let number = next_number(&mut *space, "venda").map_err(|why| why.to_string())?;
        space.commit().map_err(|why| why.to_string())?;
        number
    };
    in_a_message(cx, format!("Venda número {number}."));
    Ok(())
}

fn keep_draft(cx: &ActionContext) -> Result<(), String> {
    {
        let mut space = cx.space.borrow_mut();
        save_draft(&mut *space, DRAFT_KIND, "Venda pela metade", "3*7891000100103")
            .map_err(|why| why.to_string())?;
        space.commit().map_err(|why| why.to_string())?;
    }
    in_a_message(cx, "Venda guardada para depois.".to_owned());
    Ok(())
}

fn pick_up_draft(cx: &ActionContext) -> Result<(), String> {
    let taken = {
        let mut space = cx.space.borrow_mut();
        let first = drafts(&*space, DRAFT_KIND).map_err(|why| why.to_string())?.into_iter().next();
        let taken = match first {
            Some(draft) => take_draft(&mut *space, &draft.id).map_err(|why| why.to_string())?,
            None => None,
        };
        space.commit().map_err(|why| why.to_string())?;
        taken
    };
    let text = taken.map_or_else(
        || "Nenhuma venda guardada.".to_owned(),
        |draft| format!("Retomou: {} ({}).", draft.title, draft.payload),
    );
    in_a_message(cx, text);
    Ok(())
}

/// Adds up the prices out of the way of the screens, with no cent lost.
fn add_up_prices(cx: &ActionContext) -> Result<(), String> {
    let session = session_of(cx)?;
    let records = cx.space.borrow().query("CounterProduct", &Query::all()).map_err(|why| why.to_string())?;
    let prices: Vec<Decimal> =
        records.iter().map(|record| Decimal::from_value(record.get("price"))).collect();
    session.run_background(
        "Somando os preços",
        move |progress| {
            let mut total = Decimal::ZERO;
            for (index, price) in prices.iter().enumerate() {
                if progress.is_canceled() {
                    break;
                }
                total += price;
                progress.report((index + 1) as f32 / prices.len() as f32, price.to_string());
            }
            total
        },
        |session, total| {
            session.show_message(Message::new(format!("Total exato: {total}"), MessageKind::Success));
        },
    );
    Ok(())
}

struct CounterKeys;

impl WindowController for CounterKeys {
    fn id(&self) -> &str {
        "CounterKeys"
    }

    fn actions(&self) -> Vec<Action> {
        let key = |id: &str, caption: &str, keys: &str| {
            Action::simple(id, caption).hotkey(keys).category(ActionCategory::View)
        };
        vec![
            key("Counter.Shortcuts", "Atalhos", "f1").on_execute(|cx| {
                session_of(cx)?.show_shortcuts();
                Ok(())
            }),
            key("Counter.Number", "Próximo número", "f9").on_execute(take_next_number),
            key("Counter.Keep", "Guardar venda", "f10").on_execute(keep_draft),
            key("Counter.PickUp", "Retomar venda", "f11").on_execute(pick_up_draft),
            key("Counter.Sum", "Somar preços", "alt+s").on_execute(add_up_prices),
            key("Counter.Lock", "Bloquear", "f12").on_execute(|cx| {
                session_of(cx)?.lock("Caixa bloqueado. O PIN é 1234.", |typed| typed == COUNTER_PIN);
                Ok(())
            }),
        ]
    }
}

/// The keys of the counter window, and what it wants everyone to know.
struct CounterModule;

impl Module for CounterModule {
    fn name(&self) -> &str {
        "Counter"
    }

    fn window_controllers(&self) -> Vec<Box<dyn WindowController>> {
        vec![Box::new(CounterKeys)]
    }

    fn notices(&self) -> Vec<Notice> {
        vec![Notice::warning(
            "Demonstração",
            "Este aviso vem do módulo, fica em toda janela e não tem como fechar.",
        )]
    }
}

fn seed_counter(space: &mut dyn ObjectSpace) -> Result<(), String> {
    let products = [
        ("Arroz branco, 5 kg", "7891000053508", 27.9),
        ("Feijão carioca, 1 kg", "7891000244104", 8.79),
        ("Bala de hortelã", "7891000100103", 0.1),
        ("Bala de mel", "7891000100202", 0.2),
    ];
    for (name, gtin, price) in products {
        let mut record = space.new_object("CounterProduct").map_err(|why| why.to_string())?;
        record.set("name", name.to_owned());
        record.set("gtin", gtin.to_owned());
        record.set("price", price);
        space.save(record).map_err(|why| why.to_string())?;
    }
    space.commit().map_err(|why| why.to_string())
}

/// A small application in the arrangement for a counter: keys always in sight, a notice from its
/// module, numbers, drafts, work in the background and a locked window. It has its own records, so
/// its notice and its keys stay out of the rest of the center.
pub fn counter_app() -> App {
    App::new("Caixa de exemplo")
        .language(Language::BrazilianPortuguese)
        .counter()
        .module(AppRecordsModule)
        .module(CounterModule)
        .entity::<CounterProduct>()
        .seed(seed_counter)
}

/// One view built once and shown again on each rebuild, for a control that holds a running application.
#[derive(Clone)]
struct Kept(std::rc::Rc<AnyView>);

impl View for Kept {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(self.0.as_ref().as_ref())
    }
}

/// What makes the control that shows [`counter_app`] inside the center.
pub fn counter_control() -> impl Fn() -> AnyView + 'static {
    let kept = Kept(std::rc::Rc::new(counter_app().into_view()));
    move || Box::new(kept.clone()) as AnyView
}
