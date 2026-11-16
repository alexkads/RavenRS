//! `RequestsModule` and `Step`, every feature: a request goes through two steps, each decided by a
//! role, one with a deadline and one with a rule; a decision sent twice is kept once, one by the
//! wrong person is refused, and a role lent for an hour decides for that hour only. Every decision
//! writes its event in the same commit. It keeps its own records in memory, apart from the center.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs::prelude::*;
use ravenrs_app::{Identity, MemoryDatabase, Module, ObjectSpace, Query, Role, Value};
use ravenrs_app_requests::{
    Decided, Decision, OUTBOX, OutboxModule, Outcome, REQUEST, RequestsModule, Step, forbid_self_decision,
};
use ravenrs_app_ui::Session;

use super::{button, layout};

/// The control's name, as the session knows it.
pub const ID: &str = "RequestsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Solicitações";

const MANAGERS: &str = "Gerência";
const BOARD: &str = "Diretoria";
/// Where the clock of the demonstration starts, in seconds.
const START: i64 = 1_800_000_000;

fn requests() -> RequestsModule {
    let steps = vec![
        Step::new("manager", "Gerência aprova", MANAGERS).due_in(24 * 3600),
        Step::new("board", "Diretoria aprova", BOARD).when("[unit] = 'Loja Centro'"),
    ];
    match RequestsModule::new(steps) {
        Ok(module) => module.policy(forbid_self_decision()),
        Err(why) => unreachable!("two steps are given: {why}"),
    }
}

fn person(name: &str, role: &str) -> Identity {
    Identity::new(name, name, vec![Role::new(role)])
}

/// The records of the demonstration and its clock.
struct Desk {
    module: RequestsModule,
    database: MemoryDatabase,
    at: i64,
    next: i64,
}

impl Desk {
    fn new() -> Result<Self, String> {
        let module = requests();
        let mut entities = module.declared_entities();
        entities.extend(OutboxModule.entities());
        let database = MemoryDatabase::new(entities).map_err(|e| e.0)?;
        Ok(Self { module, database, at: START, next: 1 })
    }

    /// The request being decided, the last one made.
    fn current(&self) -> Option<(i64, i64)> {
        let all = self.database.space().query(REQUEST, &Query::all()).ok()?;
        let last = all.into_iter().last()?;
        let number = |field: &str| match last.get(field) {
            Value::Number(n) => n as i64,
            _ => 0,
        };
        Some((number("id"), number("version")))
    }

    fn submit(&mut self) -> String {
        let mut space = self.database.space();
        let id = self.next;
        match self.module.submit(
            &mut space,
            &person("ana", "Compras"),
            id,
            "Notebook para o caixa",
            "Loja Centro",
            self.at,
        ) {
            Ok(_) => {
                self.next += 1;
                format!("Ana pediu: solicitação {id}, na etapa Gerência aprova.")
            }
            Err(why) => format!("Recusado: {why}"),
        }
    }

    fn decide(&mut self, who: &Identity, step: &str) -> String {
        let Some((request, version)) = self.current() else { return "Faça um pedido antes.".to_owned() };
        let decision = Decision {
            id: format!("{request}/{step}"),
            request,
            version,
            outcome: Outcome::Approve,
            reason: "Dentro do orçamento".to_owned(),
        };
        let mut space = self.database.space();
        match self.module.decide(&mut space, "loja", who, &decision, self.at) {
            Ok(Decided::Recorded { state }) => format!("{} aprovou; agora: {state}.", who.name),
            Ok(Decided::AlreadyRecorded) => {
                format!("{} mandou de novo: já estava registrada, nada mudou.", who.name)
            }
            Err(why) => format!("{} foi recusado: {why}", who.name),
        }
    }

    fn lend(&mut self) -> String {
        let mut space = self.database.space();
        let bia = person("bia", MANAGERS);
        match self.module.delegate(&mut space, &bia, "bia-caio", "caio", MANAGERS, self.at + 3600, self.at) {
            Ok(()) => "Bia emprestou a Gerência a Caio por uma hora.".to_owned(),
            Err(why) => format!("Recusado: {why}"),
        }
    }

    fn lines(&self) -> Vec<String> {
        let space = self.database.space();
        let steps: Vec<String> = self
            .module
            .steps()
            .iter()
            .map(|step| {
                let due = step.due_in.map_or_else(String::new, |s| format!(", prazo de {} h", s / 3600));
                let when = step.when.as_ref().map_or_else(String::new, |w| format!(", só se {w}"));
                format!("{} ({}{due}{when})", step.caption, step.decided_by)
            })
            .collect();
        let entities: Vec<String> = self.module.declared_entities().into_iter().map(|e| e.name).collect();
        let mut lines = vec![
            format!("Etapas: {}", steps.join(" → ")),
            format!("Entidades do pacote: {}", entities.join(", ")),
            format!("Relógio: {} min depois do início", (self.at - START) / 60),
        ];
        if let Ok(all) = space.query(REQUEST, &Query::all()) {
            for request in all {
                lines.push(format!(
                    "Solicitação {}: {} ({})",
                    request.get("id").to_text(),
                    request.get("title").to_text(),
                    request.get("state").to_text()
                ));
            }
        }
        let caio = person("caio", BOARD);
        for who in [person("bia", MANAGERS), caio.clone()] {
            let tasks = self.module.tasks_for(&space, &who, self.at).unwrap_or_default();
            let titles: Vec<String> = tasks.iter().map(|t| t.get("step").to_text()).collect();
            lines.push(format!("Caixa de entrada de {}: {} ({})", who.name, tasks.len(), titles.join(", ")));
        }
        let roles = self.module.roles_of(&space, &caio, self.at).unwrap_or_default();
        let lent: Vec<String> = self
            .module
            .delegated_roles(&space, &caio, self.at)
            .unwrap_or_default()
            .iter()
            .map(|role| role.name().to_owned())
            .collect();
        lines.push(format!(
            "Caio decide como: {}; emprestado: {}",
            roles.join(", "),
            if lent.is_empty() { "nada".to_owned() } else { lent.join(", ") }
        ));
        let events = space.query(OUTBOX, &Query::all()).map(|events| events.len()).unwrap_or(0);
        lines.push(format!("Eventos na caixa de saída: {events}"));
        lines
    }
}

/// The screen over the records of one session.
struct RequestsScreen {
    desk: Rc<RefCell<Option<Desk>>>,
}

impl View for RequestsScreen {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let said = cx.use_state(|| "Comece pelo pedido da Ana.".to_owned());
        let act = |caption: &str, run: fn(&mut Desk) -> String| {
            let (desk, said) = (self.desk.clone(), said.clone());
            button(caption, move || {
                let mut desk = desk.borrow_mut();
                let text = match desk.as_mut() {
                    Some(desk) => run(desk),
                    None => "Sem registros para trabalhar.".to_owned(),
                };
                said.set(text);
            })
        };
        let buttons = vec![
            act("Ana pede um notebook", Desk::submit),
            act("Ana tenta aprovar", |d| d.decide(&person("ana", "Compras"), "manager")),
            act("Bia aprova", |d| d.decide(&person("bia", MANAGERS), "manager")),
            act("Bia aprova de novo", |d| d.decide(&person("bia", MANAGERS), "manager")),
            act("Bia empresta a Gerência a Caio", Desk::lend),
            act("Caio aprova como gerência", |d| d.decide(&person("caio", BOARD), "manager")),
            act("Caio aprova como diretoria", |d| d.decide(&person("caio", BOARD), "board")),
            act("Passar duas horas", |d| {
                d.at += 2 * 3600;
                "Duas horas depois: o empréstimo acabou.".to_owned()
            }),
        ];
        let shown = self.desk.borrow().as_ref().map_or_else(Vec::new, Desk::lines);
        let intro = "Uma solicitação passa pela Gerência e pela Diretoria. Cada etapa só é decidida pelo seu papel, \
                     uma decisão mandada duas vezes fica uma só, e cada decisão grava o seu evento junto.";
        let view = layout(cx, intro, buttons, shown, said.get());
        cx.child(view.as_ref())
    }
}

/// Registers the screen with the session, with records of its own.
pub fn register(session: &Session) {
    let desk = Rc::new(RefCell::new(Desk::new().ok()));
    session.register_control(ID, move || Box::new(RequestsScreen { desk: desk.clone() }) as AnyView);
}
