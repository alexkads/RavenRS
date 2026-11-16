//! The web API and what it trades with others, every feature of `ApiService`, `ApiRequest`,
//! `ApiOptions` (in `src/bin/api.rs`), `Webhook`, `Event` and `Dispatcher`, with no network: each
//! request is handed to the service in memory, as `cargo run --bin api` would over HTTP.
//!
//! A payment service tells the API, signed, that an order was paid. The order changes and an
//! event goes out in the same commit, and the dispatcher sends it to a destination that can be
//! down, trying again after a while and never twice.

use ravenrs::prelude::AnyView;

/// The control's name, as the session knows it.
pub const ID: &str = "IntegrationsDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Integrações";

/// What the page shows.
pub fn page() -> AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    return Box::new(desktop::IntegrationsPage);
    #[cfg(target_arch = "wasm32")]
    super::desktop_only(
        "A API web é um servidor, que só o desktop roda: lá esta tela entra com um código, recebe um webhook assinado e envia os eventos que ele gerou.",
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub use desktop::center_endpoint;

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use ravenrs::prelude::*;
    use ravenrs_app::{
        AuthError, Authenticator, CAN_TELL_STATUS, Credentials, Dispatcher, EntityInfo, Event, EventSink,
        FieldInfo, FieldKind, Identity, MemoryDatabase, OUTBOX_EVENT, ObjectSpace, ObjectSpaceExt, Provider,
        Query, Role, RoleMerge, SecurityContext, SpaceError, SpaceMaker, UserDirectory, Value, local_now,
        outbox_entity, security_entities,
    };
    use ravenrs_app_server::{
        ApiRequest, ApiService, Failure, ORGANIZATION_HEADER, SIGNATURE_HEADER, Webhook, sign,
        webhook_event_entity,
    };
    use ravenrs_criteria::DateTime;
    use serde_json::{Value as Json, json};

    const ORDER: &str = "Order";
    /// The one who signs in for the payment service.
    const WHO: &str = "integra";
    /// What the payment service signs its notifications with, for the head office.
    const SECRET: &str = "segredo-da-matriz";
    /// The code of the day, which signs `integra` in instead of a password.
    pub(super) const DAY_CODE: &str = "246810";

    fn orders() -> EntityInfo {
        EntityInfo::new(ORDER)
            .caption("Pedidos")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("customer", FieldKind::Text).caption("Cliente").default_property())
            .field(FieldInfo::new("paid", FieldKind::Boolean).caption("Pago"))
    }

    /// Signs in with the code of the day instead of a password, as a token from another service
    /// would; who it is and their roles still come from the users kept here.
    struct DayCode(UserDirectory);

    impl Authenticator for DayCode {
        fn authenticate(&self, credentials: &Credentials) -> Result<Identity, AuthError> {
            if credentials.password != DAY_CODE {
                return Err(AuthError::Rejected);
            }
            self.0
                .identity_of(&credentials.name)
                .map_err(|why| AuthError::Storage(why.to_string()))?
                .ok_or(AuthError::Rejected)
        }
    }

    /// Where events go: it keeps what it received, can say whether it has one, and can be down.
    #[derive(Default)]
    struct Destination {
        received: RefCell<Vec<String>>,
        down: Cell<bool>,
    }

    impl Provider for Destination {
        fn name(&self) -> &str {
            "ERP de exemplo"
        }

        fn capabilities(&self) -> Vec<String> {
            vec![CAN_TELL_STATUS.to_owned()]
        }
    }

    impl EventSink for Destination {
        fn send(&self, key: &str, _payload: &str) -> Result<(), SpaceError> {
            if self.down.get() {
                return Err(SpaceError::Temporary("o ERP está fora do ar".to_owned()));
            }
            self.received.borrow_mut().push(key.to_owned());
            Ok(())
        }

        fn status(&self, key: &str) -> Result<bool, SpaceError> {
            Ok(self.received.borrow().iter().any(|own| own == key))
        }
    }

    /// Marks the order of a notification as paid and raises its event, in the same unit of work.
    fn paid(context: &ravenrs_app_server::WebhookContext<'_>, data: &Json) -> Result<(), Failure> {
        let number =
            data.get("pedido").and_then(Json::as_f64).ok_or_else(|| Failure::bad_request("Qual pedido?"))?;
        let mut space = context.space.borrow_mut();
        let mut order = space.find(ORDER, &Value::Number(number))?.ok_or_else(Failure::not_found)?;
        order.set("paid", true);
        space.save(order)?;
        let event = Event::new(context.organization, "pedido.pago", context.subject, data.to_string())
            .version(2)
            .at(local_now());
        space.emit(&event)?;
        Ok(())
    }

    /// The service, its records and the destination of its events.
    #[derive(Clone)]
    struct Integration {
        service: Rc<RefCell<ApiService>>,
        database: MemoryDatabase,
        directory: UserDirectory,
        destination: Rc<Destination>,
        token: Rc<RefCell<Option<String>>>,
    }

    fn open() -> Result<Integration, String> {
        let mut entities = security_entities();
        entities.extend([webhook_event_entity(), outbox_entity(), orders()]);
        let database = MemoryDatabase::new(entities).map_err(|e| e.0)?;
        let kept = database.clone();
        let space: SpaceMaker = Rc::new(move || Box::new(kept.space()) as Box<dyn ObjectSpace>);
        let roles = vec![Role::new("Administradores").administrative()];
        let directory = UserDirectory::new(space.clone(), roles.clone()).map_err(|e| e.to_string())?;
        directory.create_first_administrator(WHO, "senha-longa-da-integracao").map_err(|e| e.to_string())?;
        let mut records = space();
        for customer in ["Mercado Boa Vista", "Padaria Central"] {
            let mut order = records.new_object(ORDER).map_err(|e| e.to_string())?;
            order.set("customer", customer);
            order.set("paid", false);
            records.save(order).map_err(|e| e.to_string())?;
        }
        records.commit().map_err(|e| e.to_string())?;
        let service = ApiService::new(space, roles)
            .map_err(|e| e.to_string())?
            .title("Integrações do centro")
            .role_merge(RoleMerge::AllRoles)
            .authenticator(Rc::new(DayCode(directory.clone())))
            // The events are the dispatcher's business, not a client's.
            .hide(OUTBOX_EVENT)
            .session_lifetime(Duration::from_secs(15 * 60))
            .sign_ins_a_minute(3)
            .webhook("pagamentos", Webhook::new(paid).secret("matriz", SECRET))
            .endpoint("pendentes", |context, _| {
                let orders = context.space.borrow().query(ORDER, &Query::all())?;
                let waiting = orders.iter().filter(|order| order.get("paid") != Value::Bool(true)).count();
                Ok(json!({ "pendentes": waiting, "quem": context.name }))
            });
        Ok(Integration {
            service: Rc::new(RefCell::new(service)),
            database,
            directory,
            destination: Rc::default(),
            token: Rc::default(),
        })
    }

    /// What the dispatcher of the head office does: three tries, 30 s after the first failure and
    /// twice that after each one, and a minute for one send.
    fn dispatcher() -> Dispatcher {
        Dispatcher::new("matriz", 3).backoff(30).lease(60)
    }

    fn report(done: Result<ravenrs_app::Report, SpaceError>) -> String {
        match done {
            Ok(report) => format!(
                "Eventos: {} confirmados, {} para tentar de novo, {} esperando, {} sem resposta, {} falharam",
                report.confirmed, report.retry, report.waiting, report.unknown, report.failed
            ),
            Err(why) => format!("Não enviou: {why}"),
        }
    }

    fn a_minute_from_now() -> DateTime {
        let now = local_now();
        let at = now.days * 86_400 + i64::from(now.seconds) + 60;
        DateTime { days: at.div_euclid(86_400), seconds: at.rem_euclid(86_400) as u32 }
    }

    /// The notification of the payment of order 1, as the payment service sends it.
    fn notification() -> Vec<u8> {
        json!({ "id": "evt-1", "subject": "pedido-1", "at": 1, "data": { "pedido": 1 } })
            .to_string()
            .into_bytes()
    }

    fn answer(integration: &Integration, request: ApiRequest) -> (u16, Json) {
        let answered = integration.service.borrow_mut().handle(request);
        (answered.status, answered.body)
    }

    /// The center's own API gets an endpoint too, `POST /api/run/contagem`: how many documents
    /// whoever calls can read, under their permissions.
    pub fn center_endpoint(service: ApiService) -> ApiService {
        service.endpoint("contagem", |context, _| {
            let documents = context.space.borrow().query("SecuredDocument", &Query::all())?;
            Ok(json!({ "documentos": documents.len(), "quem": context.name }))
        })
    }

    /// The page: the API in memory, the notifications it takes and the events it sends.
    #[derive(Debug, Default)]
    pub struct IntegrationsPage;

    impl View for IntegrationsPage {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let integration = cx.use_state(|| open().map_err(|why| format!("A integração não abriu: {why}")));
            let said = cx.use_state(|| "Entre com o código do dia e receba o aviso de pagamento.".to_owned());
            let Ok(now) = integration.get() else {
                return cx.child(&Text::new(integration.get().err().unwrap_or_default()));
            };
            let act = |caption: &str, primary: bool, run: Box<dyn Fn(&Integration) -> String>| -> AnyView {
                let (now, said, again) = (now.clone(), said.clone(), integration.clone());
                let button = Button::new(caption.to_owned()).on_click(move || {
                    said.set(run(&now));
                    again.set(Ok(now.clone()));
                });
                Box::new(if primary { button } else { button.style(ButtonStyle::Secondary) })
            };
            let buttons = vec![
                act(
                    "Entrar com o código",
                    true,
                    Box::new(|it| {
                        let body = json!({ "name": WHO, "password": DAY_CODE });
                        let (status, body) = answer(
                            it,
                            ApiRequest::new("POST", "/api/auth/token").client("10.0.0.7").json(&body),
                        );
                        let token = body["token"].as_str().map(str::to_owned);
                        let line = match &token {
                            Some(_) => format!("Entrou ({status}); o token vale 15 minutos"),
                            None => format!("Recusado ({status})"),
                        };
                        *it.token.borrow_mut() = token;
                        line
                    }),
                ),
                act(
                    "Entrar com código errado",
                    false,
                    Box::new(|it| {
                        let body = json!({ "name": WHO, "password": "000000" });
                        let request =
                            ApiRequest::new("POST", "/api/auth/token").client("10.0.0.9").json(&body);
                        match answer(it, request).0 {
                            429 => "429: tentativas demais deste endereço; espere um minuto".to_owned(),
                            status => format!("{status}: o código não confere"),
                        }
                    }),
                ),
                act(
                    "Pedidos pendentes",
                    false,
                    Box::new(|it| {
                        let Some(token) = it.token.borrow().clone() else { return "Entre antes".to_owned() };
                        let (status, body) =
                            answer(it, ApiRequest::new("POST", "/api/run/pendentes").bearer(token));
                        format!(
                            "{status}: {} pedidos pendentes, para {}",
                            body["pendentes"],
                            body["quem"].as_str().unwrap_or("?")
                        )
                    }),
                ),
                act(
                    "Pedidos pagos",
                    false,
                    Box::new(|it| {
                        let Some(token) = it.token.borrow().clone() else { return "Entre antes".to_owned() };
                        let request = ApiRequest::new("GET", &format!("/api/data/{ORDER}"))
                            .query("filter", "[paid] = True")
                            .query("count", "true")
                            .bearer(token);
                        let (status, body) = answer(it, request);
                        let customers: Vec<&str> = body["items"]
                            .as_array()
                            .map(|items| items.iter().filter_map(|item| item["customer"].as_str()).collect())
                            .unwrap_or_default();
                        format!("{status}: {} pagos ({})", body["count"], customers.join(", "))
                    }),
                ),
                act(
                    "Ler os eventos pela API",
                    false,
                    Box::new(|it| {
                        let Some(token) = it.token.borrow().clone() else { return "Entre antes".to_owned() };
                        let request =
                            ApiRequest::new("GET", &format!("/api/data/{OUTBOX_EVENT}")).bearer(token);
                        format!("{}: os eventos ficam fora da API", answer(it, request).0)
                    }),
                ),
                act(
                    "Receber pagamento assinado",
                    true,
                    Box::new(|it| {
                        let body = notification();
                        let request = ApiRequest::new("POST", "/api/hooks/pagamentos")
                            .header(ORGANIZATION_HEADER, "matriz")
                            .header(SIGNATURE_HEADER, sign(SECRET.as_bytes(), &body))
                            .bytes(body);
                        let (status, body) = answer(it, request);
                        if body["duplicate"] == Json::Bool(true) {
                            format!("{status}: aviso repetido, nada mudou")
                        } else {
                            format!("{status}: pedido 1 pago, e um evento ficou para enviar")
                        }
                    }),
                ),
                act(
                    "Receber aviso falso",
                    false,
                    Box::new(|it| {
                        let request = ApiRequest::new("POST", "/api/hooks/pagamentos")
                            .header(ORGANIZATION_HEADER, "matriz")
                            .header(SIGNATURE_HEADER, "sha256=0000")
                            .bytes(notification());
                        format!("{}: a assinatura não confere", answer(it, request).0)
                    }),
                ),
                act(
                    "ERP fora do ar",
                    false,
                    Box::new(|it| {
                        let down = !it.destination.down.get();
                        it.destination.down.set(down);
                        if down { "O ERP saiu do ar".to_owned() } else { "O ERP voltou".to_owned() }
                    }),
                ),
                act(
                    "Enviar eventos",
                    true,
                    Box::new(|it| report(dispatcher().run(&mut it.database.space(), &*it.destination))),
                ),
                act(
                    "Enviar como integra",
                    false,
                    Box::new(|it| {
                        let identity = match it.directory.identity_of(WHO) {
                            Ok(Some(identity)) => identity,
                            _ => return "Não há quem envie".to_owned(),
                        };
                        let entities = it.database.space().entities();
                        let context = SecurityContext::new(
                            identity,
                            it.directory.clone(),
                            entities,
                            RoleMerge::AnyRole,
                        );
                        let space = Box::new(it.database.space());
                        report(dispatcher().run_as(&Rc::new(context), space, &*it.destination, local_now()))
                    }),
                ),
                act(
                    "Enviar daqui a um minuto",
                    false,
                    Box::new(|it| {
                        report(dispatcher().run_at(
                            &mut it.database.space(),
                            &*it.destination,
                            a_minute_from_now(),
                        ))
                    }),
                ),
            ];

            let orders = now.database.space().query(ORDER, &Query::all()).unwrap_or_default();
            let paid = orders.iter().filter(|order| order.get("paid") == Value::Bool(true)).count();
            let events = now.database.space().query(OUTBOX_EVENT, &Query::all()).unwrap_or_default();
            let mut shown = vec![
                format!("Pedidos: {} de {} pagos", paid, orders.len()),
                format!(
                    "O ERP recebeu: {}",
                    match now.destination.received.borrow().as_slice() {
                        [] => "nada".to_owned(),
                        keys => keys.join(", "),
                    }
                ),
                format!("O ERP está {}", if now.destination.down.get() { "fora do ar" } else { "no ar" }),
            ];
            for event in events {
                shown.push(format!(
                    "Evento {} versão {}: {}, {} tentativas",
                    event.get("kind").to_text(),
                    event.get("version").to_text(),
                    event.get("state").to_text(),
                    event.get("attempts").to_text()
                ));
            }
            let view = super::super::layout(
                cx,
                "A API web de uma integração, em memória: entrar com um código no lugar da senha, chamar um endpoint próprio, receber um aviso assinado de pagamento e enviar o evento que ele gerou a um ERP que pode cair.",
                buttons,
                shown,
                said.get(),
            );
            cx.child(view.as_ref())
        }
    }
}
