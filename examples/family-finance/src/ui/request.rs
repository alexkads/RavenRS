//! Reads a request written in plain words into the fields of a form. The amounts,
//! dates and names come from here, from the person's words, because a small model
//! makes them up. Each is checked against what the app has.

use ravenrs::prelude::*;
use ravenrs::text::fold;

use crate::model::{Account, Category, Member, Recurrence};
use crate::money::parse_brl;

/// The most installments the form offers.
const MOST_INSTALLMENTS: i32 = 60;
/// Fewer digits than this are not a barcode.
const BARCODE_DIGITS: usize = 20;
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

/// What a request says about a new bill or spending. What it does not say stays empty,
/// and the form uses its usual choice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Prefill {
    /// What it is.
    pub description: String,
    /// The amount, or the total when it is in installments.
    pub cents: Option<i64>,
    /// When it is due, or when it was spent.
    pub date: Option<Date>,
    /// Its category.
    pub category: Option<Category>,
    /// Who it belongs to.
    pub member: Option<Member>,
    /// The account it came from, for a spending.
    pub account: Option<Account>,
    /// How often it comes back.
    pub recurrence: Option<Recurrence>,
    /// How many installments, from 2.
    pub installments: Option<i32>,
    /// The barcode's digits.
    pub barcode: Option<String>,
}

/// Whether a date is ahead, as for a bill, or behind, as for what was spent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum When {
    /// The next time it is that day.
    Ahead,
    /// The last time it was that day.
    Behind,
}

pub(crate) fn folded(text: &str) -> String {
    text.chars().flat_map(fold).collect()
}

/// The words of a request, folded, without the punctuation around them.
fn words_of(text: &str) -> Vec<String> {
    folded(text)
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| ",.;:!?()".contains(c)).to_owned())
        .filter(|word| !word.is_empty())
        .collect()
}

fn digits_of(word: &str) -> String {
    word.chars().filter(char::is_ascii_digit).collect()
}

fn is_barcode(word: &str) -> bool {
    word.chars().all(|c| c.is_ascii_digit() || ".- ".contains(c)) && digits_of(word).len() >= BARCODE_DIGITS
}

/// The installments a request asks for: "em 3x", "3 parcelas", "3 vezes".
fn installments_in(words: &[String]) -> Option<(usize, i32)> {
    words.iter().enumerate().find_map(|(i, word)| {
        let times = word.strip_suffix('x').and_then(|n| n.parse::<i32>().ok()).map(|n| (i, n));
        let named = word.parse::<i32>().ok().filter(|_| {
            words.get(i + 1).is_some_and(|next| next.starts_with("parcela") || next.starts_with("veze"))
        });
        times.or(named.map(|n| (i, n))).filter(|(_, n)| (2..=MOST_INSTALLMENTS).contains(n))
    })
}

/// Whether the word at `i` belongs to a date: "dia 3".
fn after_day(words: &[String], i: usize) -> bool {
    i > 0 && words[i - 1] == "dia"
}

/// The amount: a number that is not a day, a date, a count of installments or a barcode.
fn amount_in(words: &[String]) -> Option<i64> {
    let times = installments_in(words).map(|(i, _)| i);
    words.iter().enumerate().find_map(|(i, word)| {
        let word = word.strip_prefix("r$").unwrap_or(word);
        let starts = word.chars().next().is_some_and(|c| c.is_ascii_digit());
        if !starts || word.contains('/') || after_day(words, i) || Some(i) == times || is_barcode(word) {
            return None;
        }
        parse_brl(&word.replace('.', ",")).ok().filter(|cents| *cents > 0)
    })
}

fn day_of_month(year: i32, month: u8, day: u8) -> Option<Date> {
    Date::new(year, month, day.min(days_in_month(year, month)))
}

/// That day of the nearest month, ahead of today or behind it.
fn nearest_day(today: Date, day: u8, when: When) -> Option<Date> {
    if !(1..=31).contains(&day) {
        return None;
    }
    let first = today.first_of_month();
    let this = day_of_month(first.year(), first.month(), day)?;
    let month = match when {
        When::Ahead if this < today => first.add_months(1),
        When::Behind if this > today => first.add_months(-1),
        _ => first,
    };
    day_of_month(month.year(), month.month(), day)
}

/// That day and month, in the year that keeps it ahead of today or behind it.
fn nearest_date(today: Date, day: u8, month: u8, when: When) -> Option<Date> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let this = day_of_month(today.year(), month, day)?;
    let year = match when {
        When::Ahead if this < today => today.year() + 1,
        When::Behind if this > today => today.year() - 1,
        _ => today.year(),
    };
    day_of_month(year, month, day)
}

/// The date a request names: "dia 3", "dia 3 de dezembro", "03/12", "03/12/2026",
/// "hoje", "amanhã", "ontem".
fn date_in(words: &[String], today: Date, when: When) -> Option<Date> {
    for (i, word) in words.iter().enumerate() {
        match word.as_str() {
            "hoje" => return Some(today),
            "amanha" => return Some(today.add_days(1)),
            "ontem" => return Some(today.add_days(-1)),
            _ => {}
        }
        let parts: Vec<&str> = word.split('/').collect();
        if let [day, month, rest @ ..] = parts.as_slice()
            && let (Ok(day), Ok(month)) = (day.parse::<u8>(), month.parse::<u8>())
        {
            return match rest {
                [year] => year
                    .parse()
                    .ok()
                    .and_then(|year: i32| Date::new(if year < 100 { 2000 + year } else { year }, month, day)),
                _ => nearest_date(today, day, month, when),
            };
        }
        if after_day(words, i)
            && let Ok(day) = word.parse::<u8>()
        {
            let of = words.get(i + 1).filter(|next| *next == "de").and_then(|_| words.get(i + 2));
            let month = of.and_then(|name| MONTHS.iter().position(|month| month == name));
            return month.map_or_else(
                || nearest_day(today, day, when),
                |month| nearest_date(today, day, month as u8 + 1, when),
            );
        }
    }
    None
}

fn recurrence_in(text: &str) -> Option<Recurrence> {
    let says = |phrases: &[&str]| phrases.iter().any(|phrase| text.contains(phrase));
    if says(&["todo mes", "todos os meses", "mensal", "por mes"]) {
        Some(Recurrence::Monthly)
    } else if says(&["todo ano", "todos os anos", "anual", "por ano"]) {
        Some(Recurrence::Yearly)
    } else {
        None
    }
}

/// Reads `request`. The model may suggest a category, which counts only if it exists:
/// one named in the words themselves comes first.
pub(crate) fn read_request(
    request: &str,
    today: Date,
    when: When,
    categories: &[Category],
    members: &[Member],
    accounts: &[Account],
    suggested_category: Option<&str>,
) -> Prefill {
    let (words, text) = (words_of(request), folded(request));
    let named = |name: &str| text.contains(&folded(name));
    let category = categories.iter().find(|category| named(&category.name)).or_else(|| {
        let suggested = folded(suggested_category?.trim());
        categories.iter().find(|category| folded(&category.name) == suggested)
    });
    Prefill {
        description: String::new(),
        cents: amount_in(&words),
        date: date_in(&words, today, when),
        category: category.cloned(),
        member: members.iter().find(|member| words.contains(&folded(&member.name))).cloned(),
        account: accounts.iter().find(|account| named(&account.name)).cloned(),
        recurrence: recurrence_in(&text),
        installments: installments_in(&words).map(|(_, count)| count),
        barcode: words.iter().find(|word| is_barcode(word)).map(|word| digits_of(word)),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "test helpers fail loudly")]
mod tests {
    use super::*;
    use crate::model::{AccountKind, Flow};

    fn today() -> Date {
        Date::new(2026, 11, 15).expect("valid date")
    }

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::new(year, month, day).expect("valid date")
    }

    fn read(request: &str, when: When) -> Prefill {
        let categories = [("Contas da casa"), ("Mercado")].map(|name| Category {
            id: 1,
            name: name.to_owned(),
            flow: Flow::Expense,
            budget_cents: 0,
        });
        let members = [Member { id: 7, name: "Ana".to_owned() }];
        let accounts = [Account {
            id: 3,
            name: "Poupança".to_owned(),
            kind: AccountKind::Savings,
            opening_cents: 0,
            balance_cents: 0,
        }];
        read_request(request, today(), when, &categories, &members, &accounts, None)
    }

    fn ahead(request: &str) -> Prefill {
        read(request, When::Ahead)
    }

    #[test]
    fn every_field_of_a_bill_comes_from_one_sentence() {
        let got = ahead(
            "criar conta de luz de 330 para o dia 3 de dezembro, da Ana, todo mês, em 3x, contas da casa, boleto 34191.79001 01043510047910201500000000000",
        );
        assert_eq!(got.cents, Some(33_000));
        assert_eq!(got.date, Some(date(2026, 12, 3)));
        assert_eq!(got.member.map(|m| m.name), Some("Ana".to_owned()));
        assert_eq!(got.recurrence, Some(Recurrence::Monthly));
        assert_eq!(got.installments, Some(3));
        assert_eq!(got.category.map(|c| c.name), Some("Contas da casa".to_owned()));
    }

    #[test]
    fn what_is_not_said_stays_empty() {
        assert_eq!(ahead("agendar a luz"), Prefill::default());
    }

    #[test]
    fn numbers_are_told_apart() {
        assert_eq!(ahead("para o dia 3 no valor de 100").cents, Some(10_000));
        assert_eq!(ahead("agendar para o dia 3").cents, None);
        assert_eq!(ahead("TV em 10x de 1500").cents, Some(150_000));
        assert_eq!(ahead("3 parcelas de 200").cents, Some(20_000));
        assert_eq!(ahead("R$ 12,50 de pão").cents, Some(1250));
        assert_eq!(ahead("r$99 de pão").cents, Some(9900));
        assert_eq!(ahead("para 03/12 de 40").cents, Some(4000));
    }

    #[test]
    fn dates_are_ahead_for_a_bill_and_behind_for_a_spending() {
        assert_eq!(ahead("dia 20").date, Some(date(2026, 11, 20)));
        assert_eq!(ahead("dia 3").date, Some(date(2026, 12, 3)));
        assert_eq!(read("dia 20", When::Behind).date, Some(date(2026, 10, 20)));
        assert_eq!(read("dia 3", When::Behind).date, Some(date(2026, 11, 3)));
        assert_eq!(ahead("03/01").date, Some(date(2027, 1, 3)));
        assert_eq!(ahead("03/12/2028").date, Some(date(2028, 12, 3)));
        assert_eq!(ahead("amanhã").date, Some(date(2026, 11, 16)));
        assert_eq!(read("ontem", When::Behind).date, Some(date(2026, 11, 14)));
        assert_eq!(ahead("dia 31 de fevereiro").date, Some(date(2027, 2, 28)));
        assert_eq!(ahead("dia 45").date, None);
    }

    #[test]
    fn names_must_exist() {
        assert_eq!(ahead("da Maria").member, None);
        assert_eq!(ahead("banana").member, None, "ana inside a word is not Ana");
        assert_eq!(ahead("paguei da poupanca").account.map(|a| a.id), Some(3));
    }

    #[test]
    fn a_suggested_category_counts_only_if_it_exists() {
        let categories =
            [Category { id: 1, name: "Mercado".to_owned(), flow: Flow::Expense, budget_cents: 0 }];
        let read = |suggested| read_request("pão", today(), When::Behind, &categories, &[], &[], suggested);
        assert_eq!(read(Some("mercado")).category.map(|c| c.id), Some(1));
        assert_eq!(read(Some("Astronomia")).category, None);
    }

    #[test]
    fn repetition_is_read() {
        assert_eq!(ahead("todos os meses").recurrence, Some(Recurrence::Monthly));
        assert_eq!(ahead("anual").recurrence, Some(Recurrence::Yearly));
    }
}
