//! Questions the Telegram bot answers, by text or by voice, with the numbers the reports
//! use: the `ledger` view behind "Extrato do mês", the accounts' balances and the open
//! bills. "quanto gastei com mercado esse mês?", "quanto a Ana gastou em outubro?",
//! "quanto recebi mês passado?", "qual o saldo?", "quais contas vencem?". Everyone sees the
//! whole family's numbers, as in the app; "eu", "meu" and "gastei" are the chat's member.

use ravenrs::prelude::Date;

use crate::db::{Database, DbResult};
use crate::model::{Flow, Member};
use crate::money::format_brl;

const MONTHS: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "marco",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];
const MONTH_TITLES: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];
const STARTS: &[&str] = &["quanto", "quanta", "quantos", "quantas", "qual", "quais", "saldo", "resumo"];
const EXPENSE: &[&str] =
    &["gastei", "gastou", "gastamos", "gasto", "gastos", "despesa", "despesas", "paguei", "pagou", "pagamos"];
const INCOME: &[&str] = &[
    "recebi",
    "recebeu",
    "recebemos",
    "receita",
    "receitas",
    "ganhei",
    "ganhou",
    "entrou",
    "entrada",
    "entradas",
];
/// Words a question is made of that name nothing to look for.
const FILLER: &[&str] = &[
    "quanto", "quanta", "quantos", "quantas", "qual", "quais", "foi", "foram", "o", "a", "os", "as", "de",
    "da", "do", "das", "dos", "com", "em", "no", "na", "nos", "nas", "esse", "este", "essa", "esta", "neste",
    "nesse", "mes", "passado", "anterior", "ate", "agora", "hoje", "eu", "ja", "total", "meu", "minha",
    "nosso", "nossa", "e", "por", "pra", "para", "que", "reais", "ai", "la", "ao", "todo", "tudo",
];
/// The most bills or categories one answer lists.
const LISTED: usize = 5;

/// Words in the first person singular: the one asking, who is `me` when the chat has a member.
const ME: &[&str] =
    &["eu", "meu", "minha", "meus", "minhas", "gastei", "paguei", "comprei", "recebi", "ganhei"];

/// The answer to `text`, if it is a question; one not understood says what can be asked.
/// `me` is the member the chat speaks for, if any.
pub fn answer(db: &Database, today: Date, text: &str, me: Option<&Member>) -> Option<DbResult<String>> {
    let keys: Vec<String> = text
        .split_whitespace()
        .map(|word| folded(word.trim_matches(|c: char| !c.is_alphanumeric())))
        .filter(|key| !key.is_empty())
        .collect();
    let asks = keys.first().is_some_and(|key| STARTS.contains(&key.as_str()))
        || (text.trim_end().ends_with('?') && !text.chars().any(|c| c.is_ascii_digit()));
    asks.then(|| answer_to(db, today, &keys, me))
}

const HELP: &str = "Posso responder: quanto gastei (com uma categoria, de uma pessoa, este mês, mês passado ou em outubro), quanto recebi, qual o saldo e quais contas vencem.";

fn answer_to(db: &Database, today: Date, keys: &[String], me: Option<&Member>) -> DbResult<String> {
    let has = |words: &[&str]| keys.iter().any(|key| words.contains(&key.as_str()));
    if has(&["saldo"]) || (has(&["quanto"]) && has(&["tenho", "temos"])) {
        return balance(db);
    }
    if has(&["conta", "contas", "boleto", "boletos"])
        && has(&["pagar", "vence", "vencem", "vencidas", "abertas", "aberto"])
        || has(&["vence", "vencem", "vencimento", "vencimentos"])
    {
        return bills(db, today);
    }
    let flow = if has(INCOME) {
        Flow::Income
    } else if has(EXPENSE) {
        Flow::Expense
    } else {
        return Ok(HELP.to_owned());
    };
    let month = month_in(keys, today);
    let only_today = has(&["hoje"]);

    let mut known: Vec<&str> = Vec::new();
    let categories = db.categories()?;
    let category = categories
        .iter()
        .filter(|category| category.flow == flow)
        .filter_map(|category| found(keys, &category.name).map(|words| (words, category)))
        .max_by_key(|(words, _)| words.len());
    let members = db.members()?;
    let named = members
        .iter()
        .filter_map(|member| found(keys, &member.name).map(|words| (words, member)))
        .max_by_key(|(words, _)| words.len());
    // A member named wins; "eu" or "gastei" is the one asking; "gastamos" is everyone.
    let member = named.or_else(|| me.filter(|_| has(ME)).map(|me| (Vec::new(), me)));
    for (words, _) in category
        .iter()
        .map(|(w, c)| (w, c.name.as_str()))
        .chain(member.iter().map(|(w, m)| (w, m.name.as_str())))
    {
        known.extend(words.iter().map(String::as_str));
    }
    let left: Vec<&str> = keys
        .iter()
        .map(String::as_str)
        .filter(|key| {
            !FILLER.contains(key)
                && !EXPENSE.contains(key)
                && !INCOME.contains(key)
                && !MONTHS.contains(key)
                && !known.contains(key)
        })
        .collect();
    if category.is_none() && !left.is_empty() {
        let kind = if flow == Flow::Income { "receita" } else { "despesa" };
        return Ok(format!(
            "Não encontrei “{}” entre as categorias de {kind} nem entre os membros.",
            left.join(" ")
        ));
    }

    let kind = if flow == Flow::Income { "Receita" } else { "Despesa" };
    let today_iso = today.to_string();
    let rows: Vec<_> = db
        .ledger(month, None)?
        .into_iter()
        .filter(|row| row.kind == kind)
        .filter(|row| category.as_ref().is_none_or(|(_, c)| row.category == c.name))
        .filter(|row| member.as_ref().is_none_or(|(_, m)| row.member == m.name))
        .filter(|row| !only_today || row.date == today_iso)
        .collect();
    let cents = |value: f64| (value * 100.0).round() as i64;
    let total: i64 = rows.iter().map(|row| cents(row.value)).sum();

    let what = if flow == Flow::Income { "Receitas" } else { "Gastos" };
    let with = category.as_ref().map(|(_, c)| format!(" com {}", c.name)).unwrap_or_default();
    let who = member.as_ref().map(|(_, m)| format!(" de {}", m.name)).unwrap_or_default();
    let when = if only_today {
        "hoje".to_owned()
    } else {
        format!("em {} de {}", MONTH_TITLES[usize::from(month.month() - 1)], month.year())
    };
    let count = match rows.len() {
        1 => " (1 lançamento)".to_owned(),
        n => format!(" ({n} lançamentos)"),
    };
    let mut text = format!("{what}{with}{who} {when}: {}{count}.", format_brl(total));
    if category.is_none() && !rows.is_empty() {
        let mut by_category: Vec<(String, i64)> = Vec::new();
        for row in &rows {
            match by_category.iter_mut().find(|(name, _)| *name == row.category) {
                Some((_, sum)) => *sum += cents(row.value),
                None => by_category.push((row.category.clone(), cents(row.value))),
            }
        }
        by_category.sort_by_key(|(name, sum)| (std::cmp::Reverse(*sum), name.clone()));
        let top: Vec<String> = by_category
            .iter()
            .take(LISTED)
            .map(|(name, sum)| format!("{name} {}", format_brl(*sum)))
            .collect();
        text.push_str(&format!(" Por categoria: {}.", top.join(", ")));
    }
    Ok(text)
}

fn balance(db: &Database) -> DbResult<String> {
    let accounts = db.accounts()?;
    let total: i64 = accounts.iter().map(|account| account.balance_cents).sum();
    let each: Vec<String> = accounts
        .iter()
        .map(|account| format!("{} {}", account.name, format_brl(account.balance_cents)))
        .collect();
    Ok(match each.as_slice() {
        [] => "Nenhuma conta cadastrada no Raven.".to_owned(),
        _ => format!("Saldo: {}. {}.", format_brl(total), each.join(", ")),
    })
}

fn bills(db: &Database, today: Date) -> DbResult<String> {
    let bills = db.open_bills()?;
    if bills.is_empty() {
        return Ok("Nenhuma conta em aberto.".to_owned());
    }
    let late = bills.iter().filter(|bill| bill.due < today).count();
    let each: Vec<String> = bills
        .iter()
        .take(LISTED)
        .map(|bill| {
            let due = format!("{:02}/{:02}", bill.due.day(), bill.due.month());
            let late = if bill.due < today { ", vencida" } else { "" };
            format!("{} {} ({due}{late})", bill.description, format_brl(bill.amount_cents))
        })
        .collect();
    let more = bills.len().saturating_sub(LISTED);
    let more = if more > 0 { format!(" e mais {more}") } else { String::new() };
    Ok(format!("{} em aberto, {late} vencidas: {}{more}.", bills.len(), each.join("; ")))
}

/// The month a question is about: this one, "mês passado", or one named, in the year that
/// keeps it from being ahead of today.
fn month_in(keys: &[String], today: Date) -> Date {
    let this = today.first_of_month();
    let said = |a: &str, b: &str| keys.windows(2).any(|pair| pair[0] == a && pair[1] == b);
    if said("mes", "passado") || said("mes", "anterior") {
        return this.add_months(-1);
    }
    match keys.iter().find_map(|key| MONTHS.iter().position(|month| month == key)) {
        Some(index) => {
            let month = index as i32 + 1;
            let back = (i32::from(this.month()) - month).rem_euclid(12);
            this.add_months(-back)
        }
        None => this,
    }
}

/// The words of `name`, if they all appear in a row in `keys`.
fn found(keys: &[String], name: &str) -> Option<Vec<String>> {
    let words: Vec<String> = name.split_whitespace().map(folded).collect();
    (!words.is_empty() && keys.windows(words.len()).any(|window| window == words.as_slice())).then_some(words)
}

fn folded(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, reason = "tests fail loudly")]

    use super::*;
    use crate::model::{AccountKind, BillInput, Recurrence, TransactionInput};

    fn family() -> (Database, Date) {
        let db = Database::in_memory().expect("db");
        let bank = db.add_account("Banco", AccountKind::Checking, 1_000_00).expect("account");
        let ana = db.add_member("Ana").expect("member");
        let market = db.add_category("Mercado", Flow::Expense, 0).expect("category");
        let pharmacy = db.add_category("Farmácia", Flow::Expense, 0).expect("category");
        let salary = db.add_category("Salário", Flow::Income, 0).expect("category");
        let add = |date: Date, cents, flow, category_id, member_id| {
            db.add_transaction(&TransactionInput {
                date,
                description: "x".into(),
                amount_cents: cents,
                flow,
                account_id: bank,
                category_id,
                member_id,
            })
            .expect("transaction");
        };
        let today = Date::new(2026, 11, 7).expect("date");
        add(today, 50_00, Flow::Expense, market, Some(ana));
        add(Date::new(2026, 11, 2).expect("date"), 30_00, Flow::Expense, market, None);
        add(Date::new(2026, 11, 3).expect("date"), 20_00, Flow::Expense, pharmacy, Some(ana));
        add(Date::new(2026, 10, 15).expect("date"), 70_00, Flow::Expense, market, None);
        add(Date::new(2026, 11, 5).expect("date"), 3_000_00, Flow::Income, salary, None);
        (db, today)
    }

    fn ask(db: &Database, today: Date, text: &str) -> String {
        answer(db, today, text, None).expect(text).expect("database")
    }

    #[test]
    fn answers_how_much_was_spent() {
        let (db, today) = family();
        assert_eq!(
            ask(&db, today, "quanto gastei com mercado esse mês?"),
            "Gastos com Mercado em novembro de 2026: R$ 80,00 (2 lançamentos)."
        );
        assert_eq!(
            ask(&db, today, "Quanto a Ana gastou?"),
            "Gastos de Ana em novembro de 2026: R$ 70,00 (2 lançamentos). Por categoria: Mercado R$ 50,00, Farmácia R$ 20,00."
        );
        assert_eq!(
            ask(&db, today, "quanto gastei com mercado mês passado"),
            "Gastos com Mercado em outubro de 2026: R$ 70,00 (1 lançamento)."
        );
        assert_eq!(
            ask(&db, today, "quanto gastei em outubro?"),
            ask(&db, today, "quanto gastei mês passado")
        );
        assert!(
            ask(&db, today, "quanto gastei em dezembro").contains("dezembro de 2025"),
            "never ahead of today"
        );
        assert!(ask(&db, today, "quanto gastei hoje").starts_with("Gastos hoje: R$ 50,00"));
        assert!(ask(&db, today, "Quanto recebi?").starts_with("Receitas em novembro de 2026: R$ 3.000,00"));
        assert!(ask(&db, today, "quanto gastei com farmacia").contains("R$ 20,00"), "without the accent");
    }

    #[test]
    fn i_is_the_member_of_the_chat() {
        let (db, today) = family();
        let ana = db.members().unwrap().into_iter().next().expect("Ana");
        let mine = |text: &str| answer(&db, today, text, Some(&ana)).expect(text).expect("database");
        assert!(
            mine("quanto gastei?").starts_with("Gastos de Ana em novembro de 2026: R$ 70,00"),
            "gastei is Ana"
        );
        assert!(mine("quanto eu gastei com mercado?").starts_with("Gastos com Mercado de Ana"), "eu is Ana");
        assert!(
            mine("quanto gastamos?").starts_with("Gastos em novembro de 2026: R$ 100,00"),
            "gastamos is everyone"
        );
        assert!(
            mine("quanto a família gastou com mercado?").starts_with("Gastos com Mercado em"),
            "the whole family"
        );
        assert!(
            ask(&db, today, "quanto gastei?").starts_with("Gastos em novembro"),
            "a chat with no member: everyone"
        );
    }

    #[test]
    fn never_answers_about_something_it_does_not_know() {
        let (db, today) = family();
        assert_eq!(
            ask(&db, today, "quanto gastei com academia?"),
            "Não encontrei “academia” entre as categorias de despesa nem entre os membros."
        );
        assert_eq!(ask(&db, today, "qual a capital da França?"), HELP);
        assert!(answer(&db, today, "gastei 50 no mercado", None).is_none(), "a transaction, not a question");
        assert!(answer(&db, today, "gastei 50 no mercado?", None).is_none(), "an amount is never a question");
        assert!(answer(&db, today, "despesa Mercado", None).is_none());
    }

    #[test]
    fn answers_the_balance_and_the_bills() {
        let (db, today) = family();
        assert_eq!(ask(&db, today, "qual o saldo?"), "Saldo: R$ 3.830,00. Banco R$ 3.830,00.");
        assert_eq!(ask(&db, today, "quais contas vencem?"), "Nenhuma conta em aberto.");
        let market = db.categories().unwrap()[1].id;
        for (description, cents, due) in
            [("Luz", 180_00, Date::new(2026, 11, 3)), ("Internet", 99_90, Date::new(2026, 11, 20))]
        {
            db.add_bill(&BillInput {
                description: description.into(),
                category_id: market,
                member_id: None,
                amount_cents: cents,
                due: due.expect("date"),
                recurrence: Recurrence::None,
                installments: 1,
                barcode: String::new(),
            })
            .expect("bill");
        }
        assert_eq!(
            ask(&db, today, "quais contas vencem?"),
            "2 em aberto, 1 vencidas: Luz R$ 180,00 (03/11, vencida); Internet R$ 99,90 (20/11)."
        );
    }
}
