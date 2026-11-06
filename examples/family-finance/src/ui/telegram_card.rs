//! The Telegram card in Ajustes: the bot's token, the account receipts go to, and the QR
//! that pairs one private chat with this database.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ravenrs::prelude::*;

use super::help;
use super::store::{Store, titled_card};
use crate::telegram;

/// Where the QR stands. The link comes from Telegram on another thread, so the card
/// never waits on the network.
#[derive(Clone, Debug, Default)]
enum Pairing {
    #[default]
    Idle,
    Asking,
    Ready(String),
    Failed(String),
}

type Shared = Arc<Mutex<Pairing>>;

fn now() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default()
}

fn set(shared: &Shared, pairing: Pairing) {
    if let Ok(mut slot) = shared.lock() {
        *slot = pairing;
    }
}

enum Visual {
    Qr(String),
    Connected,
}

impl View for Visual {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (node, qr) = match self {
            Self::Qr(link) => (
                Barcode::qr(link.clone())
                    .size(180.0, 180.0)
                    .alt("QR para conectar o Telegram a este Raven")
                    .build(cx),
                true,
            ),
            Self::Connected => {
                (Icon::new(IconName::CircleCheck).size(28.0).color(Semantic::Success).build(cx), false)
            }
        };
        let motion = if qr {
            Motion::new()
                .enter(Effect::hidden(), cx.theme().motion.gentle)
                .exit(Effect::hidden(), cx.theme().motion.fast)
        } else {
            Motion::new()
                .enter(Effect::hidden().with_scale(0.7), cx.theme().motion.snappy)
                .exit(Effect::hidden().with_scale(0.7), cx.theme().motion.fast)
        };
        cx.motion(node, motion);
        node
    }
}

/// The card, with its own state: nothing of it lives in the app.
pub(crate) struct TelegramCard {
    store: Store,
}

impl TelegramCard {
    pub(crate) fn new(store: &Store) -> Self {
        Self { store: store.clone() }
    }

    fn connect(&self, shared: &Shared, waker: UiWaker) {
        let token = telegram::pairing_token();
        let hash = telegram::pairing_hash(&token);
        if self.store.write("Gerando o QR…", |db| db.begin_telegram_pairing(&hash, now())).is_none() {
            return;
        }
        set(shared, Pairing::Asking);
        let shared = shared.clone();
        std::thread::spawn(move || {
            let pairing = match telegram::pairing_link(&token) {
                Ok(link) => Pairing::Ready(link),
                Err(error) => Pairing::Failed(error),
            };
            set(&shared, pairing);
            waker.wake();
        });
    }

    fn token_row(&self, typed: &State<String>, saved: &State<bool>) -> impl View + use<> {
        let (input, store) = (typed.clone(), self.store.clone());
        let (typed, saved) = (typed.clone(), saved.clone());
        HStack::new((
            TextField::new(input.binding())
                .secure()
                .placeholder("Token do bot recebido do BotFather")
                .stretch()
                .grow(1.0),
            Button::new("Salvar token")
                .icon(IconName::Check)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .on_click(move || match telegram::save_bot_token(&typed.get()) {
                    Ok(()) => {
                        typed.set(String::new());
                        saved.set(true);
                    }
                    Err(error) => {
                        store.toasts.show_for("Não foi possível salvar o token", error, Tone::Danger, 8.0)
                    }
                }),
        ))
        .spacing(8.0)
        .align(Align::Center)
    }

    /// Which account the bot's transactions go to, when there is more than one.
    fn account_choice(&self, chosen: &State<u64>) -> Option<impl View + use<>> {
        let path = self.store.path.clone()?;
        let accounts = self.store.read(|db| db.accounts());
        if accounts.len() < 2 {
            return None;
        }
        let ids: Vec<i64> = accounts.iter().map(|account| account.id).collect();
        let current = telegram::chosen_account(&path).and_then(|id| ids.iter().position(|&each| each == id));
        let (bump, read_bump) = (chosen.clone(), chosen.clone());
        let selected = Binding::new(
            move || {
                let _ = read_bump.get();
                current.unwrap_or(0)
            },
            move |index: usize| {
                if let Some(&id) = ids.get(index) {
                    telegram::choose_account(&path, id);
                    bump.set(bump.get().wrapping_add(1));
                }
            },
        );
        let names: Vec<String> = accounts.into_iter().map(|account| account.name).collect();
        let field = Select::new(selected, names).label("Conta dos lançamentos").width(260.0);
        Some(
            VStack::new((
                FormField::new("Conta dos lançamentos", field),
                (current.is_none()).then(|| {
                    Text::new("Escolha a conta: com mais de uma, o bot não adivinha.")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::Warning)
                }),
            ))
            .spacing(6.0),
        )
    }
}

impl View for TelegramCard {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let typed = cx.use_state(String::new);
        let saved = cx.use_state(telegram::token_configured);
        let chosen = cx.use_state(|| 0_u64);
        let shared: Shared = cx.use_state(Shared::default).get();
        let waker = cx.ui_waker();
        let _revision = self.store.version();
        let linked = self.store.read(|db| db.has_telegram_pairing());
        let pairing = shared.lock().map(|slot| slot.clone()).unwrap_or_default();
        let has_token = saved.get();

        let qr = match (&pairing, linked) {
            (Pairing::Ready(link), false) => Some(
                VStack::new((
                    Visual::Qr(link.clone()),
                    Text::new(format!("Com o Telegram neste computador, abra: {link}"))
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(6.0)
                .align(Align::Start),
            ),
            _ => None,
        };
        let failure = match &pairing {
            Pairing::Failed(error) => Some(
                Text::new(format!("Não foi possível gerar o QR: {error}"))
                    .style(TextStyle::Callout)
                    .foreground(Semantic::Danger),
            ),
            _ => None,
        };
        let asking = matches!(pairing, Pairing::Asking);
        let connect = (!linked).then(|| {
            let label = match pairing {
                Pairing::Asking => "Gerando QR…",
                Pairing::Ready(_) | Pairing::Failed(_) => "Gerar novo QR",
                Pairing::Idle => "Conectar Telegram",
            };
            let (card, shared, waker) = (TelegramCard::new(&self.store), shared.clone(), waker.clone());
            Button::new(label)
                .icon(IconName::Link)
                .size(ButtonSize::Small)
                .disabled(!has_token || asking)
                .on_click(move || card.connect(&shared, waker.clone()))
        });
        let unlink = linked.then(|| {
            let (store, shared) = (self.store.clone(), shared.clone());
            Button::new("Desvincular")
                .icon(IconName::X)
                .size(ButtonSize::Small)
                .style(ButtonStyle::Secondary)
                .on_click(move || {
                    if store
                        .write("Telegram desvinculado.", |db| db.revoke_telegram_pairings(now()))
                        .is_some()
                    {
                        set(&shared, Pairing::Idle);
                    }
                })
        });

        let card = titled_card(
            "Telegram",
            help::TELEGRAM,
            VStack::new((
                Text::new(
                    "Envie seus comprovantes em PDF pelo Telegram. Depois, responda se é despesa ou receita e informe a categoria.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                Text::new(
                    "No Telegram, abra @BotFather, envie /newbot e siga as instruções para criar seu bot. Cole aqui o token recebido; ele fica salvo somente neste computador. Depois, conecte e escaneie o QR code.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                self.token_row(&typed, &saved),
                Text::new(if has_token { "Token salvo neste computador." } else { "Informe e salve o token antes de conectar." })
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                self.account_choice(&chosen),
                HStack::new((
                    linked.then_some(Visual::Connected),
                    Text::new(if linked { "Telegram conectado" } else { "Nenhum chat conectado." })
                        .style(TextStyle::Callout)
                        .foreground(if linked { Semantic::Success } else { Semantic::InkMuted }),
                ))
                .spacing(8.0)
                .align(Align::Center),
                qr,
                failure,
                HStack::new((connect, unlink)).spacing(8.0).wrap(),
            ))
            .spacing(10.0)
            .align(Align::Start),
        );
        cx.child(&card)
    }
}
