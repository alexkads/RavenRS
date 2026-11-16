//! PostgreSQL and PostgREST, every feature of `PostgresOptions`, `PostgresBackend`,
//! `PostgrestBuilder` and `ResourceMap`.
//!
//! There is no database here. The addresses come from the build, `RAVEN_POSTGRES_URL` and
//! `RAVEN_POSTGREST_URL`, and never from the page, so no link can send a password somewhere else.
//! With no address, the screen shows what is checked before anything goes on the network, and
//! PostgREST answers from a server in memory that speaks as it does.

use ravenrs::prelude::AnyView;

/// The control's name, as the session knows it.
pub const ID: &str = "PostgresDemo";
/// The page's name, as the navigation shows it.
pub const PAGE: &str = "Postgres e PostgREST";

/// What the page shows.
pub fn page() -> AnyView {
    #[cfg(not(target_arch = "wasm32"))]
    return Box::new(desktop::PostgresPage);
    #[cfg(target_arch = "wasm32")]
    super::desktop_only(
        "Os dois bancos remotos usam drivers do desktop: lá esta tela monta a conexão e lê pela API do PostgREST.",
    )
}

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    use ravenrs::prelude::*;
    use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, ObjectSpace, Query};
    use ravenrs_app_postgres::{PostgresBackend, PostgresOptions, Statements};
    use ravenrs_app_postgrest::{
        Http, HttpError, HttpRequest, HttpResponse, PostgrestBuilder, PostgrestDatabase, ResourceMap,
        SqlWords, TokenSource,
    };

    const PRODUCT: &str = "Product";

    /// The PostgreSQL of the build, if it was given one.
    const POSTGRES_URL: Option<&str> = option_env!("RAVEN_POSTGRES_URL");
    /// The PostgREST of the build, if it was given one.
    const POSTGREST_URL: Option<&str> = option_env!("RAVEN_POSTGREST_URL");

    fn products() -> EntityInfo {
        EntityInfo::new(PRODUCT)
            .caption("Produtos")
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("name", FieldKind::Text).caption("Nome").default_property())
            .field(FieldInfo::new("price", FieldKind::Decimal(2)).caption("Preço"))
    }

    /// How the center connects: statements unprepared, as a pooler in transaction mode needs, and
    /// a short wait for a database that is asleep.
    fn options() -> PostgresOptions {
        PostgresOptions::new().statements(Statements::Unprepared).connect_timeout(Duration::from_secs(5))
    }

    /// A PostgREST in memory: three products, answered a page at a time under the names `select`
    /// gives them (`name:nome`), and a commit that finds the name already taken. It writes down
    /// what it was asked.
    #[derive(Clone, Debug, Default)]
    pub struct Simulated {
        /// What it was asked, in order.
        pub asked: Rc<RefCell<Vec<HttpRequest>>>,
    }

    impl Http for Simulated {
        fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
            self.asked.borrow_mut().push(request.clone());
            if request.method == "POST" {
                let refused = r#"{"code": "23505", "message": "duplicate key", "details": "Key (nome)=(Café) already exists."}"#;
                return Ok(HttpResponse::new(409, refused));
            }
            let given = |name: &str| {
                request
                    .url
                    .split(['?', '&'])
                    .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                    .map(str::to_owned)
            };
            let at = |name: &str| given(name).and_then(|number| number.parse::<usize>().ok()).unwrap_or(0);
            // One by its key, as `id=eq.2`, or all of them.
            let wanted =
                given("id").and_then(|condition| condition.strip_prefix("eq.").and_then(|n| n.parse().ok()));
            let all = [(1, "Café", 18.5), (2, "Açúcar", 6.2), (3, "Leite", 5.9)];
            let page: Vec<String> = all
                .iter()
                .filter(|(id, _, _)| wanted.is_none_or(|wanted: i32| wanted == *id))
                .skip(at("offset"))
                .take(at("limit"))
                .map(|(id, name, price)| format!(r#"{{"id": {id}, "name": "{name}", "price": {price}}}"#))
                .collect();
            Ok(HttpResponse::new(200, format!("[{}]", page.join(","))))
        }
    }

    /// The database through PostgREST, over `http`, with the token `token` gives each time.
    fn postgrest(
        url: &str,
        http: impl Http + 'static,
        token: TokenSource,
    ) -> Result<PostgrestDatabase, String> {
        let builder = PostgrestDatabase::builder(url)
            .schema("loja")
            .api_key("chave-publica-de-exemplo")
            .token_source(token)
            .words(SqlWords::pt_br())
            .page_size(2)
            .http(http);
        let map = ResourceMap::new("produtos").column("name", "nome").column("price", "preco");
        Ok(builder.resource(products(), map).map_err(|e| e.to_string())?.build())
    }

    /// The same, with one token that never changes, as a service account has.
    fn with_fixed_token(url: &str, http: impl Http + 'static) -> Result<PostgrestDatabase, String> {
        let builder: PostgrestBuilder = PostgrestDatabase::builder(url).bearer("token-da-conta-de-servico");
        let map = ResourceMap::new("produtos").column("name", "nome").column("price", "preco");
        Ok(builder.http(http).resource(products(), map).map_err(|e| e.to_string())?.build())
    }

    fn names(database: &PostgrestDatabase) -> Result<Vec<String>, String> {
        let found = database.space().query(PRODUCT, &Query::all()).map_err(|e| e.to_string())?;
        Ok(found.iter().map(|product| product.get("name").to_text()).collect())
    }

    /// What the headers of a request say, without their values.
    fn headers(request: &HttpRequest) -> String {
        let names: Vec<&str> = request.headers.iter().map(|(name, _)| name.as_str()).collect();
        names.join(", ")
    }

    /// What connecting says, with the address never in it.
    fn connected(made: Result<PostgresBackend, ravenrs_app_postgres::ConnectError>) -> String {
        match made {
            Ok(backend) => {
                // Behind a pooler of its own the connection is told to go unprepared anyway.
                let backend = backend.with_statements(Statements::Unprepared);
                format!("Conectado; comandos {:?}", backend.statement_mode())
            }
            Err(why) => format!("Recusado antes da rede: {why}"),
        }
    }

    /// The page: the connection to PostgreSQL and the reading through PostgREST.
    #[derive(Debug, Default)]
    pub struct PostgresPage;

    impl View for PostgresPage {
        fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
            let server = cx.use_state(Simulated::default);
            let renewed = cx.use_state(|| Rc::new(Cell::new(0_u32)));
            let said = cx.use_state(|| "Confira um endereço ou leia pela API.".to_owned());

            let wrong = said.clone();
            let bad_address = Button::new("Conferir um endereço errado").on_click(move || {
                wrong.set(connected(PostgresBackend::connect("isto não é um endereço")));
            });
            let bad = said.clone();
            let bad_certificate = Button::new("Conferir um certificado errado")
                .style(ButtonStyle::Secondary)
                .on_click(move || {
                    let options = options().root_certificate_pem("isto não é um certificado");
                    let address = "postgresql://loja@db.exemplo.com/loja?sslmode=require";
                    bad.set(connected(PostgresBackend::connect_with(address, options)));
                });
            let mut buttons: Vec<AnyView> = vec![Box::new(bad_address), Box::new(bad_certificate)];
            if let Some(address) = POSTGRES_URL {
                let real = said.clone();
                buttons.push(Box::new(Button::new("Conectar ao Postgres").on_click(move || {
                    real.set(connected(PostgresBackend::connect_with(address, options())));
                })));
            }

            let reading = (server.clone(), renewed.clone(), said.clone());
            buttons.push(Box::new(Button::new("Ler pela API").on_click(move || {
                let renewed = reading.1.get();
                // A token renewed by the session is read again at each request.
                let token: TokenSource = Rc::new(move || {
                    renewed.set(renewed.get() + 1);
                    Some(format!("token-{}", renewed.get()))
                });
                let before = reading.0.get().asked.borrow().len();
                let line = postgrest("https://exemplo.supabase.co/rest/v1", reading.0.get(), token)
                    .and_then(|database| names(&database));
                let asked = reading.0.get().asked.borrow().len() - before;
                reading.2.set(match line {
                    Ok(found) => format!("Leu {} em {asked} pedidos de 2: {}", found.len(), found.join(", ")),
                    Err(why) => format!("Não leu: {why}"),
                });
            })));
            let fixed = (server.clone(), said.clone());
            buttons.push(Box::new(Button::new("Ler com token fixo").style(ButtonStyle::Secondary).on_click(
                move || {
                    let line = with_fixed_token("https://exemplo.supabase.co/rest/v1", fixed.0.get())
                        .and_then(|database| names(&database));
                    let last = fixed.0.get().asked.borrow().last().map(headers).unwrap_or_default();
                    fixed.1.set(match line {
                        Ok(found) => format!("Leu {} com o token da conta; cabeçalhos: {last}", found.len()),
                        Err(why) => format!("Não leu: {why}"),
                    });
                },
            )));
            let writing = (server.clone(), said.clone());
            buttons.push(Box::new(
                Button::new("Gravar nome repetido").style(ButtonStyle::Secondary).on_click(move || {
                    let token: TokenSource = Rc::new(|| Some("token-da-sessao".to_owned()));
                    let written = postgrest("https://exemplo.supabase.co/rest/v1", writing.0.get(), token)
                        .and_then(|database| {
                            let mut space = database.space();
                            let mut product = space.new_object(PRODUCT).map_err(|e| e.to_string())?;
                            product.set("id", 4.0);
                            product.set("name", "Café");
                            space.save(product).map_err(|e| e.to_string())?;
                            space.commit().map_err(|e| e.to_string())
                        });
                    writing.1.set(match written {
                        Ok(()) => "Gravou".to_owned(),
                        Err(why) => format!("O servidor recusou: {why}"),
                    });
                }),
            ));
            if let Some(url) = POSTGREST_URL {
                let real = said.clone();
                buttons.push(Box::new(Button::new("Ler do PostgREST").on_click(move || {
                    let token: TokenSource = Rc::new(|| None);
                    let http = ravenrs_app_postgrest::UreqHttp::default();
                    real.set(match postgrest(url, http, token).and_then(|database| names(&database)) {
                        Ok(found) => format!("O servidor tem {} produtos", found.len()),
                        Err(why) => format!("Não leu: {why}"),
                    });
                })));
            }

            let asked = server.get().asked.borrow().clone();
            let mut shown = vec![
                match POSTGRES_URL {
                    Some(_) => "Postgres: o endereço veio da compilação (RAVEN_POSTGRES_URL)".to_owned(),
                    None => "Postgres: sem RAVEN_POSTGRES_URL na compilação, esta tela só confere o endereço e o certificado, que são lidos antes de qualquer rede. Com ele, Conectar abre a conexão com TLS e comandos sem preparo.".to_owned(),
                },
                match POSTGREST_URL {
                    Some(_) => "PostgREST: o endereço veio da compilação (RAVEN_POSTGREST_URL)".to_owned(),
                    None => "PostgREST: sem RAVEN_POSTGREST_URL, um servidor em memória responde como ele.".to_owned(),
                },
                format!("Pedidos ao PostgREST: {}", asked.len()),
            ];
            if let Some(last) = asked.last() {
                shown.push(format!("Último: {} {}", last.method, last.url));
                shown.push(format!(
                    "Esquema: {}",
                    last.header("Accept-Profile").or(last.header("Content-Profile")).unwrap_or("nenhum")
                ));
            }
            let view = super::super::layout(
                cx,
                "A conexão com o PostgreSQL e a leitura pelo PostgREST, com o esquema, a chave pública, o token da sessão e os nomes das colunas.",
                buttons,
                shown,
                said.get(),
            );
            cx.child(view.as_ref())
        }
    }
}
