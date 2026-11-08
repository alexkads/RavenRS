//! Reading a transaction from words: the kind, the amount, written or spelled, and who.

use crate::model::{Category, Flow, Member};

use super::receipts::*;

/// What a reply says about a receipt: typed (`despesa Mercado Ana`) or spoken
/// ("despesa de mercado, quem gastou foi a Ana").
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Reply {
    pub(super) flow: Flow,
    pub(super) category: String,
    /// Whether the category already exists; one that does not is asked about first.
    pub(super) known: bool,
    pub(super) member: Option<Member>,
}

pub(super) const EXPENSE_WORDS: &[&str] =
    &["despesa", "gasto", "gastei", "gastou", "paguei", "pagou", "comprei", "comprou"];

pub(super) const INCOME_WORDS: &[&str] = &["receita", "recebi", "recebeu", "ganhei", "ganhou", "entrada"];

/// Words that only join the others; they are dropped from the ends of a category.
pub(super) const JOINING_WORDS: &[&str] = &[
    "a",
    "o",
    "as",
    "os",
    "de",
    "da",
    "do",
    "das",
    "dos",
    "com",
    "em",
    "na",
    "no",
    "e",
    "foi",
    "quem",
    "gastou",
    "recebeu",
    "pagou",
    "por",
    "pela",
    "pelo",
    "para",
    "pra",
    "categoria",
    "reais",
    "real",
];

/// What comes before a name nobody registered: "por Pedro", "foi o Pedro", "quem gastou Pedro",
/// "para o Pedro".
pub(super) const NAME_MARKERS: &[&str] = &["por", "foi", "quem", "para", "pra"];

/// Reads a reply against the people and categories this Raven already has. A name
/// that is not a member is asked about, never created.
pub(super) fn read_reply(text: &str, members: &[Member], categories: &[Category]) -> Result<Reply, String> {
    let mut words: Vec<String> = text
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()).to_owned())
        .filter(|word| !word.is_empty())
        .collect();
    let keys = |words: &[String]| words.iter().map(|word| folded(word)).collect::<Vec<_>>();

    let found = keys(&words).iter().enumerate().find_map(|(i, key)| {
        if EXPENSE_WORDS.iter().any(|word| sounds_like(key, word, 5)) {
            Some((i, Flow::Expense))
        } else {
            INCOME_WORDS.iter().any(|word| sounds_like(key, word, 5)).then_some((i, Flow::Income))
        }
    });
    let Some((at, flow)) = found else {
        return Err(
            "Diga se é despesa ou receita, por exemplo: `despesa Mercado` ou `despesa Mercado Ana`.".into()
        );
    };
    words.remove(at);

    let member = members
        .iter()
        .filter_map(|member| {
            let name = member.name.split_whitespace().map(folded).collect::<Vec<_>>();
            let said = keys(&words);
            let exact = find_run(&said, &name).map(|at| (at, true));
            let heard = || {
                said.windows(name.len().max(1))
                    .position(|window| window.iter().zip(&name).all(|(a, b)| sounds_like(a, b, 6)))
                    .map(|at| (at, false))
            };
            exact.or_else(heard).map(|(at, exact)| (at, name.len(), exact, member))
        })
        .max_by_key(|(_, len, exact, _)| (*len, *exact));
    let member = match member {
        Some((at, len, _, member)) => {
            words.drain(at..at + len);
            Some(member.clone())
        }
        None => {
            if let Some(name) = unknown_name(&words, categories) {
                return Err(format!(
                    "Não encontrei “{name}” entre os membros do Raven. Cadastre em Membros ou responda sem o nome."
                ));
            }
            None
        }
    };

    words.retain(|word| !word.chars().any(|c| c.is_ascii_digit()) && folded(word) != "r");
    while words.first().is_some_and(|word| JOINING_WORDS.contains(&folded(word).as_str())) {
        words.remove(0);
    }
    while words.last().is_some_and(|word| JOINING_WORDS.contains(&folded(word).as_str())) {
        words.pop();
    }
    if words.is_empty() {
        return Err("Faltou a categoria, por exemplo: `despesa Mercado`.".into());
    }
    let said = keys(&words);
    let known = categories
        .iter()
        .filter(|category| category.flow == flow)
        .filter(|category| {
            let name = category.name.split_whitespace().map(folded).collect::<Vec<_>>();
            find_run(&said, &name).is_some()
        })
        .max_by_key(|category| category.name.len());
    let (category, known) = match known {
        Some(category) => (category.name.clone(), true),
        None => (capitalized(&words.join(" ")), false),
    };
    Ok(Reply { flow, category, known, member })
}

/// The same word, or one of 6 letters or more heard with one letter wrong, missing or
/// extra: Whisper writes "despeza", and "astei" for "gastei". Neither may be shorter than
/// `shortest`: 5 for the keywords, 6 for names, so "Marina" is not taken for Maria.
pub(super) fn sounds_like(heard: &str, word: &str, shortest: usize) -> bool {
    if heard == word {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (heard.chars().collect(), word.chars().collect());
    if a.len().max(b.len()) < 6 || a.len().min(b.len()) < shortest || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let same_start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let same_end = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count();
    same_start + same_end + 1 >= a.len().max(b.len())
}

/// Where `run` appears whole in `words`.
pub(super) fn find_run(words: &[String], run: &[String]) -> Option<usize> {
    if run.is_empty() || run.len() > words.len() {
        return None;
    }
    words.windows(run.len()).position(|window| window == run)
}

/// A name after "por", "foi", "quem", "para" or "pra", at the end of a reply, that no member has.
/// "para" also comes before a place ("para o mercado"), so after it only a capitalized word
/// that names no category is taken for a name.
pub(super) fn unknown_name(words: &[String], categories: &[Category]) -> Option<String> {
    let keys = words.iter().map(|word| folded(word)).collect::<Vec<_>>();
    let marker = keys.iter().rposition(|key| NAME_MARKERS.contains(&key.as_str()))?;
    let (name, name_keys): (Vec<&str>, Vec<String>) = words[marker + 1..]
        .iter()
        .zip(&keys[marker + 1..])
        .skip_while(|(_, key)| JOINING_WORDS.contains(&key.as_str()))
        .map(|(word, key)| (word.as_str(), key.clone()))
        .unzip();
    if matches!(keys[marker].as_str(), "para" | "pra") {
        let capitalized = name.first().and_then(|word| word.chars().next()).is_some_and(char::is_uppercase);
        let a_category = categories
            .iter()
            .any(|category| category.name.split_whitespace().map(folded).collect::<Vec<_>>() == name_keys);
        if !capitalized || a_category {
            return None;
        }
    }
    (!name.is_empty() && name.len() <= 3).then(|| name.join(" "))
}

/// The amount a message says, in cents, and the message without it: "50", "50,90",
/// "R$ 1.234,56", "2 mil reais", "cinquenta reais e noventa centavos". Spelled-out numbers
/// count only before "reais" or "centavos", since "um" is also "a". Two amounts are asked
/// about, unless only one is marked as money (R$, reais, centavos or a comma).
pub(super) fn take_amount(text: &str) -> Result<Option<(i64, String)>, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = words
        .iter()
        .map(|word| {
            let key = folded(word);
            let key = key.strip_prefix("r$").unwrap_or(&key);
            key.trim_matches(|c: char| !c.is_alphanumeric()).to_owned()
        })
        .collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < keys.len() {
        match amount_at(&keys, at) {
            Some((cents, end, marked)) => {
                let marked =
                    marked || folded(words[at]).starts_with("r$") || at > 0 && keys[at - 1].is_empty();
                found.push((at, end, cents, marked));
                at = end;
            }
            None => at += 1,
        }
    }
    let marked: Vec<_> = found.iter().filter(|amount| amount.3).collect();
    let (start, end, cents) = match (found.as_slice(), marked.as_slice()) {
        ([], _) => return Ok(None),
        ([(start, end, cents, _)], _) | (_, [(start, end, cents, _)]) => (*start, *end, *cents),
        _ => {
            return Err(
                "Encontrei mais de um valor na mensagem. Diga só um, por exemplo: `gastei 50 no mercado`."
                    .into(),
            );
        }
    };
    let rest = words.iter().enumerate().filter(|(i, _)| !(start..end).contains(i)).map(|(_, word)| *word);
    Ok(Some((cents, rest.collect::<Vec<_>>().join(" "))))
}

/// An amount starting at `keys[at]`: its cents, where it ends, and whether a currency
/// word or a comma marks it as money.
pub(super) fn amount_at(keys: &[String], at: usize) -> Option<(i64, usize, bool)> {
    let word = |i: usize| keys.get(i).map_or("", String::as_str);
    let (mut cents, mut end, spelled, comma) = match digits_in_cents(word(at)) {
        Some(cents) => (cents, at + 1, false, word(at).contains(',')),
        None => {
            let (units, end) = spelled_number(keys, at)?;
            (units.checked_mul(100)?, end, true, false)
        }
    };
    if !spelled && word(end) == "mil" {
        cents = cents.checked_mul(1000)?;
        end += 1;
    }
    if word(end) == "centavos" {
        return (cents < 10_000).then_some((cents / 100, end + 1, true));
    }
    let currency = matches!(word(end), "reais" | "real" | "conto" | "contos");
    if currency {
        end += 1;
        let small = digits_in_cents(word(end + 1))
            .filter(|_| !word(end + 1).contains(','))
            .map(|cents| (cents / 100, end + 2))
            .or_else(|| spelled_number(keys, end + 1));
        if word(end) == "e"
            && let Some((extra, after)) = small
            && extra < 100
            && word(after) == "centavos"
        {
            cents += extra;
            end = after + 1;
        }
    }
    if spelled && !currency {
        return None;
    }
    (cents > 0 && cents <= 100_000_000).then_some((cents, end, currency || comma))
}

/// "50" → 5000, "50,9" → 5090, "1.234,56" → 123456, "1.234" → 123400, "50.90" → 5090.
pub(super) fn digits_in_cents(key: &str) -> Option<i64> {
    if !key.starts_with(|c: char| c.is_ascii_digit())
        || !key.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return None;
    }
    let (whole, fraction) = match key.rsplit_once(',') {
        Some((whole, fraction)) => (whole, fraction),
        None => match key.rsplit_once('.') {
            Some((whole, fraction)) if fraction.len() <= 2 && !whole.contains('.') => (whole, fraction),
            _ => (key, ""),
        },
    };
    if fraction.len() > 2 || fraction.contains('.') || whole.contains(',') {
        return None;
    }
    let whole: i64 = whole.replace('.', "").parse().ok()?;
    let fraction: i64 = if fraction.is_empty() { 0 } else { format!("{fraction:0<2}").parse().ok()? };
    whole.checked_mul(100)?.checked_add(fraction)
}

/// A number spelled out from `keys[at]`, "mil duzentos e trinta e cinco", and where it ends.
pub(super) fn spelled_number(keys: &[String], at: usize) -> Option<(i64, usize)> {
    let value = |key: &str| -> Option<i64> {
        Some(match key {
            "um" | "uma" => 1,
            "dois" | "duas" => 2,
            "tres" => 3,
            "quatro" => 4,
            "cinco" => 5,
            "seis" => 6,
            "sete" => 7,
            "oito" => 8,
            "nove" => 9,
            "dez" => 10,
            "onze" => 11,
            "doze" => 12,
            "treze" => 13,
            "catorze" | "quatorze" => 14,
            "quinze" => 15,
            "dezesseis" => 16,
            "dezessete" => 17,
            "dezoito" => 18,
            "dezenove" => 19,
            "vinte" => 20,
            "trinta" => 30,
            "quarenta" => 40,
            "cinquenta" => 50,
            "sessenta" => 60,
            "setenta" => 70,
            "oitenta" => 80,
            "noventa" => 90,
            "cem" | "cento" => 100,
            "duzentos" | "duzentas" => 200,
            "trezentos" | "trezentas" => 300,
            "quatrocentos" | "quatrocentas" => 400,
            "quinhentos" | "quinhentas" => 500,
            "seiscentos" | "seiscentas" => 600,
            "setecentos" | "setecentas" => 700,
            "oitocentos" | "oitocentas" => 800,
            "novecentos" | "novecentas" => 900,
            _ => return None,
        })
    };
    let is_number = |i: usize| keys.get(i).is_some_and(|key| key == "mil" || value(key).is_some());
    let (mut total, mut group, mut end) = (0, 0, at);
    while let Some(key) = keys.get(end) {
        if key == "mil" {
            total += group.max(1) * 1000;
            group = 0;
        } else if let Some(units) = value(key) {
            group += units;
        } else if !(key == "e" && end > at && is_number(end + 1)) {
            break;
        }
        end += 1;
    }
    (end > at).then_some((total + group, end))
}

pub(super) fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub(super) fn format_brl(cents: i64) -> String {
    format!("{:.2}", cents as f64 / 100.0).replace('.', ",")
}
