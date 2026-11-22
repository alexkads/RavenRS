//! The service of the web API, `ravenrs_app_server::ApiService`, with every feature it is set up
//! with, each one asked through `ApiService::handle` as a request from outside would be.
//!
//! The page keeps a service over the center's records in memory, with the same people (`ana`,
//! `bia`, `caio`, password `raven-demo`), and shows the answer to each request: signing in (with
//! the lifetime of a session and the limit of tries), a guest let in by another way of signing in,
//! a setting per person, an endpoint of the center's own, a signed webhook, the description with an
//! entity kept out, and a route of its own that changes while it runs (`fallback`), answered as text
//! (`ApiResponse::raw`). Desktop only: the server does not run in a browser.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use ravenrs::prelude::*;
use ravenrs_app::{
    AuditMode, AuditSettings, AuthError, Authenticator, Credentials, Identity, MemoryDatabase, ObjectSpace,
    PermissionPolicy, Role, RoleMerge, RuleWords, UserDirectory,
};
use ravenrs_app_server::{
    ApiRequest, ApiResponse, ApiService, Failure, ORGANIZATION_HEADER, SIGNATURE_HEADER, SettingAccess,
    Webhook, sign, webhook_event_entity,
};
use serde_json::json;

/// The control registered in the center.
pub const ID: &str = "ApiServiceDemo";
/// The page's navigation caption.
pub const PAGE: &str = "Serviço da API";
/// The name of the service, in its description.
pub const TITLE: &str = "API do Centro de Recursos";
/// How long a session lasts here.
pub const LIFETIME: Duration = Duration::from_secs(30 * 60);
/// How many tries to sign in an address gets a minute here.
pub const TRIES: u32 = 3;
/// The entity the description leaves out.
pub const HIDDEN: &str = "AuditOwner";
/// The guest the other way of signing in lets in, and their invitation.
pub const GUEST: (&str, &str) = ("visitante", "convite-do-centro");
/// The source of the webhook, its organization and its secret.
const HOOK: (&str, &str, &[u8]) = ("pagamentos", "T1", b"segredo-da-loja-do-centro");

/// Lets the people of the center in by their passwords, and a guest by an invitation.
struct Invitation(UserDirectory);

impl Authenticator for Invitation {
    fn authenticate(&self, credentials: &Credentials) -> Result<Identity, AuthError> {
        if (credentials.name.as_str(), credentials.password.as_str()) == GUEST {
            let reader = Role::new("Leitura").policy(PermissionPolicy::ReadOnlyAll);
            return Ok(Identity::new(GUEST.0, GUEST.0, vec![reader]));
        }
        self.0.authenticate(credentials)
    }
}

/// The service of the page: how the roles add up as `merge` says, and the route `/api/hora` on
/// while `clock` is.
pub fn service(merge: RoleMerge, clock: Rc<Cell<bool>>) -> Result<ApiService, String> {
    let mut entities = crate::storage_entities()?;
    entities.push(webhook_event_entity());
    let database = MemoryDatabase::new(entities).map_err(|e| e.0)?;
    let maker: ravenrs_app::SpaceMaker = Rc::new(move || Box::new(database.space()) as Box<dyn ObjectSpace>);
    let roles =
        vec![Role::new("Administradores").administrative(), Role::new("Vendas"), Role::new("Leitura")];
    // The service puts the roles in storage; the people of the center come after them.
    let service = ApiService::new(maker.clone(), roles.clone()).map_err(|e| e.to_string())?;
    let mut records = maker();
    crate::seed::seed(&mut *records)?;
    records.commit().map_err(|e| e.to_string())?;
    let directory = UserDirectory::new(maker, roles).map_err(|e| e.to_string())?;
    // What the provider says comes as `data`: here, that a charge was paid.
    let hook = Webhook::new(|_, data| {
        if data.get("pago").is_none() {
            return Err(Failure::bad_request("o aviso não diz se foi pago"));
        }
        Ok(())
    })
    .secret(HOOK.1, HOOK.2);
    let service = service
        .title(TITLE)
        .rule_words(RuleWords::pt_br())
        .role_merge(merge)
        .audit(AuditSettings::new(AuditMode::Off).entity("AuditFull", AuditMode::Full))
        .authenticator(Rc::new(Invitation(directory)))
        .hide(HIDDEN)
        .session_lifetime(LIFETIME)
        .sign_ins_a_minute(TRIES)
        .setting("tema", SettingAccess::PerPerson)
        .webhook(HOOK.0, hook)
        .endpoint("somar", |context, body| {
            let (a, b) = (body["a"].as_f64().unwrap_or(0.0), body["b"].as_f64().unwrap_or(0.0));
            Ok(json!({ "soma": a + b, "quem": context.name }))
        })
        .fallback(move |request, who| {
            if request.path.join("/") != "api/hora" || !clock.get() {
                return None;
            }
            Some(match who() {
                Some(caller) => {
                    ApiResponse::raw(200, "text/plain", format!("12:00 para {}", caller.name).into_bytes())
                }
                None => ApiResponse::json(401, json!({ "message": "Entre para ver a hora." })),
            })
        });
    Ok(service)
}

/// The body of an answer in one line: its text when it is not JSON.
fn said(answer: &ApiResponse) -> String {
    match &answer.raw {
        Some((kind, bytes)) => format!("{} {kind}: {}", answer.status, String::from_utf8_lossy(bytes)),
        None => format!("{} {}", answer.status, answer.body),
    }
}

/// A request a button makes, and the part of the page and the line its answer goes to.
type Asked = Rc<dyn Fn(&Demo) -> (String, String)>;

/// What the page keeps: the service, the token of whoever signed in, and the last answer of each
/// part.
struct Demo {
    service: RefCell<Result<ApiService, String>>,
    clock: Rc<Cell<bool>>,
    token: RefCell<Option<String>>,
}

impl Demo {
    fn ask(&self, request: ApiRequest) -> ApiResponse {
        let request = match self.token.borrow().clone() {
            Some(token) => request.bearer(token),
            None => request,
        };
        match self.service.borrow_mut().as_mut() {
            Ok(service) => service.handle(request),
            Err(why) => ApiResponse::json(500, json!({ "message": format!("o serviço não abriu: {why}") })),
        }
    }

    fn sign_in(&self, name: &str, password: &str) -> ApiResponse {
        let answer = self.ask(
            ApiRequest::new("POST", "/api/auth/token").json(&json!({ "name": name, "password": password })),
        );
        if answer.status == 200 {
            *self.token.borrow_mut() = answer.body["token"].as_str().map(str::to_owned);
        }
        answer
    }
}

/// Every feature of `ApiService` on one page.
#[derive(Debug, Default)]
pub struct ApiServicePage;

impl View for ApiServicePage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let merge = cx.use_state(|| 0_usize);
        let shown = cx.use_state(Vec::<(String, String)>::new);
        let built_for = cx.use_state(|| usize::MAX);
        let clock_on = cx.use_state(|| false);
        let demo = cx.use_state(|| {
            Rc::new(Demo {
                service: RefCell::new(Err(String::new())),
                clock: Rc::new(Cell::new(false)),
                token: RefCell::new(None),
            })
        });
        let demo_now = demo.get();
        if built_for.get() != merge.get() {
            let chosen = if merge.get() == 1 { RoleMerge::AllRoles } else { RoleMerge::AnyRole };
            *demo_now.service.borrow_mut() = service(chosen, demo_now.clock.clone());
            *demo_now.token.borrow_mut() = None;
            built_for.set(merge.get());
        }
        demo_now.clock.set(clock_on.get());
        let note = {
            let shown = shown.clone();
            move |part: &str, text: String| {
                shown.update(|list| {
                    list.retain(|(p, _)| p != part);
                    list.push((part.to_owned(), text));
                });
            }
        };
        let action = |label: &str, run: Asked| {
            let (demo, note) = (demo.clone(), note.clone());
            Button::new(label).style(ButtonStyle::Secondary).on_click(move || {
                let (part, text) = run(&demo.get());
                note(&part, text);
            })
        };
        let enter = action(
            "Entrar como ana",
            Rc::new(|d: &Demo| {
                let answer = d.sign_in("ana", crate::seed::PASSWORD);
                let said = if answer.status == 200 {
                    format!("a sessão dura {} s", answer.body["expires_in"])
                } else {
                    answer.body["message"].as_str().unwrap_or_default().to_owned()
                };
                ("Entrar".to_owned(), format!("{}: {said}", answer.status))
            }),
        );
        let guest = action(
            "Entrar com convite",
            Rc::new(|d: &Demo| {
                let answer = d.sign_in(GUEST.0, GUEST.1);
                ("Convite".to_owned(), format!("{}: {} entrou por convite", answer.status, GUEST.0))
            }),
        );
        let wrong = action(
            "Errar a senha 4 vezes",
            Rc::new(|d: &Demo| {
                let statuses: Vec<String> = (0..=TRIES)
                    .map(|_| d.sign_in("bia", "senha-errada-de-proposito").status.to_string())
                    .collect();
                ("Tentativas".to_owned(), format!("respostas: {}", statuses.join(", ")))
            }),
        );
        let theme = action(
            "Guardar o tema escuro",
            Rc::new(|d: &Demo| {
                d.ask(ApiRequest::new("PUT", "/api/settings/tema").json(&json!({ "value": "escuro" })));
                let answer = d.ask(ApiRequest::new("GET", "/api/settings/tema"));
                ("Tema".to_owned(), format!("{}: tema {}", answer.status, answer.body["value"]))
            }),
        );
        let add = action(
            "Somar 2 e 3",
            Rc::new(|d: &Demo| {
                let answer =
                    d.ask(ApiRequest::new("POST", "/api/run/somar").json(&json!({ "a": 2, "b": 3 })));
                ("Somar".to_owned(), format!("{}: soma {}", answer.status, answer.body["soma"]))
            }),
        );
        let hook = action(
            "Receber aviso assinado",
            Rc::new(|d: &Demo| {
                let body = json!({ "id": "evt_1", "subject": "cobranca_9", "at": 1_731_600_000_000_i64, "data": { "pago": true } });
                let bytes = body.to_string().into_bytes();
                let signed = ApiRequest::new("POST", &format!("/api/hooks/{}", HOOK.0))
                    .header(ORGANIZATION_HEADER, HOOK.1)
                    .header(SIGNATURE_HEADER, sign(HOOK.2, &bytes))
                    .bytes(bytes.clone());
                let good = d.ask(signed).status;
                let forged = ApiRequest::new("POST", &format!("/api/hooks/{}", HOOK.0))
                    .header(ORGANIZATION_HEADER, HOOK.1)
                    .header(SIGNATURE_HEADER, "sha256=00")
                    .bytes(bytes);
                let bad = d.ask(forged).status;
                ("Aviso".to_owned(), format!("assinado: {good}; com assinatura falsa: {bad}"))
            }),
        );
        let describe = action(
            "Ler a descrição",
            Rc::new(|d: &Demo| {
                let answer = d.ask(ApiRequest::new("GET", "/api/openapi.json"));
                let text = answer.body.to_string();
                let title = answer.body["info"]["title"].as_str().unwrap_or("—").to_owned();
                let kept_out = if text.contains(&format!("/api/data/{HIDDEN}")) { "não" } else { "sim" };
                (
                    "Descrição".to_owned(),
                    format!("{}: {title}; {HIDDEN} fica de fora: {kept_out}", answer.status),
                )
            }),
        );
        let clock = action(
            "Pedir /api/hora",
            Rc::new(|d: &Demo| ("Hora".to_owned(), said(&d.ask(ApiRequest::new("GET", "/api/hora"))))),
        );
        let lines = shown.clone();
        let merge_words = merge.clone();
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "O serviço que responde a API web: cada botão manda um pedido, como viria de fora, e a \
                     resposta aparece embaixo.",
                )
                .foreground(Semantic::InkMuted),
                VStack::new((
                    Text::new("Entrar").style(TextStyle::Headline),
                    Text::new(format!(
                        "A sessão dura {} minutos; {TRIES} tentativas por minuto de cada endereço; além das senhas, \
                         um convite deixa o visitante entrar.",
                        LIFETIME.as_secs() / 60
                    ))
                    .foreground(Semantic::InkMuted),
                    HStack::new((enter, guest, wrong)).spacing(8.0).wrap(),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("O que a aplicação acrescenta").style(TextStyle::Headline),
                    Text::new(
                        "Uma preferência por pessoa, um comando do próprio centro e os avisos de um provedor, \
                         assinados por organização.",
                    )
                    .foreground(Semantic::InkMuted),
                    HStack::new((theme, add, hook)).spacing(8.0).wrap(),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Descrição, papéis e regras").style(TextStyle::Headline),
                    Text::new(format!(
                        "A descrição se chama {TITLE} e deixa {HIDDEN} de fora; as regras falam português; a \
                         auditoria guarda o que muda em AuditFull."
                    ))
                    .foreground(Semantic::InkMuted),
                    SegmentedControl::new(merge.binding(), ["Um papel basta", "Todos os papéis"]).label("Como os papéis somam"),
                    Text::dynamic(move || {
                        if merge_words.get() == 1 {
                            "Papéis: todos precisam permitir.".to_owned()
                        } else {
                            "Papéis: um que permita basta.".to_owned()
                        }
                    }),
                    describe,
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Uma rota que muda enquanto roda").style(TextStyle::Headline),
                    Text::new(
                        "Ligada, /api/hora responde em texto puro antes das rotas da API; desligada, o pedido segue \
                         e ninguém o conhece.",
                    )
                    .foreground(Semantic::InkMuted),
                    Toggle::new(clock_on.binding()).label("Ligar a rota /api/hora"),
                    clock,
                ))
                .spacing(8.0)
                .align(Align::Start),
                Text::new("Respostas").style(TextStyle::Headline),
                Text::dynamic(move || {
                    let list = lines.get();
                    if list.is_empty() {
                        "Nenhum pedido ainda.".to_owned()
                    } else {
                        list.iter().map(|(part, text)| format!("{part}: {text}")).collect::<Vec<_>>().join("\n")
                    }
                }),
            ))
            .spacing(16.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
