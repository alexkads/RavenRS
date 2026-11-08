//! A conversation with one chat: what a message asks for, the questions back and saving it.

use std::path::Path;
use std::time::Instant;

use crate::db::Database;
use crate::model::{Flow, TransactionInput};
use crate::reminders::{ReminderSettings, TelegramNotice, TelegramNotices};
use crate::today;

use super::*;
use super::{api::*, reading::*, receipts::*, voice::*};

pub(super) fn handle_message(service: &Service, message: Message) -> Result<(), String> {
    let chat = message.chat.id;
    let user = message.from.as_ref().map(|user| user.id).ok_or_else(|| "remetente ausente".to_owned())?;
    if let Some(token) = message.text.as_deref().and_then(start_token) {
        let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
        if db
            .claim_telegram_pairing(&pairing_hash(token), chat, user, unix_seconds())
            .map_err(|e| e.to_string())?
        {
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
            let who = db.telegram_chat_member(chat).map_err(|e| e.to_string())?;
            let who = who.map(|member| format!(" em nome de {}", member.name)).unwrap_or_default();
            send_message(
                service,
                chat,
                &format!("Telegram conectado a este Raven{who}. Para lançar, diga: `gastei 50 no mercado`."),
            )?;
        } else {
            send_message(service, chat, "Esse QR expirou ou já foi usado. Gere outro no Raven.")?;
        }
        return Ok(());
    }
    let linked = Database::open(&service.config.db_path)
        .and_then(|db| db.telegram_chat_is_linked(chat, user))
        .map_err(|e| e.to_string())?;
    if !linked {
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
        send_message(service, chat, "Escaneie o QR em Ajustes no Raven para conectar este chat.")?;
        return Ok(());
    }
    if let Some(document) = message.document {
        if document.mime_type.as_deref() != Some("application/pdf")
            || document.file_size.is_some_and(|n| n > MAX_PDF_BYTES)
        {
            send_message(service, chat, "Envie um PDF de até 14 MB.")?;
            return Ok(());
        }
        let bytes = download_pdf(service, &document.file_id)?;
        let amount_cents = match total_in_pdf(&bytes)? {
            Total::One(cents) => cents,
            Total::Several(values) => {
                let rows: Vec<Vec<(String, String)>> = values
                    .chunks(2)
                    .map(|pair| {
                        pair.iter()
                            .map(|cents| (format!("R$ {}", format_brl(*cents)), format!("rf:valor:{cents}")))
                            .collect()
                    })
                    .collect();
                let question = "Encontrei mais de um valor no PDF. Qual é o total? Ou diga o valor: `gastei 22,74 no mercado`.";
                return send_with_buttons(service, chat, question, &rows);
            }
        };
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(
            chat,
            Pending { amount_cents, from_pdf: true, since: Instant::now(), step: Step::Amount },
        );
        send_message(
            service,
            chat,
            &format!(
                "Valor encontrado: R$ {}. Envie `despesa Categoria` ou `receita Categoria`, e quem gastou se quiser, em até 5 minutos.",
                format_brl(amount_cents)
            ),
        )?;
        return Ok(());
    }
    let text = match message.voice {
        Some(voice) => match hear(service, chat, &voice)? {
            Some(text) => Some(text),
            None => return Ok(()),
        },
        None => message.text,
    };
    let Some(text) = text.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    if !text.starts_with('/') {
        let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
        let me = db.telegram_chat_member(chat).map_err(|e| e.to_string())?;
        if let Some(answer) = crate::questions::answer(&db, today(), &text, me.as_ref()) {
            // A question leaves whatever is pending as it was.
            return send_message(service, chat, &answer.map_err(|e| e.to_string())?);
        }
    }
    let said = folded(&text);
    let pending = service.pending.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
    let keep = |pending: Pending| -> Result<(), String> {
        service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, pending);
        Ok(())
    };
    if text == "/avisos" || text.starts_with("/avisos ") {
        if let Some(pending) = pending {
            keep(pending)?;
        }
        return notices_command(service, chat, &text);
    }
    if let Some(cents) = text.strip_prefix("/valor ").and_then(|cents| cents.parse::<i64>().ok()) {
        if !(1..=100_000_000).contains(&cents) {
            return send_message(service, chat, "Valor inválido.");
        }
        let chosen =
            Pending { amount_cents: cents, from_pdf: true, since: Instant::now(), step: Step::Amount };
        keep(chosen)?;
        return send_message(
            service,
            chat,
            &format!(
                "Valor: R$ {}. Envie `despesa Categoria` ou `receita Categoria`, e quem gastou se quiser, em até 5 minutos.",
                format_brl(cents)
            ),
        );
    }
    if text == "/desfazer" || said == "desfazer" {
        if let Some(pending) = pending {
            keep(pending)?;
        }
        return undo(service, chat);
    }
    let asking = pending.as_ref().is_some_and(|p| matches!(p.step, Step::Category(_) | Step::Confirm(_)));
    if text == "/cancelar" || (asking && NO_WORDS.contains(&said.as_str())) {
        let what = match pending {
            Some(Pending { amount_cents, step: Step::Category(reply) | Step::Confirm(reply), .. }) => {
                summary(amount_cents, &reply)
            }
            Some(Pending { amount_cents, .. }) => format!("R$ {}", format_brl(amount_cents)),
            None => return send_message(service, chat, "❌ Nada a cancelar."),
        };
        return send_message(service, chat, &format!("❌ Cancelado: {what}. Nada foi salvo."));
    }
    let expired = pending.as_ref().is_some_and(|p| p.since.elapsed() > PENDING_TTL);
    let pending = pending.filter(|_| !expired);
    let confirmed = text == "/confirmar" || YES_WORDS.contains(&said.as_str());
    let command = confirmed || text.starts_with('/');
    match pending {
        Some(Pending { step: Step::Confirm(reply), amount_cents, from_pdf, since }) if confirmed => {
            let id = match save_transaction(&service.config.db_path, amount_cents, from_pdf, &reply) {
                Ok(id) => id,
                Err(error) => {
                    keep(Pending { amount_cents, from_pdf, since, step: Step::Confirm(reply) })?;
                    return Err(error);
                }
            };
            let new_category = !reply.known;
            let summary = summary(amount_cents, &Reply { known: true, ..reply });
            let rows = [vec![("Desfazer".to_owned(), "rf:undo".to_owned())]];
            send_with_buttons(service, chat, &format!("✅ Lançado: {summary}"), &rows)?;
            let saved = Saved { id, summary, new_category, at: Instant::now() };
            service.saved.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, saved);
        }
        Some(Pending { step: Step::Confirm(_), amount_cents, from_pdf, .. }) if text == "/corrigir" => {
            keep(Pending { amount_cents, from_pdf, since: Instant::now(), step: Step::Amount })?;
            send_message(service, chat, "Envie de novo, por exemplo: `despesa Mercado`.")?;
        }
        Some(Pending { step: Step::Category(reply), amount_cents, from_pdf, since }) if text == "/criar" => {
            ask(service, chat, Pending { amount_cents, from_pdf, since, step: Step::Confirm(reply) })?;
        }
        Some(Pending { step: Step::Category(mut reply), amount_cents, from_pdf, since })
            if text.starts_with("/categoria ") =>
        {
            let id = text["/categoria ".len()..].parse::<i64>().ok();
            let chosen = Database::open(&service.config.db_path)
                .and_then(|db| db.categories())
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|category| Some(category.id) == id && category.flow == reply.flow);
            let step = match chosen {
                Some(category) => {
                    (reply.category, reply.known) = (category.name, true);
                    Step::Confirm(reply)
                }
                None => Step::Category(reply),
            };
            ask(service, chat, Pending { amount_cents, from_pdf, since, step })?;
        }
        pending if !command => match take_amount(&text) {
            Err(question) => {
                if let Some(pending) = pending {
                    keep(pending)?;
                }
                send_message(service, chat, &question)?;
            }
            Ok(Some((amount_cents, rest))) => {
                let said =
                    Pending { amount_cents, from_pdf: false, since: Instant::now(), step: Step::Amount };
                read(service, chat, said, &rest)?;
            }
            Ok(None) => match pending {
                Some(pending) => read(service, chat, pending, &text)?,
                None if expired => {
                    send_message(service, chat, "O prazo desse lançamento terminou. Envie de novo.")?;
                }
                None => send_message(service, chat, NOTHING_WAITING)?,
            },
        },
        Some(pending) => {
            keep(pending)?;
            send_message(service, chat, "Responda com os botões, ou envie `despesa Categoria` de novo.")?;
        }
        None if expired => send_message(service, chat, "O prazo desse lançamento terminou. Envie de novo.")?,
        None => send_message(service, chat, NOTHING_WAITING)?,
    }
    Ok(())
}

/// Takes back the last transaction this chat saved, while it is recent: only that one, and
/// only from the chat that saved it. A category created with it goes too, if unused.
pub(super) fn undo(service: &Service, chat: i64) -> Result<(), String> {
    let saved = service.saved.lock().map_err(|_| "estado do bot indisponível")?.remove(&chat);
    let Some(saved) = saved else {
        return send_message(service, chat, "Não há lançamento recente deste chat para desfazer.");
    };
    if saved.at.elapsed() > UNDO_TTL {
        return send_message(
            service,
            chat,
            "O prazo para desfazer terminou. Apague o lançamento no Raven, em Lançamentos.",
        );
    }
    let db = Database::open(&service.config.db_path).map_err(|e| e.to_string())?;
    let Some(transaction) = db.transaction(saved.id).map_err(|e| e.to_string())? else {
        return send_message(service, chat, "Esse lançamento já foi apagado no Raven.");
    };
    db.delete_transaction(saved.id).map_err(|e| e.to_string())?;
    if saved.new_category {
        // Refused while another transaction or a bill uses it, and then it stays.
        let _ = db.delete_category(transaction.input.category_id);
    }
    send_message(service, chat, &format!("↩️ Desfeito: {}, apagado.", saved.summary))
}

/// Sends every paired chat what is due, each notice with a button that turns its kind off.
/// A failure is logged, never fatal: notices are not worth stopping the bot for.
pub(super) fn tell(service: &Service) {
    let path = &service.config.db_path;
    let file = TelegramNotices::file(path);
    let mut notices = TelegramNotices::load(&file);
    let chosen = ReminderSettings::load(&crate::reminders::settings_file(path));
    let hour = u8::try_from(jiff::Zoned::now().hour()).unwrap_or_default();
    let due = Database::open(path)
        .and_then(|db| Ok((db.telegram_chats()?, notices.due(&db, today(), hour, &chosen)?)));
    let (chats, due) = match due {
        Ok((chats, due)) if !chats.is_empty() => (chats, due),
        Ok(_) => return,
        Err(error) => return tracing::warn!(%error, "could not look for Telegram notices"),
    };
    for (kind, text) in &due {
        let off = [vec![(format!("Desligar: {}", kind.label()), format!("rf:aviso:{}:off", kind.word()))]];
        for chat in &chats {
            if let Err(error) = send_with_buttons(service, *chat, text, &off) {
                tracing::warn!(%error, "could not send a Telegram notice");
            }
        }
    }
    if let Err(error) = notices.save(&file) {
        tracing::warn!(%error, "could not save what the Telegram notices told");
    }
}

/// `/avisos` lists the notices with a button to turn each on or off; `/avisos contas
/// desligar` does it.
pub(super) fn notices_command(service: &Service, chat: i64, text: &str) -> Result<(), String> {
    let file = TelegramNotices::file(&service.config.db_path);
    let mut notices = TelegramNotices::load(&file);
    let mut words = text.split_whitespace().skip(1);
    if let (Some(word), Some(state)) = (words.next(), words.next()) {
        let kind = TelegramNotice::ALL.into_iter().find(|kind| kind.word() == folded(word));
        let on = match state {
            "ligar" => true,
            "desligar" => false,
            _ => {
                return send_message(
                    service,
                    chat,
                    "Use `/avisos contas ligar` ou `/avisos contas desligar`.",
                );
            }
        };
        let Some(kind) = kind else {
            return send_message(service, chat, "Os avisos são: contas, orcamento e resumo.");
        };
        notices.set(kind, on);
        notices.save(&file)?;
        let done = if on { "ligado" } else { "desligado" };
        return send_message(
            service,
            chat,
            &format!("Aviso {done}: {}. Veja todos em /avisos.", kind.label()),
        );
    }
    let rows: Vec<Vec<(String, String)>> = TelegramNotice::ALL
        .iter()
        .map(|kind| {
            let (label, state) = if notices.is_on(*kind) { ("Desligar", "off") } else { ("Ligar", "on") };
            vec![(format!("{label}: {}", kind.label()), format!("rf:aviso:{}:{state}", kind.word()))]
        })
        .collect();
    let lines: Vec<String> = TelegramNotice::ALL
        .iter()
        .map(|kind| {
            format!("{}: {}", kind.label(), if notices.is_on(*kind) { "ligado" } else { "desligado" })
        })
        .collect();
    let text = format!("Avisos do Raven, a partir da hora escolhida em Ajustes:\n{}", lines.join("\n"));
    send_with_buttons(service, chat, &text, &rows)
}

pub(super) const NOTHING_WAITING: &str =
    "Nada aguardando. Para lançar, diga o valor: `gastei 50 no mercado`. Ou envie o PDF do comprovante.";

/// Reads what `text` says about the pending amount and asks the next question: which
/// category, whether to save, or what is missing, the amount kept.
pub(super) fn read(service: &Service, chat: i64, pending: Pending, text: &str) -> Result<(), String> {
    let Pending { amount_cents, from_pdf, since, .. } = pending;
    let read = Database::open(&service.config.db_path)
        .and_then(|db| Ok((db.members()?, db.categories()?, db.telegram_chat_member(chat)?)))
        .map_err(|e| e.to_string())
        .map(|(members, categories, me)| {
            // Who sends is who spent, unless the message names someone else.
            read_reply(text, &members, &categories)
                .map(|reply| Reply { member: reply.member.or(me), ..reply })
        });
    let waiting = Pending { amount_cents, from_pdf, since, step: Step::Amount };
    match read {
        Ok(Ok(reply)) => {
            let step = if reply.known { Step::Confirm(reply) } else { Step::Category(reply) };
            ask(service, chat, Pending { step, ..waiting })
        }
        Ok(Err(question)) => {
            let question = if from_pdf {
                question
            } else {
                format!("Valor: R$ {}. {question}", format_brl(amount_cents))
            };
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, waiting);
            send_message(service, chat, &question)
        }
        Err(error) => {
            service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, waiting);
            Err(error)
        }
    }
}

pub(super) const YES_WORDS: &[&str] = &["sim", "confirmar", "confirmo", "confirma", "ok"];

pub(super) const NO_WORDS: &[&str] = &["nao", "cancelar", "cancela"];

pub(super) const CONFIRM_BUTTONS: [(&str, &str); 3] =
    [("Confirmar", "rf:ok"), ("Corrigir", "rf:fix"), ("Cancelar", "rf:no")];

/// How many of the most used categories are offered when the one said does not exist.
pub(super) const CATEGORY_CHOICES: usize = 6;

/// Keeps `pending` and asks its question: confirm what will be saved, or which category,
/// since a category is never created without the person saying so.
pub(super) fn ask(service: &Service, chat: i64, pending: Pending) -> Result<(), String> {
    let (rows, question) = match &pending.step {
        Step::Category(reply) => {
            let used = Database::open(&service.config.db_path)
                .and_then(|db| db.categories_by_use(reply.flow))
                .map_err(|e| e.to_string())?;
            let mut rows: Vec<Vec<(String, String)>> = used
                .iter()
                .take(CATEGORY_CHOICES)
                .map(|category| (category.name.clone(), format!("rf:cat:{}", category.id)))
                .collect::<Vec<_>>()
                .chunks(2)
                .map(<[_]>::to_vec)
                .collect();
            rows.push(vec![
                (format!("Criar “{}”", reply.category), "rf:new".to_owned()),
                ("Cancelar".to_owned(), "rf:no".to_owned()),
            ]);
            let kind = if reply.flow == Flow::Income { "receitas" } else { "despesas" };
            let pick = if used.is_empty() { "Crie" } else { "Escolha uma das mais usadas ou crie" };
            (rows, format!("Não encontrei a categoria “{}” em {kind}. {pick}:", reply.category))
        }
        Step::Confirm(reply) => {
            let row = CONFIRM_BUTTONS.iter().map(|(label, data)| ((*label).to_owned(), (*data).to_owned()));
            (vec![row.collect()], format!("{}\nConfirma?", summary(pending.amount_cents, reply)))
        }
        Step::Amount => return Err("nada a perguntar".into()),
    };
    service.pending.lock().map_err(|_| "estado do bot indisponível")?.insert(chat, pending);
    send_with_buttons(service, chat, &question, &rows)
}

/// One line with everything that will be saved: "Despesa · Mercado · Ana · R$ 87,50".
pub(super) fn summary(amount_cents: i64, reply: &Reply) -> String {
    let who = reply.member.as_ref().map(|member| format!(" · {}", member.name)).unwrap_or_default();
    let new = if reply.known { "" } else { " (nova)" };
    format!("{} · {}{new}{who} · R$ {}", reply.flow.label(), reply.category, format_brl(amount_cents))
}

pub(super) fn save_transaction(
    path: &Path,
    amount_cents: i64,
    from_pdf: bool,
    reply: &Reply,
) -> Result<i64, String> {
    let db = Database::open(path).map_err(|e| e.to_string())?;
    let accounts = db.accounts().map_err(|e| e.to_string())?;
    let account_id = match std::env::var("RAVEN_ACCOUNT_ID") {
        Ok(id) => id.parse().map_err(|_| "RAVEN_ACCOUNT_ID inválido")?,
        Err(_) => match (chosen_account(path), accounts.as_slice()) {
            (Some(id), _) => id,
            (None, [only]) => only.id,
            (None, []) => return Err("cadastre uma conta no Raven antes de lançar".into()),
            (None, _) => return Err("escolha em Ajustes, no cartão Telegram, a conta dos lançamentos".into()),
        },
    };
    if !accounts.iter().any(|a| a.id == account_id) {
        return Err("a conta configurada não existe no Raven".into());
    }
    db.atomically(|db| {
        let category_id = db.find_or_create_category(&reply.category, reply.flow)?;
        db.add_transaction(&TransactionInput {
            date: today(),
            description: format!(
                "{} — {}",
                if from_pdf { "Comprovante PDF" } else { "Telegram" },
                reply.category
            ),
            amount_cents,
            flow: reply.flow,
            account_id,
            category_id,
            member_id: reply.member.as_ref().map(|member| member.id),
        })
    })
    .map_err(|e| e.to_string())
}
