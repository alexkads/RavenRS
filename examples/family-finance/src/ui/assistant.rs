//! The assistant in the corner of every screen. A local model, when one is in use,
//! only chooses what to do from the person's words: the numbers come from the
//! database, by this code, so it cannot make them up, and nothing is saved until the
//! person confirms it on the form this opens.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use ravenrs::movie::{Moment, MovieAnswers, MovieChatLabels, MovieWriter};
use ravenrs::prelude::*;
use ravenrs_ai::{LocalAssistant, ModelManager, Tool, ToolCall, VoiceManager, read_tool_call, tool_rules};

use super::forms::Forms;
use super::request::{Prefill, When, folded, read_request};
use super::store::Store;
use super::training::{TRAINING_PAGE, TrainingState};
use crate::model::{Category, Flow, Recurrence, date_label};
use crate::money::format_brl;
use crate::ui::spoken::spoken;

/// What tells the model it is choosing.
pub const ASSISTANT_INTRO: &str =
    "Você escolhe a ferramenta certa para o pedido da pessoa no app Raven Finanças.";
/// What the model can choose from. It chooses; this file does the work.
pub const ASSISTANT_TOOLS: &[Tool] = &[
    Tool { name: "saldo_das_contas", description: "quanto dinheiro há em cada conta. Sem argumentos." },
    Tool {
        name: "gastos_por_categoria",
        description: "pergunta sobre quanto já foi gasto no mês. Argumento opcional: categoria.",
    },
    Tool { name: "contas_a_pagar", description: "contas vencidas e as próximas a pagar. Sem argumentos." },
    Tool {
        name: "abrir_tela",
        description: "a pessoa quer ver ou ir para uma tela. Argumento: tela (painel, lancamentos, contas, agenda, orcamentos, relatorios, cadastros, ajustes, treinamentos).",
    },
    Tool {
        name: "lancar_despesa",
        description: "registrar um gasto já feito, como \"gastei na padaria\" ou \"criar uma despesa de lanche\". Argumentos: descricao (só o que foi comprado), categoria.",
    },
    Tool {
        name: "criar_conta",
        description: "criar ou agendar algo para pagar em uma data, como \"agendar a luz\" ou \"criar um pagamento\". Argumentos: descricao (só o nome da conta), categoria.",
    },
    Tool {
        name: "pagar_conta",
        description: "a pessoa diz que já pagou uma conta que existe, como \"paguei o streaming\". Argumento: conta.",
    },
    Tool {
        name: "ajuda",
        description: "como fazer alguma coisa no app, como \"como faço um backup?\". Argumento: pergunta.",
    },
    Tool {
        name: "nenhuma",
        description: "o pedido não tem relação com finanças nem com o app. Sem argumentos.",
    },
];
/// How the model is told to write about the films.
const ANSWER_RULES: &str = "Você é o assistente do app Raven Finanças. Responda em português do Brasil, em até duas frases, usando só os trechos do manual dados. Se os trechos não responderem, diga que não sabe.";
/// The screens the model can open, in the app's order of pages.
const SCREENS: [(&str, &str); 9] = [
    ("painel", "Painel"),
    ("lancamentos", "Lançamentos"),
    ("contas", "Contas a pagar"),
    ("agenda", "Agenda"),
    ("orcamentos", "Orçamentos"),
    ("relatorios", "Relatórios"),
    ("cadastros", "Cadastros"),
    ("ajustes", "Ajustes"),
    ("treinamentos", "Treinamentos"),
];
/// A message up to this long may depend on the one before it.
const FOLLOW_UP_WORDS: usize = 5;
/// How many rows an answer lists.
const ROWS: usize = 6;

/// Said once, when a model took too long: from then on it answers without one.
const TOO_SLOW: &str = "Esta IA demorou demais neste computador, então continuo respondendo sem ela. Em Ajustes, você pode escolher uma IA menor.";

/// A question the model is thinking about.
pub(crate) struct Pending {
    question: String,
    choice: Arc<Mutex<String>>,
    done: Arc<AtomicBool>,
    waker: UiWaker,
}

/// The description a model read from the request, unless it is only a number.
fn description_in(call: &ToolCall) -> Option<&str> {
    call.argument("descricao").filter(|text| text.chars().any(char::is_alphabetic))
}

/// Has the model in use, if any, write the answer about the films. Only the question
/// and the film's own text reach it.
fn writer(models: Option<&ModelManager>) -> Option<MovieWriter> {
    let assistant = models?.assistant()?;
    Some(Rc::new(move |question, moments, text, waker, if_it_fails| {
        let passages: String =
            moments.iter().map(|moment| format!("{}: {}\n", moment.title, moment.text)).collect();
        let user = format!("Trechos:\n{passages}\nPergunta: {question}");
        assistant.write(ANSWER_RULES.to_owned(), user, text, waker, if_it_fails);
    }))
}

/// An answer with a button that does something once: when `act` tells it that it is done,
/// the button gives way to `done`, so the same thing cannot be saved again by accident.
fn act_once(
    text: String,
    label: &'static str,
    done: &'static str,
    style: ButtonStyle,
    act: impl Fn(Rc<dyn Fn()>) + 'static,
) -> ChatMessage {
    let (finished, act) = (State::new(false), Rc::new(act));
    ChatMessage::assistant(text).card(move || -> AnyView {
        if finished.get() {
            return Box::new(HStack::new((Badge::new(done).tone(Tone::Success).icon(IconName::Check),)));
        }
        let (finished, act) = (finished.clone(), act.clone());
        Box::new(Button::new(label).style(style).size(ButtonSize::Small).on_click(move || {
            let finished = finished.clone();
            act(Rc::new(move || finished.set(true)));
        }))
    })
}

/// What was understood, in words, so the person sees it before opening the form.
fn summary(fill: &Prefill) -> String {
    let mut parts = vec![fill.description.clone()];
    if let Some(cents) = fill.cents {
        match fill.installments {
            Some(count) => parts.push(format!("{} em {count}x", format_brl(cents))),
            None => parts.push(format_brl(cents)),
        }
    }
    if let Some(date) = fill.date {
        parts.push(date_label(date));
    }
    if let Some(category) = &fill.category {
        parts.push(category.name.clone());
    }
    if let Some(member) = &fill.member {
        parts.push(member.name.clone());
    }
    if let Some(account) = &fill.account {
        parts.push(account.name.clone());
    }
    match fill.recurrence {
        Some(Recurrence::Monthly) => parts.push("todo mês".to_owned()),
        Some(Recurrence::Yearly) => parts.push("todo ano".to_owned()),
        _ => {}
    }
    if fill.barcode.is_some() {
        parts.push("com código de barras".to_owned());
    }
    parts.join(", ")
}

/// What answering takes.
#[derive(Clone)]
struct Replies {
    store: Store,
    forms: Forms,
    page: State<usize>,
    open: State<bool>,
    messages: State<Vec<ChatMessage>>,
    pending: Rc<RefCell<Option<Pending>>>,
    films: Option<MovieAnswers>,
    models: Option<ModelManager>,
    voices: Option<VoiceManager>,
}

impl Replies {
    /// Takes a question from the chat.
    fn send(&self, question: String, waker: UiWaker) {
        let assistant = self.assistant().filter(|assistant| !assistant.too_slow());
        let Some(assistant) = assistant else {
            let reply = self.about_films(&question, &waker);
            self.read_aloud(&reply.text(), &waker);
            self.messages.update(|all| all.push(reply));
            return;
        };
        // On its way: the model is choosing, away from the UI thread.
        let waiting = ChatMessage::assistant_writing(Arc::new(Mutex::new(String::new())));
        self.messages.update(|all| all.push(waiting));
        let (choice, done) = (Arc::new(Mutex::new(String::new())), Arc::new(AtomicBool::new(false)));
        let categories = self.store.read(|db| db.categories());
        let names: Vec<&str> = categories
            .iter()
            .filter(|category| category.flow == Flow::Expense)
            .map(|category| category.name.as_str())
            .collect();
        let rules = format!(
            "{}Categorias de gasto: {}.",
            tool_rules(ASSISTANT_INTRO, ASSISTANT_TOOLS),
            names.join(", ")
        );
        let question = self.with_context(&question);
        assistant.answer(rules, question.clone(), choice.clone(), done.clone(), waker.clone());
        *self.pending.borrow_mut() = Some(Pending { question, choice, done, waker });
    }

    /// A short message goes on from the one before it, as in "but I want to create it".
    fn with_context(&self, question: &str) -> String {
        if question.split_whitespace().count() > FOLLOW_UP_WORDS {
            return question.to_owned();
        }
        let before = self.messages.with(|all| {
            all.iter()
                .rev()
                .filter(|message| message.author() == ChatAuthor::Person)
                .nth(1)
                .map(ChatMessage::text)
        });
        before.map_or_else(|| question.to_owned(), |before| format!("{before}. {question}"))
    }

    /// Takes in what the model chose, if it has finished.
    fn poll(&self) {
        let finished = self.pending.borrow().as_ref().is_some_and(|p| p.done.load(Ordering::Acquire));
        if !finished {
            return;
        }
        let Some(pending) = self.pending.borrow_mut().take() else { return };
        let chosen = pending.choice.lock().map(|text| text.clone()).unwrap_or_default();
        let reply =
            self.reply(read_tool_call(&chosen, ASSISTANT_TOOLS).as_ref(), &pending.question, &pending.waker);
        self.read_aloud(&reply.text(), &pending.waker);
        let too_slow = self.assistant().is_some_and(|assistant| assistant.too_slow());
        self.messages.update(|all| {
            all.pop();
            all.push(reply);
            if too_slow {
                all.push(ChatMessage::assistant(TOO_SLOW));
            }
        });
    }

    /// Reads an answer when it arrives, if the person asked for that.
    fn read_aloud(&self, text: &str, waker: &UiWaker) {
        if let Some(voices) = self.voices.as_ref().filter(|voices| voices.reads_aloud())
            && !text.is_empty()
        {
            voices.say(spoken(text), waker.clone());
        }
    }

    fn assistant(&self) -> Option<LocalAssistant> {
        self.models.as_ref().and_then(ModelManager::assistant)
    }

    /// Does what was chosen. Anything it cannot do goes to the films, as with no model.
    fn reply(&self, call: Option<&ToolCall>, question: &str, waker: &UiWaker) -> ChatMessage {
        let done = call.and_then(|call| match call.name.as_str() {
            "saldo_das_contas" => Some(self.balance()),
            "gastos_por_categoria" => Some(self.spending(call.argument("categoria"))),
            "contas_a_pagar" => Some(self.bills()),
            "abrir_tela" => self.open_screen(call.argument("tela")?),
            // Amounts, dates and names come from the person's words: see `request`.
            "lancar_despesa" => Some(self.expense(call, question)),
            "criar_conta" => Some(self.new_bill(call, question)),
            "pagar_conta" => Some(self.pay(call.argument("conta").unwrap_or(question), question)),
            _ => None,
        });
        done.unwrap_or_else(|| self.about_films(question, waker))
    }

    fn about_films(&self, question: &str, waker: &UiWaker) -> ChatMessage {
        self.films.as_ref().map_or_else(
            || ChatMessage::assistant(MovieChatLabels::pt_br().nothing_found),
            |films| films.answer(question, waker),
        )
    }

    fn balance(&self) -> ChatMessage {
        let accounts = self.store.read(|db| db.accounts());
        let total: i64 = accounts.iter().map(|account| account.balance_cents).sum();
        let text = format!("Você tem {} nas contas:", format_brl(total));
        ChatMessage::assistant(text).card(move || {
            let rows: Vec<ListItem> = accounts
                .iter()
                .map(|account| {
                    ListItem::new(account.name.clone()).trailing(format_brl(account.balance_cents))
                })
                .collect();
            Box::new(List::new(rows))
        })
    }

    fn spending(&self, category: Option<&str>) -> ChatMessage {
        let today = self.store.today;
        let mut lines = self.store.read(|db| db.budget_lines(today));
        lines.retain(|line| line.spent_cents > 0);
        lines.sort_by_key(|line| std::cmp::Reverse(line.spent_cents));
        // A model may name something that is no category: then it is about everything.
        let wanted = category.map(folded).filter(|wanted| !wanted.is_empty());
        let one =
            wanted.and_then(|wanted| lines.iter().find(|line| folded(&line.category).contains(&wanted)));
        if let Some(line) = one {
            return ChatMessage::assistant(format!(
                "Você gastou {} com {} neste mês.",
                format_brl(line.spent_cents),
                line.category
            ));
        }
        let total: i64 = lines.iter().map(|line| line.spent_cents).sum();
        ChatMessage::assistant(format!("Você gastou {} neste mês. Onde mais pesou:", format_brl(total))).card(
            move || {
                let rows: Vec<ListItem> = lines
                    .iter()
                    .take(ROWS)
                    .map(|line| ListItem::new(line.category.clone()).trailing(format_brl(line.spent_cents)))
                    .collect();
                Box::new(List::new(rows))
            },
        )
    }

    fn bills(&self) -> ChatMessage {
        let today = self.store.today;
        let bills = self.store.read(|db| db.open_bills());
        if bills.is_empty() {
            return ChatMessage::assistant("Nenhuma conta em aberto.");
        }
        let late = bills.iter().filter(|bill| bill.due < today).count();
        let text = format!("{} em aberto, {late} vencidas. Clique em uma para pagar:", bills.len());
        let (store, form) = (self.store.clone(), self.forms.pay.clone());
        ChatMessage::assistant(text).card(move || {
            let rows: Vec<ListItem> = bills
                .iter()
                .take(ROWS)
                .map(|bill| {
                    let (store, form, id) = (store.clone(), form.clone(), bill.id);
                    ListItem::new(bill.description.clone())
                        .subtitle(date_label(bill.due))
                        .trailing(format_brl(bill.amount_cents))
                        .on_click(move || form.start(&store, id))
                })
                .collect();
            Box::new(List::new(rows))
        })
    }

    fn open_screen(&self, screen: &str) -> Option<ChatMessage> {
        let wanted = folded(screen);
        let index = SCREENS.iter().position(|(name, _)| wanted.contains(name))?;
        self.page.set(index);
        // The screen is what was asked for, so the panel gets out of the way.
        self.open.set(false);
        Some(ChatMessage::assistant(format!("Abri {}.", SCREENS[index].1)))
    }

    /// What a request says, with the description the model read and a category only
    /// if it exists.
    fn prefill(&self, call: Option<&ToolCall>, question: &str, when: When) -> Prefill {
        let (categories, members, accounts) =
            self.store.read(|db| Ok((db.categories()?, db.members()?, db.accounts()?)));
        let expenses: Vec<Category> =
            categories.into_iter().filter(|category| category.flow == Flow::Expense).collect();
        let suggested = call.and_then(|call| call.argument("categoria"));
        let today = self.store.today;
        let mut fill = read_request(question, today, when, &expenses, &members, &accounts, suggested);
        fill.description = call.and_then(description_in).unwrap_or(question).to_owned();
        fill
    }

    /// Prepares a spending. Only the form's own button saves it.
    fn expense(&self, call: &ToolCall, question: &str) -> ChatMessage {
        let fill = self.prefill(Some(call), question, When::Behind);
        let text = format!("Preparei a despesa {}. Confira e salve.", summary(&fill));
        let (store, form) = (self.store.clone(), self.forms.transaction.clone());
        act_once(text, "Conferir e salvar", "Salvo", ButtonStyle::Primary, move |done| {
            form.start_with(&store, &fill, done);
        })
    }

    fn new_bill(&self, call: &ToolCall, question: &str) -> ChatMessage {
        let fill = self.prefill(Some(call), question, When::Ahead);
        let text = format!("Preparei a conta {}. Confira e salve.", summary(&fill));
        let (store, form) = (self.store.clone(), self.forms.bill.clone());
        act_once(text, "Conferir e salvar", "Salvo", ButtonStyle::Primary, move |done| {
            form.start_with(&store, &fill, done);
        })
    }

    fn pay(&self, name: &str, question: &str) -> ChatMessage {
        let wanted = folded(name);
        let bills = self.store.read(|db| db.open_bills());
        let found = bills.iter().find(|bill| {
            let description = folded(&bill.description);
            description.contains(&wanted) || wanted.contains(&description)
        });
        let Some(bill) = found else {
            // "Paguei 80 de gasolina" is a spending, when no such bill exists.
            let fill = self.prefill(None, question, When::Behind);
            if fill.cents.is_some() {
                let call = ToolCall {
                    name: "lancar_despesa".to_owned(),
                    arguments: HashMap::from([("descricao".to_owned(), name.to_owned())]),
                };
                return self.expense(&call, question);
            }
            // It may be that the person wants one: offer it rather than a dead end.
            let (store, form, name) = (self.store.clone(), self.forms.bill.clone(), name.to_owned());
            let fill = Prefill { description: name, ..Prefill::default() };
            let text = format!("Não achei uma conta em aberto chamada {}.", fill.description);
            return act_once(text, "Criar esta conta", "Criada", ButtonStyle::Secondary, move |done| {
                form.start_with(&store, &fill, done);
            });
        };
        let text =
            format!("Achei {}, de {}. Confira e pague:", bill.description, format_brl(bill.amount_cents));
        let (store, form, id) = (self.store.clone(), self.forms.pay.clone(), bill.id);
        act_once(text, "Conferir e pagar", "Paga", ButtonStyle::Primary, move |done| {
            form.start_then(&store, id, done);
        })
    }
}

/// The conversation, which needs a waker from the UI it is built in.
struct AssistantChat {
    replies: Replies,
}

impl View for AssistantChat {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let (replies, waker) = (self.replies.clone(), cx.ui_waker());
        if let Some(voices) = &self.replies.voices {
            voices.play_ready(&cx.sound());
        }
        let thinking = self.replies.models.as_ref().and_then(ModelManager::assistant).is_some();
        let (intro, examples): (&str, &[&str]) = if thinking {
            (
                "Pergunte sobre seu dinheiro, peça para abrir uma tela, conte um gasto ou agende uma conta. Nada é gravado sem você conferir.",
                &[
                    "Qual é o meu saldo?",
                    "Onde estou gastando mais?",
                    "Tenho conta vencida?",
                    "Gastei 50 reais na padaria",
                ],
            )
        } else {
            (
                "Pergunte como fazer alguma coisa, e eu abro o momento do filme de treinamento que mostra.",
                &["Como lançar uma despesa?", "Como pagar uma conta?", "Como fazer um backup?"],
            )
        };
        let chat = Chat::new(self.replies.messages.binding())
            .placeholder("Pergunte alguma coisa")
            .intro(intro)
            .examples(examples.iter().copied())
            .busy(self.replies.pending.borrow().is_some())
            .on_send(move |question| replies.send(question, waker.clone()));
        let chat = match self.replies.voices.clone().filter(|voices| voices.voice().is_some()) {
            Some(voices) => {
                let waker = cx.ui_waker();
                chat.on_listen(move |text| voices.say(spoken(&text), waker.clone()))
            }
            None => chat,
        };
        chat.build(cx)
    }
}

/// The sphere and its panel, and takes in what the model has finished.
pub(crate) fn chat(
    state: &TrainingState,
    store: &Store,
    forms: &Forms,
    page: &State<usize>,
) -> Option<FloatingPanel> {
    let library = state.library.clone()?;
    let (cue, open) = (state.cue.clone(), state.chat_open.clone());
    let (going, closing, models) = (page.clone(), open.clone(), state.models.clone());
    let mut labels = MovieChatLabels::pt_br();
    if models.as_ref().and_then(ModelManager::assistant).is_some() {
        labels.nothing_found = "Não entendi. Posso responder sobre saldo, gastos e contas, abrir telas, lançar despesas e criar contas a pagar.".to_owned();
    }
    let films = MovieAnswers {
        library,
        labels,
        on_open: Rc::new(move |moment: &Moment| {
            going.set(TRAINING_PAGE);
            cue.go_to_film(moment.film, moment.at_ms);
            closing.set(false);
        }),
        writer: writer(models.as_ref()),
    };
    let replies = Replies {
        store: store.clone(),
        forms: forms.clone(),
        page: page.clone(),
        open: open.clone(),
        messages: state.messages.clone(),
        pending: state.pending.clone(),
        films: Some(films),
        models,
        voices: state.voices.clone(),
    };
    replies.poll();
    Some(
        FloatingPanel::new(open.binding(), "Assistente", move || {
            Box::new(AssistantChat { replies: replies.clone() })
        })
        .bottom_inset(StatusBar::HEIGHT),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "test helpers fail loudly")]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::seed;
    use crate::update::Updater;

    fn replies() -> Replies {
        let today = Date::new(2026, 11, 15).expect("valid date");
        let db = Database::in_memory().expect("database");
        seed::load_demo(&db, today).expect("seed");
        Replies {
            store: Store::new(db, today, None, Updater::idle()),
            forms: Forms::new(),
            page: State::new(0),
            open: State::new(true),
            messages: State::new(Vec::new()),
            pending: Rc::new(RefCell::new(None)),
            films: None,
            models: None,
            voices: None,
        }
    }

    fn contents(replies: &Replies) -> String {
        replies.store.read(|db| db.contents().map(|contents| format!("{contents:?}")))
    }

    fn said(replies: &Replies, answer: &str) -> String {
        let call = read_tool_call(answer, ASSISTANT_TOOLS);
        replies.reply(call.as_ref(), "a pergunta", &UiWaker::default()).text()
    }

    #[test]
    fn the_balance_comes_from_the_database() {
        let replies = replies();
        let total: i64 = replies.store.read(|db| db.accounts()).iter().map(|a| a.balance_cents).sum();
        let text = said(&replies, r#"{"tool":"saldo_das_contas","args":{}}"#);
        assert!(text.contains(&format_brl(total)), "{text}");
    }

    #[test]
    fn a_number_the_model_wrote_is_never_shown_as_the_balance() {
        let replies = replies();
        let text = said(&replies, r#"{"tool":"saldo_das_contas","args":{"valor":"999999"}}"#);
        assert!(!text.contains("999.999"), "{text}");
    }

    #[test]
    fn spending_can_be_about_one_category() {
        let replies = replies();
        let all = said(&replies, r#"{"tool":"gastos_por_categoria","args":{}}"#);
        assert!(all.starts_with("Você gastou"), "{all}");
        let first = replies.store.read(|db| db.budget_lines(replies.store.today));
        let line = first.iter().find(|line| line.spent_cents > 0).expect("something spent");
        let answer =
            format!(r#"{{"tool":"gastos_por_categoria","args":{{"categoria":"{}"}}}}"#, line.category);
        let one = said(&replies, &answer);
        assert!(one.contains(&line.category) && one.contains(&format_brl(line.spent_cents)), "{one}");
        let not_one = said(&replies, r#"{"tool":"gastos_por_categoria","args":{"categoria":"mais"}}"#);
        assert_eq!(not_one, all, "no such category: it is about everything");
    }

    #[test]
    fn a_screen_opens_and_an_unknown_one_goes_to_the_films() {
        let replies = replies();
        assert_eq!(
            said(&replies, r#"{"tool":"abrir_tela","args":{"tela":"relatórios"}}"#),
            "Abri Relatórios."
        );
        assert_eq!(replies.page.get(), 5);
        let lost = said(&replies, r#"{"tool":"abrir_tela","args":{"tela":"cassino"}}"#);
        assert!(lost.starts_with("Nenhum filme"), "{lost}");
        assert_eq!(replies.page.get(), 5, "it stayed");
    }

    #[test]
    fn preparing_a_spending_saves_nothing() {
        let replies = replies();
        let before = contents(&replies);
        let call =
            read_tool_call(r#"{"tool":"lancar_despesa","args":{"descricao":"padaria"}}"#, ASSISTANT_TOOLS);
        let text = replies.reply(call.as_ref(), "gastei 50 na padaria", &UiWaker::default()).text();
        assert!(text.contains("R$ 50,00") && text.contains("padaria"), "{text}");
        assert_eq!(contents(&replies), before);
    }

    #[test]
    fn preparing_a_bill_finds_the_amount_and_the_day_and_saves_nothing() {
        let replies = replies();
        let before = contents(&replies);
        let call = read_tool_call(r#"{"tool":"criar_conta","args":{"descricao":"luz"}}"#, ASSISTANT_TOOLS);
        let question = "Criar um pagamento de luz para o dia 3 no valor de 100";
        let text = replies.reply(call.as_ref(), question, &UiWaker::default()).text();
        assert!(text.contains("luz") && text.contains("R$ 100,00"), "{text}");
        assert!(text.contains("03/12/2026"), "the 3rd already passed in this month: {text}");
        assert!(!text.contains("100,00, 100"), "{text}");
        assert_eq!(contents(&replies), before);
    }

    #[test]
    fn an_amount_or_a_day_the_model_made_up_is_ignored() {
        let replies = replies();
        let call = read_tool_call(
            r#"{"tool":"criar_conta","args":{"descricao":"luz","valor":100,"dia":9}}"#,
            ASSISTANT_TOOLS,
        );
        let text = replies.reply(call.as_ref(), "agendar a luz", &UiWaker::default()).text();
        assert_eq!(text, "Preparei a conta luz. Confira e salve.");
    }

    #[test]
    fn a_whole_request_fills_the_bill_form() {
        let replies = replies();
        let call = read_tool_call(
            r#"{"tool":"criar_conta","args":{"descricao":"luz","categoria":"Moradia"}}"#,
            ASSISTANT_TOOLS,
        );
        let question = "criar conta de luz de 330 para o dia 20, da Ana, todo mês, em 2x";
        let text = replies.reply(call.as_ref(), question, &UiWaker::default()).text();
        assert_eq!(
            text,
            "Preparei a conta luz, R$ 330,00 em 2x, 20/11/2026, Moradia, Ana, todo mês. Confira e salve."
        );
    }

    #[test]
    fn a_short_message_carries_the_one_before_it() {
        let replies = replies();
        replies.messages.update(|all| {
            all.push(ChatMessage::person("Agendar a luz para o dia 3"));
            all.push(ChatMessage::assistant("Não achei."));
            all.push(ChatMessage::person("Mas eu quero criar"));
        });
        assert_eq!(
            replies.with_context("Mas eu quero criar"),
            "Agendar a luz para o dia 3. Mas eu quero criar"
        );
        let long = "agora quero ver quanto gastei neste mês";
        assert_eq!(replies.with_context(long), long);
    }

    #[test]
    fn an_amount_left_in_the_sentence_is_still_found() {
        let replies = replies();
        let call =
            read_tool_call(r#"{"tool":"lancar_despesa","args":{"descricao":"padaria"}}"#, ASSISTANT_TOOLS);
        let text = replies.reply(call.as_ref(), "gastei 50 reais na padaria", &UiWaker::default()).text();
        assert!(text.contains("R$ 50,00"), "{text}");
    }

    #[test]
    fn a_bill_is_found_by_name_and_is_not_paid_until_confirmed() {
        let replies = replies();
        let open = replies.store.read(|db| db.open_bills());
        let bill = open.first().expect("a bill in the demo");
        let answer = format!(r#"{{"tool":"pagar_conta","args":{{"conta":"{}"}}}}"#, bill.description);
        let text = said(&replies, &answer);
        assert!(text.contains(&bill.description), "{text}");
        assert_eq!(replies.store.read(|db| db.open_bills()).len(), open.len());
        let lost = said(&replies, r#"{"tool":"pagar_conta","args":{"conta":"dragão"}}"#);
        assert!(lost.starts_with("Não achei"), "{lost}");
        let call = read_tool_call(r#"{"tool":"pagar_conta","args":{"conta":"gasolina"}}"#, ASSISTANT_TOOLS);
        let spent = replies.reply(call.as_ref(), "paguei 80 de gasolina", &UiWaker::default()).text();
        assert!(spent.starts_with("Preparei a despesa gasolina, R$ 80,00"), "{spent}");
    }

    #[test]
    fn what_the_app_did_not_offer_does_nothing() {
        let replies = replies();
        let before = contents(&replies);
        let text = said(&replies, r#"{"tool":"apagar_tudo","args":{}}"#);
        assert!(text.starts_with("Nenhum filme"), "{text}");
        assert_eq!(contents(&replies), before);
    }

    /// The conversation on a screen, for clicking its buttons.
    fn on_screen(replies: &Replies) -> Ui {
        let mut ui = Ui::new(AssistantChat { replies: replies.clone() }, Theme::raven_light());
        ui.set_viewport(Size::new(500.0, 700.0), 1.0);
        ui.update();
        ui
    }

    fn names(ui: &mut Ui) -> Vec<String> {
        ui.access_tree().nodes.iter().filter_map(|node| node.name.clone()).collect()
    }

    fn click(ui: &mut Ui, name: &str) {
        let tree = ui.access_tree();
        let node = tree.nodes.iter().find(|node| node.name.as_deref() == Some(name));
        ui.perform_action(node.unwrap_or_else(|| panic!("no {name}")).id, AccessAction::Click);
        ui.update();
    }

    fn spending(replies: &Replies) -> usize {
        let today = replies.store.today;
        replies
            .store
            .read(|db| db.ledger(today, None))
            .iter()
            .filter(|row| row.description == "padaria")
            .count()
    }

    fn ask_for_a_spending(replies: &Replies) -> ChatMessage {
        let call =
            read_tool_call(r#"{"tool":"lancar_despesa","args":{"descricao":"padaria"}}"#, ASSISTANT_TOOLS);
        replies.reply(call.as_ref(), "gastei 50 na padaria", &UiWaker::default())
    }

    #[test]
    fn a_spending_can_be_saved_only_once_from_its_answer() {
        let replies = replies();
        let answer = ask_for_a_spending(&replies);
        replies.messages.update(|all| all.push(answer));
        let mut ui = on_screen(&replies);
        click(&mut ui, "Conferir e salvar");
        assert_eq!(spending(&replies), 0, "opening the form saves nothing");
        replies.forms.transaction.save(&replies.store);
        ui.update();
        assert_eq!(spending(&replies), 1);
        let shown = names(&mut ui);
        assert!(shown.iter().any(|name| name == "Salvo"), "{shown:?}");
        assert!(!shown.iter().any(|name| name == "Conferir e salvar"), "{shown:?}");
    }

    #[test]
    fn a_second_click_on_save_saves_nothing_more() {
        let replies = replies();
        let answer = ask_for_a_spending(&replies);
        replies.messages.update(|all| all.push(answer));
        let mut ui = on_screen(&replies);
        click(&mut ui, "Conferir e salvar");
        replies.forms.transaction.save(&replies.store);
        replies.forms.transaction.save(&replies.store);
        assert_eq!(spending(&replies), 1);
    }

    #[test]
    fn cancelling_leaves_the_answer_ready_to_save() {
        let replies = replies();
        let answer = ask_for_a_spending(&replies);
        replies.messages.update(|all| all.push(answer));
        let mut ui = on_screen(&replies);
        click(&mut ui, "Conferir e salvar");
        // Another way into the same form forgets what the answer asked.
        replies.forms.transaction.start_new(&replies.store, Flow::Expense);
        ui.update();
        assert!(names(&mut ui).iter().any(|name| name == "Conferir e salvar"));
        assert!(!names(&mut ui).iter().any(|name| name == "Salvo"));
    }

    #[test]
    fn a_bill_paid_from_its_answer_says_so() {
        let replies = replies();
        let open = replies.store.read(|db| db.open_bills());
        let bill = open.first().expect("a bill in the demo");
        let answer = replies.pay(&bill.description, "paguei a conta");
        replies.messages.update(|all| all.push(answer));
        let mut ui = on_screen(&replies);
        click(&mut ui, "Conferir e pagar");
        replies.forms.pay.save(&replies.store);
        ui.update();
        let paid = replies.store.read(|db| db.bill(bill.id)).expect("the bill");
        assert!(paid.paid_on.is_some(), "paid once");
        let shown = names(&mut ui);
        assert!(shown.iter().any(|name| name == "Paga"), "{shown:?}");
        assert!(!shown.iter().any(|name| name == "Conferir e pagar"), "{shown:?}");
    }

    #[test]
    fn a_bill_created_from_its_answer_can_be_saved_only_once() {
        let replies = replies();
        let call = read_tool_call(
            r#"{"tool":"criar_conta","args":{"descricao":"Dragão","categoria":"Moradia"}}"#,
            ASSISTANT_TOOLS,
        );
        let answer = replies.reply(call.as_ref(), "criar conta de 330 para o dia 20", &UiWaker::default());
        replies.messages.update(|all| all.push(answer));
        let mut ui = on_screen(&replies);
        click(&mut ui, "Conferir e salvar");
        replies.forms.bill.save(&replies.store);
        ui.update();
        let shown = names(&mut ui);
        assert!(shown.iter().any(|name| name == "Salvo"), "{shown:?}");
        assert!(!shown.iter().any(|name| name == "Conferir e salvar"), "{shown:?}");
    }
}
