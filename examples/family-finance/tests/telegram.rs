//! The Telegram bot against a Telegram that lives in this test, the way MSW stands in for
//! a server: the bot binary and the window talk HTTP to it, and the test plays the person
//! in the chat. Nothing reaches the internet, and it runs the same on every system.
#![allow(clippy::expect_used, clippy::unwrap_used, reason = "test helpers fail loudly")]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use family_finance::db::Database;
use family_finance::model::{AccountKind, BillInput, Flow, Recurrence, TransactionInput};
use family_finance::reminders::{ReminderSettings, TelegramNotice, TelegramNotices, settings_file};
use family_finance::telegram::{choose_account, pairing_hash};
use serde_json::{Value, json};

const BOT_TOKEN: &str = "123456:TESTE";
const CHAT: i64 = 111;
const STRANGER: i64 = 222;
const BRUNO: i64 = 333;

// ---------------------------------------------------------------- the stand-in Telegram

#[derive(Default)]
struct Inbox {
    updates: Vec<Value>,
    next_id: i64,
    sent: Vec<Value>,
    files: HashMap<String, Vec<u8>>,
    get_me_delay: Duration,
}

#[derive(Clone)]
struct FakeTelegram {
    base: String,
    inbox: Arc<(Mutex<Inbox>, Condvar)>,
}

impl FakeTelegram {
    fn start(get_me_delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("address"));
        let inbox =
            Arc::new((Mutex::new(Inbox { next_id: 1, get_me_delay, ..Inbox::default() }), Condvar::new()));
        let shared = inbox.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let inbox = shared.clone();
                std::thread::spawn(move || serve_connection(stream, &inbox));
            }
        });
        Self { base, inbox }
    }

    /// The fake of another process: the window's test drives the one its parent started.
    fn at(base: String) -> Self {
        Self { base, inbox: Arc::default() }
    }

    fn file(&self, name: &str, bytes: Vec<u8>) {
        self.inbox.0.lock().unwrap().files.insert(name.to_owned(), bytes);
    }

    fn sent(&self) -> Vec<Value> {
        let mut response = ureq::get(&format!("{}/_sent", self.base)).call().expect("sent");
        serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap()
    }

    fn push(&self, update: Value) {
        ureq::post(&format!("{}/_push", self.base))
            .header("content-type", "application/json")
            .send(update.to_string())
            .expect("push");
    }

    /// Sends what the person does (a message or a tap) and returns the bot's answer, or
    /// `None` after `wait`.
    fn say_within(&self, update: Value, wait: Duration, mut tick: impl FnMut()) -> Option<String> {
        let before = self.sent().len();
        self.push(update);
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            tick();
            let sent = self.sent();
            if sent.len() > before {
                return sent.last().and_then(|m| m["text"].as_str()).map(str::to_owned);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }

    fn say(&self, update: Value) -> String {
        self.say_within(update, Duration::from_secs(20), || ()).unwrap_or_else(|| "(sem resposta)".into())
    }
}

fn serve_connection(stream: TcpStream, inbox: &Arc<(Mutex<Inbox>, Condvar)>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let (method, path) = (parts.next().unwrap_or("").to_owned(), parts.next().unwrap_or("").to_owned());
        let mut length = 0;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).unwrap_or(0) == 0 {
                return;
            }
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("content-length")
            {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let (status, bytes) = route(&method, &path, &body, inbox);
        let head =
            format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n", bytes.len());
        if writer.write_all(head.as_bytes()).and_then(|()| writer.write_all(&bytes)).is_err() {
            return;
        }
    }
}

fn reply(result: Value) -> (&'static str, Vec<u8>) {
    ("200 OK", json!({ "ok": true, "result": result }).to_string().into_bytes())
}

fn route(
    method: &str,
    path: &str,
    body: &Value,
    inbox: &Arc<(Mutex<Inbox>, Condvar)>,
) -> (&'static str, Vec<u8>) {
    let (lock, changed) = &**inbox;
    let api = format!("/bot{BOT_TOKEN}/");
    let files = format!("/file/bot{BOT_TOKEN}/");
    if path == "/_sent" {
        let sent = lock.lock().unwrap().sent.clone();
        return ("200 OK", Value::Array(sent).to_string().into_bytes());
    }
    if path == "/_push" {
        let mut inbox = lock.lock().unwrap();
        let mut update = body.clone();
        update["update_id"] = json!(inbox.next_id);
        inbox.next_id += 1;
        inbox.updates.push(update);
        changed.notify_all();
        return reply(json!(true));
    }
    if let Some(name) = path.strip_prefix(&files) {
        return match lock.lock().unwrap().files.get(name) {
            Some(bytes) => ("200 OK", bytes.clone()),
            None => ("404 Not Found", Vec::new()),
        };
    }
    let Some(call) = path.strip_prefix(&api) else {
        return (
            "401 Unauthorized",
            json!({"ok": false, "description": "Unauthorized"}).to_string().into_bytes(),
        );
    };
    match (method, call) {
        (_, "getMe") => {
            let delay = lock.lock().unwrap().get_me_delay;
            std::thread::sleep(delay);
            reply(json!({ "id": 1, "is_bot": true, "username": "raven_teste_bot" }))
        }
        (_, "getUpdates") => {
            let offset = body["offset"].as_i64().unwrap_or(0);
            let deadline = Instant::now() + Duration::from_millis(500);
            let mut inbox = lock.lock().unwrap();
            loop {
                let ready: Vec<Value> = inbox
                    .updates
                    .iter()
                    .filter(|u| u["update_id"].as_i64() >= Some(offset))
                    .cloned()
                    .collect();
                let left = deadline.saturating_duration_since(Instant::now());
                if !ready.is_empty() || left.is_zero() {
                    return reply(Value::Array(ready));
                }
                inbox = changed.wait_timeout(inbox, left).unwrap().0;
            }
        }
        (_, "sendMessage") => {
            lock.lock().unwrap().sent.push(body.clone());
            reply(json!({ "message_id": 1 }))
        }
        (_, "answerCallbackQuery") => {
            // What the bot says on the button itself counts as an answer, as the person sees it.
            if body["text"].as_str().is_some_and(|text| !text.is_empty()) {
                lock.lock().unwrap().sent.push(json!({ "text": body["text"], "on_button": true }));
            }
            reply(json!(true))
        }
        (_, "editMessageReplyMarkup") => reply(json!(true)),
        (_, "getFile") => reply(json!({ "file_id": body["file_id"], "file_path": body["file_id"] })),
        _ => ("404 Not Found", json!({"ok": false, "description": "Not Found"}).to_string().into_bytes()),
    }
}

// ---------------------------------------------------------------- what the person sends

fn text(chat: i64, said: &str) -> Value {
    json!({ "message": { "chat": { "id": chat, "type": "private" }, "from": { "id": chat }, "text": said } })
}

fn document(chat: i64, file: &str, size: u64) -> Value {
    json!({ "message": {
        "chat": { "id": chat, "type": "private" }, "from": { "id": chat },
        "document": { "file_id": file, "mime_type": "application/pdf", "file_size": size }
    } })
}

fn voice(chat: i64) -> Value {
    json!({ "message": {
        "chat": { "id": chat, "type": "private" }, "from": { "id": chat },
        "voice": { "file_id": "voz.ogg", "duration": 4, "mime_type": "audio/ogg", "file_size": 10_000 }
    } })
}

/// A tap on a button the bot sent, carrying its `callback_data`.
fn tap(chat: i64, data: &str) -> Value {
    json!({ "callback_query": {
        "id": "q", "from": { "id": chat }, "data": data,
        "message": { "message_id": 9, "chat": { "id": chat, "type": "private" } }
    } })
}

/// A one-page PDF with these lines in Helvetica, as plain as a bank's receipt.
fn receipt(lines: &[&str]) -> Vec<u8> {
    let escape = |line: &str| line.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
    let text: String = lines.iter().map(|line| format!("({}) Tj T*\n", escape(line))).collect();
    let content = format!("BT\n/F1 12 Tf\n16 TL\n50 780 Td\n{text}ET\n");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{content}endstream", content.len()),
    ];
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).into_bytes());
    }
    let xref = pdf.len();
    pdf.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for offset in offsets {
        pdf.extend(format!("{offset:010} 00000 n \n").into_bytes());
    }
    pdf.extend(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1)
            .into_bytes(),
    );
    pdf
}

fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("family-finance-telegram-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    folder
}

/// The bot's process, stopped even when the test fails.
struct Bot(Child);

impl Drop for Bot {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Points a process at the stand-in and at a home of its own, so no real token is read.
fn isolated(command: &mut Command, fake: &FakeTelegram, home: &Path) {
    command
        .env("TELEGRAM_BOT_TOKEN", BOT_TOKEN)
        .env("TELEGRAM_API_BASE", &fake.base)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("RAVEN_ACCOUNT_ID");
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

// ---------------------------------------------------------------- the bot

#[test]
fn the_bot_pairs_one_chat_and_saves_receipts() {
    let fake = FakeTelegram::start(Duration::ZERO);
    let home = folder("bot");
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    db.add_account("Carteira", AccountKind::Cash, 0).expect("account");
    let bank = db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let ana = db.add_member("Ana").expect("member");
    choose_account(&path, bank);
    // The notices have a test of their own; here they would come on Mondays between answers.
    let mut quiet = TelegramNotices::default();
    for kind in TelegramNotice::ALL {
        quiet.set(kind, false);
    }
    quiet.save(&TelegramNotices::file(&path)).expect("notices off");
    let code = "0123456789abcdef0123456789abcdef";
    db.begin_telegram_pairing(&pairing_hash(code), None, now()).expect("pairing");
    fake.file("pix.pdf", receipt(&["Comprovante de Pix", "Valor", "R$ 3.000,00", "Para: Maria Silva"]));
    fake.file(
        "boleto.pdf",
        receipt(&[
            "Pagamento de boleto",
            "Valor do documento R$ 543,58",
            "Desconto R$ 0,00",
            "Valor pago R$ 543,58",
        ]),
    );
    fake.file(
        "saldo.pdf",
        receipt(&["Comprovante de Pix", "Valor R$ 50,00", "Saldo disponivel R$ 1.234,56"]),
    );

    let mut command = Command::new(env!("CARGO_BIN_EXE_raven-telegram"));
    command.arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
    isolated(&mut command, &fake, &home);
    let _bot = Bot(command.spawn().expect("raven-telegram"));

    assert!(
        fake.say(document(CHAT, "pix.pdf", 2_000)).contains("Escaneie o QR"),
        "an unpaired chat is turned away"
    );
    assert!(
        fake.say(text(CHAT, &format!("/start rf1_{}", "f".repeat(32)))).contains("expirou ou já foi usado")
    );
    assert!(fake.say(text(CHAT, &format!("/start rf1_{code}"))).contains("Telegram conectado"));
    assert!(
        fake.say(text(STRANGER, &format!("/start rf1_{code}"))).contains("expirou ou já foi usado"),
        "one use"
    );
    assert!(fake.say(document(STRANGER, "pix.pdf", 2_000)).contains("Escaneie o QR"), "only the paired chat");
    let group = json!({ "message":
        { "chat": { "id": -5, "type": "group" }, "from": { "id": CHAT }, "text": "despesa Mercado" } });
    assert_eq!(fake.say_within(group, Duration::from_secs(2), || ()), None, "groups are ignored");

    assert!(fake.say(text(CHAT, "despesa Mercado")).contains("diga o valor"), "no amount, nothing waiting");
    assert!(
        fake.say(voice(CHAT)).contains("Para entender áudios"),
        "a voice note needs no receipt, only a model"
    );
    assert!(fake.say(text(CHAT, "gastei 50 no mercado e 30 na farmácia")).contains("mais de um valor"));
    let question = fake.say(text(CHAT, "50 reais no mercado"));
    assert!(question.contains("Valor: R$ 50,00") && question.contains("despesa ou receita"), "{question}");
    assert!(fake.say(text(CHAT, "qual o saldo?")).contains("Saldo: R$"), "a question in the middle");
    assert!(fake.say(text(STRANGER, "qual o saldo?")).contains("Escaneie o QR"), "only the paired chat asks");
    assert!(fake.say(text(CHAT, "despesa Mercado Ana")).contains("Não encontrei a categoria “Mercado”"));
    let question = fake.say(tap(CHAT, "rf:new"));
    assert!(question.contains("Despesa · Mercado (nova) · Ana · R$ 50,00"), "the amount kept: {question}");
    assert!(fake.say(tap(CHAT, "rf:ok")).contains("✅ Lançado"));
    let answer = fake.say(text(CHAT, "Quanto a Ana gastou com mercado esse mês?"));
    assert!(
        answer.starts_with("Gastos com Mercado de Ana em ") && answer.contains("R$ 50,00 (1 lançamento)"),
        "{answer}"
    );
    let question = fake.say(text(CHAT, "Gastei R$ 12,90 no mercado, foi a Ana."));
    assert!(
        question.contains("Despesa · Mercado · Ana · R$ 12,90") && question.contains("Confirma?"),
        "{question}"
    );
    assert!(fake.say(tap(CHAT, "rf:ok")).contains("✅ Lançado"), "said in one message, saved without a PDF");
    assert!(fake.say(text(CHAT, "gastei 7 na padaria")).contains("Não encontrei a categoria “Padaria”"));
    assert!(fake.say(tap(CHAT, "rf:new")).contains("Despesa · Padaria (nova) · R$ 7,00"));
    assert!(fake.say(tap(CHAT, "rf:ok")).contains("✅ Lançado: Despesa · Padaria · R$ 7,00"));
    let rows = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(rows[0][0]["text"], "Desfazer", "{rows}");
    assert!(fake.say(tap(STRANGER, "rf:undo")).contains("Escaneie o QR"), "only the chat that saved it");
    assert!(fake.say(tap(CHAT, "rf:undo")).contains("↩️ Desfeito: Despesa · Padaria · R$ 7,00, apagado."));
    assert!(
        fake.say(text(CHAT, "desfazer")).contains("Não há lançamento recente"),
        "only the last one, once"
    );
    let padaria: i64 = rusqlite::Connection::open(&path)
        .unwrap()
        .query_row("SELECT count(*) FROM categories WHERE name = 'Padaria'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(padaria, 0, "the category created with it goes too");
    assert!(fake.say(document(CHAT, "pix.pdf", 20 * 1024 * 1024)).contains("Envie um PDF de até 14 MB"));
    assert!(fake.say(document(CHAT, "pix.pdf", 2_000)).contains("R$ 3000,00"));
    assert!(fake.say(voice(CHAT)).contains("Para entender áudios"), "no model yet: says where to get one");
    assert!(fake.say(text(CHAT, "Salário")).contains("despesa ou receita"), "asks again, amount kept");
    let question = fake.say(text(CHAT, "receita Salário"));
    assert!(question.contains("Não encontrei a categoria “Salário” em receitas. Crie:"), "{question}");
    let rows = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(
        rows.as_array().map(Vec::len),
        Some(1),
        "no income category yet, only Criar and Cancelar: {rows}"
    );
    let question = fake.say(tap(CHAT, "rf:new"));
    assert!(
        question.contains("Receita · Salário (nova) · R$ 3000,00") && question.contains("Confirma?"),
        "{question}"
    );
    let buttons = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"][0].clone();
    assert_eq!(buttons.as_array().map(Vec::len), Some(3), "Confirmar, Corrigir e Cancelar: {buttons}");
    assert!(fake.say(tap(STRANGER, "rf:ok")).contains("Escaneie o QR"), "a stranger's tap saves nothing");
    assert!(fake.say(tap(CHAT, "rf:ok")).contains("✅ Lançado: Receita · Salário · R$ 3000,00"));

    assert!(
        fake.say(document(CHAT, "boleto.pdf", 2_000)).contains("R$ 543,58"),
        "the amount paid, not the discount"
    );
    assert!(
        fake.say(text(CHAT, "despesa energia elétrica, foi o Pedro")).contains("Pedro"),
        "asks, amount kept"
    );
    assert!(fake.say(text(CHAT, "despesa energia elétrica")).contains("Não encontrei a categoria"));
    assert!(fake.say(tap(CHAT, "rf:new")).contains("Despesa · Energia elétrica (nova) · R$"));
    assert!(fake.say(tap(CHAT, "rf:fix")).contains("Envie de novo"), "Corrigir asks again, amount kept");
    assert!(
        fake.say(text(CHAT, "despesa  energia   elétrica  Ana")).contains("Não encontrei a categoria"),
        "still not created: nothing was saved"
    );
    let question = fake.say(tap(CHAT, "rf:new"));
    assert!(question.contains("Despesa · Energia elétrica (nova) · Ana · R$ 543,58"), "{question}");
    assert!(fake.say(text(CHAT, "sim")).contains("✅ Lançado"), "a typed yes confirms too");

    assert!(fake.say(document(CHAT, "saldo.pdf", 2_000)).contains("mais de um valor"), "never guesses");
    let rows = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(
        (rows[0][0]["text"].as_str(), rows[0][1]["text"].as_str()),
        (Some("R$ 50,00"), Some("R$ 1234,56")),
        "the amounts it could be, to choose: {rows}"
    );
    assert!(fake.say(tap(CHAT, "rf:valor:5000")).contains("Valor: R$ 50,00"), "chosen, then what it was");
    assert!(fake.say(document(CHAT, "pix.pdf", 2_000)).contains("R$ 3000,00"));
    assert!(fake.say(text(CHAT, "/cancelar")).contains("❌ Cancelado: R$ 3000,00"));
    assert!(fake.say(text(CHAT, "despesa Aluguel")).contains("Nada aguardando"));
    assert!(fake.say(document(CHAT, "pix.pdf", 2_000)).contains("R$ 3000,00"));
    let question = fake.say(text(CHAT, "despesa Aluguel"));
    assert!(question.contains("Escolha uma das mais usadas ou crie"), "{question}");
    let rows = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(
        (rows[0][0]["text"].as_str(), rows[0][1]["text"].as_str()),
        (Some("Mercado"), Some("Energia elétrica")),
        "the most used first: {rows}"
    );
    let energy = rows[0][1]["callback_data"].as_str().unwrap().to_owned();
    assert!(
        fake.say(tap(CHAT, "rf:cat:999")).contains("Não encontrei a categoria"),
        "a category that is gone"
    );
    assert!(
        fake.say(tap(CHAT, &energy)).contains("Despesa · Energia elétrica · R$ 3000,00"),
        "picked, not created"
    );
    assert!(
        fake.say(tap(CHAT, "rf:no"))
            .contains("❌ Cancelado: Despesa · Energia elétrica · R$ 3000,00. Nada foi salvo."),
        "Cancelar saves nothing, and says what"
    );
    assert_eq!(
        fake.say(tap(CHAT, "rf:ok")),
        "Esse botão já foi usado.",
        "an old button, or one tapped twice on a slow network, says so on itself"
    );
    assert!(fake.sent().last().unwrap()["on_button"].as_bool().unwrap_or(false), "and nothing in the chat");

    let saved: Vec<(i64, i64, Option<i64>, String)> = {
        let conn = rusqlite::Connection::open(&path).expect("open");
        let mut query = conn
            .prepare("SELECT account_id, amount_cents, member_id, description FROM transactions ORDER BY amount_cents")
            .unwrap();
        query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let said = |cents, member, description: &str| (bank, cents, member, description.to_owned());
    assert_eq!(
        saved,
        [
            said(1_290, Some(ana), "Telegram — Mercado"),
            said(5_000, Some(ana), "Telegram — Mercado"),
            said(54_358, Some(ana), "Comprovante PDF — Energia elétrica"),
            said(300_000, None, "Comprovante PDF — Salário"),
        ],
        "said and sent, in the account chosen in Ajustes, with who spent"
    );

    db.revoke_telegram_chat(CHAT, now()).expect("revoke");
    assert!(
        fake.say(document(CHAT, "pix.pdf", 2_000)).contains("Escaneie o QR"),
        "Desvincular cuts the chat off"
    );
    drop(db);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_bot_tells_what_is_due_and_each_notice_turns_off() {
    let fake = FakeTelegram::start(Duration::ZERO);
    let home = folder("avisos");
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    let bank = db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let market = db.add_category("Mercado", Flow::Expense, 10_000).expect("category");
    let today = family_finance::today();
    db.add_transaction(&TransactionInput {
        date: today,
        description: "feira".into(),
        amount_cents: 15_000,
        flow: Flow::Expense,
        account_id: bank,
        category_id: market,
        member_id: None,
    })
    .expect("transaction");
    db.add_bill(&BillInput {
        description: "Luz".into(),
        category_id: market,
        member_id: None,
        amount_cents: 18_000,
        due: today.add_days(-2),
        recurrence: Recurrence::None,
        installments: 1,
        barcode: String::new(),
    })
    .expect("bill");
    // From midnight, so the test holds at any hour; the summary is for Mondays only.
    ReminderSettings { hour: 0, ..ReminderSettings::default() }.save(&settings_file(&path)).expect("hour");
    let mut notices = TelegramNotices::default();
    notices.set(TelegramNotice::Weekly, false);
    notices.save(&TelegramNotices::file(&path)).expect("notices");
    let code = "fedcba9876543210fedcba9876543210";
    db.begin_telegram_pairing(&pairing_hash(code), None, now()).expect("pairing");

    let mut command = Command::new(env!("CARGO_BIN_EXE_raven-telegram"));
    command.arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
    isolated(&mut command, &fake, &home);
    let _bot = Bot(command.spawn().expect("raven-telegram"));

    fake.say(text(CHAT, &format!("/start rf1_{code}")));
    let deadline = Instant::now() + Duration::from_secs(20);
    let told = loop {
        let sent = fake.sent();
        if sent.len() >= 3 || Instant::now() > deadline {
            break sent;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let texts: Vec<&str> = told.iter().filter_map(|m| m["text"].as_str()).collect();
    assert!(texts[0].contains("Telegram conectado"), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("1 conta vencida: Luz")), "{texts:?}");
    let budget = told
        .iter()
        .find(|m| m["text"].as_str().is_some_and(|t| t.starts_with("Orçamento estourado: Mercado")))
        .expect("the budget past its limit");
    assert_eq!(budget["reply_markup"]["inline_keyboard"][0][0]["callback_data"], "rf:aviso:orcamento:off");
    assert_eq!(told.len(), 3, "each once: {texts:?}");

    assert!(fake.say(tap(STRANGER, "rf:aviso:orcamento:off")).contains("Escaneie o QR"));
    assert!(fake.say(tap(CHAT, "rf:aviso:orcamento:off")).contains("Aviso desligado: Orçamento estourado"));
    assert!(!TelegramNotices::load(&TelegramNotices::file(&path)).budget, "kept beside the database");
    let list = fake.say(text(CHAT, "/avisos"));
    assert!(
        list.contains("Contas a vencer: ligado") && list.contains("Orçamento estourado: desligado"),
        "{list}"
    );
    let rows = fake.sent().last().unwrap()["reply_markup"]["inline_keyboard"].clone();
    assert_eq!(rows[1][0]["callback_data"], "rf:aviso:orcamento:on", "{rows}");
    assert!(fake.say(text(CHAT, "/avisos resumo ligar")).contains("Aviso ligado: Resumo semanal"));
    drop(db);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_bot_says_how_many_messages_waited_while_it_was_closed() {
    let fake = FakeTelegram::start(Duration::ZERO);
    let home = folder("atrasadas");
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let mut quiet = TelegramNotices::default();
    for kind in TelegramNotice::ALL {
        quiet.set(kind, false);
    }
    quiet.save(&TelegramNotices::file(&path)).expect("notices off");
    let code = "00112233445566778899aabbccddeeff";
    db.begin_telegram_pairing(&pairing_hash(code), None, now()).expect("pairing");
    // Sent an hour ago, while the app was closed; Telegram kept them.
    let earlier = |mut update: Value| {
        update["message"]["date"] = json!(now() - 3600);
        update
    };
    fake.push(earlier(text(CHAT, &format!("/start rf1_{code}"))));
    fake.push(earlier(text(CHAT, "qual o saldo?")));
    fake.push(earlier(text(STRANGER, "qual o saldo?")));

    let mut command = Command::new(env!("CARGO_BIN_EXE_raven-telegram"));
    command.arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
    isolated(&mut command, &fake, &home);
    let _bot = Bot(command.spawn().expect("raven-telegram"));

    let deadline = Instant::now() + Duration::from_secs(20);
    let sent = loop {
        let sent = fake.sent();
        if sent.len() >= 4 || Instant::now() > deadline {
            break sent;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let said: Vec<(i64, &str)> =
        sent.iter().map(|m| (m["chat_id"].as_i64().unwrap(), m["text"].as_str().unwrap())).collect();
    assert_eq!(said.len(), 4, "{said:?}");
    assert!(
        said[0].1.contains("Telegram conectado") && said[1].1.starts_with("Saldo:"),
        "answered in order: {said:?}"
    );
    assert_eq!(said[2], (STRANGER, "Escaneie o QR em Ajustes no Raven para conectar este chat."));
    assert_eq!(said[3].0, CHAT, "only the paired chat hears about it: {said:?}");
    assert!(said[3].1.starts_with("O Raven estava fechado: respondi agora 2 mensagens"), "{said:?}");
    assert!(
        fake.say(text(CHAT, "qual o saldo?")).starts_with("Saldo:"),
        "a message on time is just answered"
    );
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(fake.sent().len(), 5, "told once");
    drop(db);
    let _ = std::fs::remove_dir_all(&home);
}

/// One chat per person: who sends is who spent, unless the message names someone else.
#[test]
fn each_chat_speaks_for_its_member() {
    let fake = FakeTelegram::start(Duration::ZERO);
    let home = folder("membros");
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let ana = db.add_member("Ana").expect("member");
    let bruno = db.add_member("Bruno").expect("member");
    db.add_category("Mercado", Flow::Expense, 0).expect("category");
    let mut quiet = TelegramNotices::default();
    for kind in TelegramNotice::ALL {
        quiet.set(kind, false);
    }
    quiet.save(&TelegramNotices::file(&path)).expect("notices off");

    let mut command = Command::new(env!("CARGO_BIN_EXE_raven-telegram"));
    command.arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
    isolated(&mut command, &fake, &home);
    let _bot = Bot(command.spawn().expect("raven-telegram"));

    let ana_code = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    db.begin_telegram_pairing(&pairing_hash(ana_code), Some(ana), now()).expect("pairing");
    let hello = fake.say(text(CHAT, &format!("/start rf1_{ana_code}")));
    assert!(hello.contains("Telegram conectado a este Raven em nome de Ana"), "{hello}");
    let bruno_code = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    db.begin_telegram_pairing(&pairing_hash(bruno_code), Some(bruno), now()).expect("pairing");
    assert!(fake.say(text(BRUNO, &format!("/start rf1_{bruno_code}"))).contains("em nome de Bruno"));
    assert!(fake.say(text(STRANGER, "qual o saldo?")).contains("Escaneie o QR"), "no QR, no access");

    let question = fake.say(text(CHAT, "gastei 50 no mercado"));
    assert!(question.contains("Despesa · Mercado · Ana · R$ 50,00"), "Ana's chat: {question}");
    assert!(fake.say(tap(CHAT, "rf:ok")).contains("✅ Lançado"));
    let question = fake.say(text(BRUNO, "gastei 20 no mercado"));
    assert!(question.contains("Despesa · Mercado · Bruno · R$ 20,00"), "Bruno's chat: {question}");
    assert!(fake.say(tap(BRUNO, "rf:ok")).contains("✅ Lançado"));
    let question = fake.say(text(BRUNO, "gastei 7 no mercado, foi a Ana"));
    assert!(question.contains("Despesa · Mercado · Ana · R$ 7,00"), "the name said wins: {question}");
    assert!(fake.say(tap(BRUNO, "rf:no")).contains("❌ Cancelado"));

    let mine = fake.say(text(BRUNO, "quanto gastei?"));
    assert!(mine.starts_with("Gastos de Bruno em ") && mine.contains("R$ 20,00"), "{mine}");
    let all = fake.say(text(BRUNO, "quanto gastamos?"));
    assert!(all.contains("R$ 70,00"), "everyone sees the whole family: {all}");
    assert!(fake.say(text(CHAT, "quanto o Bruno gastou?")).contains("R$ 20,00"), "Ana can ask about Bruno");

    db.revoke_telegram_chat(CHAT, now()).expect("unlink Ana's chat");
    assert!(fake.say(text(CHAT, "quanto gastei?")).contains("Escaneie o QR"), "unlinked");
    assert!(fake.say(text(BRUNO, "quanto gastei?")).starts_with("Gastos de Bruno"), "Bruno's chat stays");
    drop(db);
    let _ = std::fs::remove_dir_all(&home);
}

/// On a bad network the bot falls and starts again; Telegram resends what it did not see
/// confirmed, and the bot must not answer it twice.
#[test]
fn a_bot_that_starts_again_does_not_answer_twice() {
    let fake = FakeTelegram::start(Duration::ZERO);
    let home = folder("de-novo");
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let mut quiet = TelegramNotices::default();
    for kind in TelegramNotice::ALL {
        quiet.set(kind, false);
    }
    quiet.save(&TelegramNotices::file(&path)).expect("notices off");
    let code = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";
    db.begin_telegram_pairing(&pairing_hash(code), None, now()).expect("pairing");
    let start = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_raven-telegram"));
        command.arg(&path).stdout(Stdio::null()).stderr(Stdio::null());
        isolated(&mut command, &fake, &home);
        Bot(command.spawn().expect("raven-telegram"))
    };

    let bot = start();
    assert!(fake.say(text(CHAT, &format!("/start rf1_{code}"))).contains("Telegram conectado"));
    assert!(fake.say(text(CHAT, "qual o saldo?")).starts_with("Saldo:"));
    drop(bot);
    let answered = fake.sent().len();
    // The stand-in resends everything from the offset asked for, as Telegram does with what
    // was never confirmed.
    let _again = start();
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(fake.sent().len(), answered, "nothing answered twice: {:?}", fake.sent());
    assert!(fake.say(text(CHAT, "qual o saldo?")).starts_with("Saldo:"), "and a new message is answered");
    drop(db);
    let _ = std::fs::remove_dir_all(&home);
}

// ---------------------------------------------------------------- the card in Ajustes

/// The window needs the stand-in's address in its environment, which a test cannot set on
/// itself, so it runs again in a process of its own with it.
#[test]
fn the_card_pairs_without_holding_up_the_window() {
    if std::env::var_os("FF_TELEGRAM_WINDOW").is_some() {
        return window_side();
    }
    let fake = FakeTelegram::start(Duration::from_millis(800));
    let home = folder("window");
    let mut command = Command::new(std::env::current_exe().expect("this test"));
    command.args([
        "--exact",
        "the_card_pairs_without_holding_up_the_window",
        "--nocapture",
        "--test-threads=1",
    ]);
    command.env("FF_TELEGRAM_WINDOW", &home);
    isolated(&mut command, &fake, &home);
    let output = command.output().expect("the window's test");
    let _ = std::fs::remove_dir_all(&home);
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(said.contains("1 passed"), "the window's side did not run: {said}");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn window_side() {
    use family_finance::ui::App;
    use family_finance::update::Updater;
    use ravenrs::prelude::*;

    let home = PathBuf::from(std::env::var_os("FF_TELEGRAM_WINDOW").expect("home"));
    let fake = FakeTelegram::at(std::env::var("TELEGRAM_API_BASE").expect("base"));
    let path = home.join("financas.db");
    let db = Database::open(&path).expect("database");
    db.add_account("Banco", AccountKind::Checking, 0).expect("account");
    let today = Date::new(2026, 11, 6).expect("date");
    let mut ui = Ui::new(App::new(db, today, Some(path), Updater::idle()), Theme::raven_light());
    ui.set_viewport(Size::new(1360.0, 3200.0), 1.0);
    ui.update();

    let names = |ui: &mut Ui| -> Vec<String> {
        ui.access_tree().nodes.iter().filter_map(|n| n.name.clone()).collect()
    };
    let find = |ui: &mut Ui, text: &str| names(ui).into_iter().find(|name| name.contains(text));
    let press = |ui: &mut Ui, name: &str| -> Duration {
        let tree = ui.access_tree();
        let node = tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|t| t.starts_with(name)));
        let id = node.unwrap_or_else(|| panic!("no {name}")).id;
        let start = Instant::now();
        ui.perform_action(id, AccessAction::Click);
        ui.update();
        start.elapsed()
    };
    let wait_for = |ui: &mut Ui, text: &str| -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            ui.update();
            if let Some(found) = find(ui, text) {
                return found;
            }
            assert!(Instant::now() < deadline, "never showed {text:?}: {:?}", names(ui));
            std::thread::sleep(Duration::from_millis(50));
        }
    };

    press(&mut ui, "Ajustes");
    wait_for(&mut ui, "Nenhum chat conectado.");
    let took = press(&mut ui, "Conectar Telegram");
    assert!(took < Duration::from_millis(400), "the click waited {took:?} on Telegram's 800 ms answer");
    assert!(find(&mut ui, "Gerando QR").is_some(), "says it is working meanwhile");
    let link = wait_for(&mut ui, "t.me/raven_teste_bot?start=rf1_");
    let code = link.rsplit("rf1_").next().expect("code").trim().to_owned();

    let mut tick = || {
        ui.update();
    };
    let answer =
        fake.say_within(text(CHAT, &format!("/start rf1_{code}")), Duration::from_secs(20), &mut tick);
    assert!(answer.is_some_and(|a| a.contains("Telegram conectado")), "the bot inside the window answered");
    wait_for(&mut ui, "Telegram conectado");
    assert!(find(&mut ui, "Gerar novo QR").is_none(), "the QR was used: no QR left on screen");
    assert!(find(&mut ui, "Conectar outro chat").is_some(), "another person can pair a chat of their own");
    press(&mut ui, "Desvincular");
    wait_for(&mut ui, "Nenhum chat conectado.");
}
