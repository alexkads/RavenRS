//! The Telegram card in Ajustes: the bot's token, the account receipts go to, the QR that
//! pairs a private chat with this database for one member, and the chats linked.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use ravenrs::prelude::*;
use ravenrs_ai::{HEARING_CATALOG, ModelManager, ModelManagerLabels};

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

/// The QR, fading in when it arrives.
struct Qr(String);

impl View for Qr {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = Barcode::qr(self.0.clone())
            .size(180.0, 180.0)
            .alt("QR para conectar o Telegram a este Raven")
            .build(cx);
        let motion = Motion::new()
            .enter(Effect::hidden(), cx.theme().motion.gentle)
            .exit(Effect::hidden(), cx.theme().motion.fast);
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

    fn connect(&self, shared: &Shared, waker: UiWaker, member: Option<i64>) {
        let token = telegram::pairing_token();
        let hash = telegram::pairing_hash(&token);
        if self.store.write("Gerando o QR…", |db| db.begin_telegram_pairing(&hash, member, now())).is_none()
        {
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

    /// The bot's token: the field while there is none, or while it is being changed;
    /// once saved, only a line that says so and a button to change it.
    fn token_field(
        &self,
        typed: &State<String>,
        saved: &State<bool>,
        editing: &State<bool>,
    ) -> impl View + use<> {
        let content: AnyView = if saved.get() && !editing.get() {
            let editing = editing.clone();
            Box::new(
                HStack::new((
                    Icon::new(IconName::Lock).size(14.0).color(Semantic::InkMuted),
                    Text::new("Salvo somente neste computador.")
                        .style(TextStyle::Callout)
                        .foreground(Semantic::InkMuted),
                    Button::new("Trocar token")
                        .size(ButtonSize::Small)
                        .style(ButtonStyle::Ghost)
                        .on_click(move || editing.set(true)),
                ))
                .spacing(8.0)
                .align(Align::Center),
            )
        } else {
            let (input, store) = (typed.clone(), self.store.clone());
            let (typed, saved, done, cancel) =
                (typed.clone(), saved.clone(), editing.clone(), editing.clone());
            let changing = saved.get();
            Box::new(
                VStack::new((
                    Text::new(
                        "No Telegram, abra @BotFather, envie /newbot e siga as instruções. Cole aqui o token que ele mandar; ele fica salvo somente neste computador.",
                    )
                    .style(TextStyle::Callout)
                    .foreground(Semantic::InkMuted),
                    HStack::new((
                        TextField::new(input.binding())
                            .secure()
                            .placeholder("Cole o token do BotFather")
                            .stretch()
                            .grow(1.0),
                        Button::new("Salvar token").icon(IconName::Check).on_click(move || {
                            match telegram::save_bot_token(&typed.get()) {
                                Ok(()) => {
                                    typed.set(String::new());
                                    saved.set(true);
                                    done.set(false);
                                }
                                Err(error) => store.toasts.show_for(
                                    "Não foi possível salvar o token",
                                    error,
                                    Tone::Danger,
                                    8.0,
                                ),
                            }
                        }),
                        changing.then(|| {
                            Button::new("Cancelar").style(ButtonStyle::Ghost).on_click(move || cancel.set(false))
                        }),
                    ))
                    .spacing(8.0)
                    .align(Align::Center),
                ))
                .spacing(8.0)
                .align(Align::Stretch),
            )
        };
        FormField::new("Token do bot", content)
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
            .spacing(8.0),
        )
    }
}

impl View for TelegramCard {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let typed = cx.use_state(String::new);
        let saved = cx.use_state(telegram::token_configured);
        let chosen = cx.use_state(|| 0_u64);
        let editing = cx.use_state(|| false);
        let path = self.store.path.clone();
        let hearing = cx
            .use_state(move || {
                path.as_deref().map(|path| {
                    ModelManager::new(HEARING_CATALOG, telegram::hearing_folder(path))
                        .labels(ModelManagerLabels::pt_br())
                })
            })
            .get();
        let shared: Shared = cx.use_state(Shared::default).get();
        let waker = cx.ui_waker();
        let _revision = self.store.version();
        let links = self.store.read(|db| db.telegram_links());
        let members = self.store.read(|db| db.members());
        let for_member = cx.use_state(|| 0_usize);
        let pairing = shared.lock().map(|slot| slot.clone()).unwrap_or_default();
        // Once scanned, or expired, a QR is gone: the next one is for another chat.
        let waiting = self.store.read(|db| db.telegram_qr_waiting(now()));
        let pairing = match pairing {
            Pairing::Ready(_) if !waiting => Pairing::Idle,
            pairing => pairing,
        };
        let has_token = saved.get();
        // The member the next QR is for: one of them, or no one in particular, last.
        let member_ids: Vec<Option<i64>> =
            members.iter().map(|member| Some(member.id)).chain(std::iter::once(None)).collect();
        let member = member_ids.get(for_member.get()).copied().flatten();

        let qr = match &pairing {
            Pairing::Ready(link) => Some(
                VStack::new((
                    Qr(link.clone()),
                    Text::new(format!("Com o Telegram neste computador, abra: {link}"))
                        .style(TextStyle::Caption)
                        .foreground(Semantic::InkMuted),
                ))
                .spacing(8.0)
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
        let connect = {
            let label = match pairing {
                Pairing::Asking => "Gerando QR…",
                Pairing::Ready(_) | Pairing::Failed(_) => "Gerar novo QR",
                Pairing::Idle if links.is_empty() => "Conectar Telegram",
                Pairing::Idle => "Conectar outro chat",
            };
            let (card, shared, waker) = (TelegramCard::new(&self.store), shared.clone(), waker.clone());
            Button::new(label)
                .icon(IconName::Link)
                .size(ButtonSize::Small)
                .disabled(!has_token || asking)
                .on_click(move || card.connect(&shared, waker.clone(), member))
        };
        let choose = (!members.is_empty()).then(|| {
            let names: Vec<String> = members
                .iter()
                .map(|member| member.name.clone())
                .chain(std::iter::once("Ninguém em particular".to_owned()))
                .collect();
            Select::new(for_member.binding(), names).label("QR para").width(220.0)
        });
        let status = (!links.is_empty()).then(|| {
            let rows: Vec<AnyView> = links
                .iter()
                .map(|link| {
                    let (store, chat) = (self.store.clone(), link.chat_id);
                    let who =
                        link.member.as_ref().map_or("Ninguém em particular", |member| member.name.as_str());
                    let row: AnyView = Box::new(
                        HStack::new((
                            Icon::new(IconName::Link).size(14.0).color(Semantic::Success),
                            Text::new(format!("{who} · chat {chat}")).style(TextStyle::Body).grow(1.0),
                            Button::new("Desvincular")
                                .size(ButtonSize::Small)
                                .style(ButtonStyle::Ghost)
                                .on_click(move || {
                                    store.write("Chat desvinculado.", |db| {
                                        db.revoke_telegram_chat(chat, now())
                                    });
                                }),
                        ))
                        .spacing(8.0)
                        .align(Align::Center),
                    );
                    row
                })
                .collect();
            VStack::new((
                Alert::new(
                    "Telegram conectado",
                    "Cada chat lança em nome do seu membro, a menos que a mensagem diga outra pessoa.",
                )
                .tone(Tone::Success),
                VStack::new(rows).spacing(4.0).align(Align::Stretch),
            ))
            .spacing(8.0)
            .align(Align::Stretch)
        });
        let pairing_area = VStack::new((
            HStack::new((
                choose,
                connect,
                Text::new(match (has_token, links.is_empty()) {
                    (false, _) => "Salve o token para conectar.",
                    (true, true) => "Nenhum chat conectado.",
                    (true, false) => "Um QR por pessoa, cada um no seu Telegram.",
                })
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
            ))
            .spacing(12.0)
            .align(Align::Center),
            qr,
            failure,
        ))
        .spacing(12.0)
        .align(Align::Start);
        let audio = hearing.map(|models| {
            VStack::new((
                Divider::horizontal(),
                Text::new("Respostas por áudio").style(TextStyle::Headline),
                Text::new(
                    "Para responder ao bot por áudio, baixe um modelo. Ele roda só neste computador: o áudio não vai para nenhum outro lugar.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                models,
            ))
            .spacing(12.0)
            .align(Align::Stretch)
        });

        let card = titled_card(
            "Telegram",
            help::TELEGRAM,
            VStack::new((
                Text::new(
                    "Cada pessoa conecta o próprio Telegram e lança em seu nome, por texto, áudio ou PDF do comprovante.",
                )
                .style(TextStyle::Callout)
                .foreground(Semantic::InkMuted),
                status,
                self.token_field(&typed, &saved, &editing),
                self.account_choice(&chosen),
                pairing_area,
                audio,
            ))
            .spacing(16.0)
            .align(Align::Stretch),
        );
        cx.child(&card)
    }
}
