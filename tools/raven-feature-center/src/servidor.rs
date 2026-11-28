//! The web API of the center, as `ravenrs_app_server::ApiOptions` sets it up: which pages may call
//! it from a browser, how many requests it answers at the same time, and the live channel it streams
//! events on.
//!
//! The page builds the options the way `src/bin/api.rs` does and shows what the server gets, so the
//! tour can check each choice. Desktop only: the server does not run in a browser.

use ravenrs::prelude::*;
use ravenrs_app_server::{ApiOptions, LiveChannels};

/// The control registered in the center.
pub const ID: &str = "ApiOptionsDemo";
/// The page's navigation caption.
pub const PAGE: &str = "API web";
/// How many threads answer the center's API when it keeps its records in a file.
pub const WORKERS_WITH_A_FILE: usize = 4;
/// The topic of the live channel the center opens.
pub const TOPIC: &str = "avisos";

/// The center's live channel, with its one topic open.
pub fn channels() -> LiveChannels {
    let channels = LiveChannels::new();
    channels.open(TOPIC);
    channels
}

/// The prefix of the paths that come in with no token, for the webhooks of other services.
pub const OPEN_PREFIX: &str = "/hooks/";
/// The headers of those webhooks the service is handed besides its own.
pub const PASSED_HEADER: &str = "x-hub-";

/// The options as `src/bin/api.rs` builds them, from what the page has; `hooks` opens
/// [`OPEN_PREFIX`] with no token and hands over the headers starting with [`PASSED_HEADER`].
pub fn options(origins: &[String], workers: i32, live: Option<&LiveChannels>, hooks: bool) -> ApiOptions {
    let threads = usize::try_from(workers).unwrap_or(0);
    let options = origins
        .iter()
        .fold(ApiOptions::new().workers(threads), |options, origin| options.allow_origin(origin.clone()));
    let options = match live {
        Some(channels) => options.live(channels.clone()),
        None => options,
    };
    if hooks { options.open(OPEN_PREFIX).pass_header(PASSED_HEADER) } else { options }
}

/// Every feature of `ApiOptions` on one page.
#[derive(Debug, Default)]
pub struct ApiOptionsPage;

impl View for ApiOptionsPage {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let origin = cx.use_state(String::new);
        let origins = cx.use_state(Vec::<String>::new);
        let workers = cx.use_state(|| 1_i32);
        let live = cx.use_state(|| false);
        let hooks = cx.use_state(|| false);
        let shown_hooks = hooks.clone();
        let published = cx.use_state(|| 0_u64);
        let channels = cx.use_state(channels);
        let (sending, counted) = (channels.clone(), published.clone());
        let publish = move || {
            let next = counted.get() + 1;
            if let Some(id) = sending.get().publish(TOPIC, "aviso", serde_json::json!({ "numero": next })) {
                counted.set(id);
            }
        };
        let shown_published = published.clone();
        let (shown_live, shown_channels) = (live.clone(), channels.clone());
        let (typed, kept) = (origin.clone(), origins.clone());
        let allow = move || {
            let text = typed.get().trim().to_owned();
            if !text.is_empty() && !kept.get().contains(&text) {
                kept.update(|list| list.push(text));
                typed.set(String::new());
            }
        };
        let (listed, threads) = (origins.clone(), workers.clone());
        let (shown_origins, shown_workers) = (origins.clone(), workers.clone());
        cx.child(
            &VStack::new((
                Text::new(PAGE).style(TextStyle::Title),
                Text::new(
                    "O que o servidor da API recebe ao abrir: quem pode chamá-lo de um navegador e quantos \
                     pedidos ele atende ao mesmo tempo.",
                )
                .foreground(Semantic::InkMuted),
                VStack::new((
                Text::new("Quem pode chamar de um navegador").style(TextStyle::Headline),
                Text::new("Sem nenhuma, só o próprio endereço chama. Cada origem é dita pelo nome: não há um 'qualquer'.")
                    .foreground(Semantic::InkMuted),
                HStack::new((
                    TextField::new(origin.binding()).placeholder("Origem, como https://exemplo.com").width(320.0),
                    Button::new("Permitir origem").style(ButtonStyle::Secondary).on_click(allow),
                ))
                .spacing(8.0),
                Text::dynamic(move || match listed.get().as_slice() {
                    [] => "Nenhuma origem permitida.".to_owned(),
                    list => format!("Permitidas: {}.", list.join(", ")),
                }),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                Text::new("Pedidos ao mesmo tempo").style(TextStyle::Headline),
                Text::new(
                    "Cada thread tem o seu serviço e todas veem as mesmas sessões. Acima de uma, os registros \
                     precisam estar num arquivo ou num banco servidor; em memória, a API fica com uma.",
                )
                .foreground(Semantic::InkMuted),
                Stepper::new(workers.binding()).range(0, 8).label("Threads"),
                Text::dynamic(move || {
                    let asked = threads.get();
                    if asked < 1 {
                        format!("Pedido: {asked}. Conta como 1: sempre há uma thread.")
                    } else {
                        format!("Pedido: {asked}.")
                    }
                }),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Canal ao vivo").style(TextStyle::Headline),
                    Text::new(
                        "O servidor transmite os eventos de cada tópico em /api/live/{tópico}, para quem pode ler                          /api/me. Quem volta depois de cair recebe o que perdeu, a partir do último que ouviu.",
                    )
                    .foreground(Semantic::InkMuted),
                    Toggle::new(live.binding()).label("Ligar o canal ao vivo"),
                    Button::new("Publicar evento").style(ButtonStyle::Secondary).on_click(publish),
                    Text::dynamic(move || match shown_published.get() {
                        0 => format!("Nenhum evento publicado em {TOPIC}."),
                        id => format!("Último evento publicado em {TOPIC}: #{id}."),
                    }),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                    Text::new("Avisos de outros serviços").style(TextStyle::Headline),
                    Text::new(format!(
                        "Um provedor que avisa (um webhook) não tem token: {OPEN_PREFIX} entra sem ele, e quem \
                         responde confere a assinatura. Os cabeçalhos {PASSED_HEADER}* chegam ao serviço, que \
                         assina com eles."
                    ))
                    .foreground(Semantic::InkMuted),
                    Toggle::new(hooks.binding()).label("Receber avisos de outros serviços"),
                ))
                .spacing(8.0)
                .align(Align::Start),
                VStack::new((
                Text::new("O que o servidor recebe").style(TextStyle::Headline),
                Text::dynamic(move || {
                    let channels = shown_channels.get();
                    let live = shown_live.get().then_some(&channels);
                    format!("{:?}", options(&shown_origins.get(), shown_workers.get(), live, shown_hooks.get()))
                }),
                Text::new("Como abrir").style(TextStyle::Headline),
                Text::new(format!(
                    "cargo run --manifest-path tools/raven-feature-center/Cargo.toml --bin api -- centro.db abre a API sobre um arquivo, com \
                     {WORKERS_WITH_A_FILE} threads. Sem o arquivo, ela guarda tudo em memória e fica com uma."
                )),
                ))
                .spacing(8.0)
                .align(Align::Start),
            ))
            .spacing(12.0)
            .padding(16.0)
            .max_width(900.0)
            .align(Align::Start),
        )
    }
}
