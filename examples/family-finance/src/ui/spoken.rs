//! Writes money, dates and numbers in words, since the voice model misreads digits.

/// Names the voice would read as a word, or misread.
const LETTERS: [(&str, &str); 2] = [("PDF", "pê dê éfe"), ("CSV", "cê ésse vê")];
const MONTHS: [&str; 12] = [
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
const UNITS: [&str; 20] = [
    "zero",
    "um",
    "dois",
    "três",
    "quatro",
    "cinco",
    "seis",
    "sete",
    "oito",
    "nove",
    "dez",
    "onze",
    "doze",
    "treze",
    "quatorze",
    "quinze",
    "dezesseis",
    "dezessete",
    "dezoito",
    "dezenove",
];
const TENS: [&str; 10] =
    ["", "", "vinte", "trinta", "quarenta", "cinquenta", "sessenta", "setenta", "oitenta", "noventa"];
const HUNDREDS: [&str; 10] = [
    "",
    "cento",
    "duzentos",
    "trezentos",
    "quatrocentos",
    "quinhentos",
    "seiscentos",
    "setecentos",
    "oitocentos",
    "novecentos",
];

/// `number` below a thousand.
fn small(number: u64) -> String {
    match number {
        100 => "cem".to_owned(),
        0..=19 => UNITS[number as usize].to_owned(),
        20..=99 => {
            let (tens, unit) = (number / 10, number % 10);
            if unit == 0 {
                TENS[tens as usize].to_owned()
            } else {
                format!("{} e {}", TENS[tens as usize], UNITS[unit as usize])
            }
        }
        _ => {
            let (hundreds, rest) = (number / 100, number % 100);
            if rest == 0 {
                HUNDREDS[hundreds as usize].to_owned()
            } else {
                format!("{} e {}", HUNDREDS[hundreds as usize], small(rest))
            }
        }
    }
}

/// `number` in words, as far as billions.
pub(crate) fn in_words(number: u64) -> String {
    if number < 1000 {
        return small(number);
    }
    let groups = [
        (number / 1_000_000_000, "um bilhão", "bilhões"),
        (number / 1_000_000 % 1000, "um milhão", "milhões"),
        (number / 1000 % 1000, "mil", "mil"),
    ];
    let mut parts: Vec<String> = groups
        .iter()
        .filter(|(count, _, _)| *count > 0)
        .map(
            |(count, one, many)| {
                if *count == 1 { (*one).to_owned() } else { format!("{} {many}", small(*count)) }
            },
        )
        .collect();
    let rest = number % 1000;
    if rest > 0 {
        // "E" before the last part only when it is round or small, as people say it.
        let joined = rest <= 100 || rest.is_multiple_of(100);
        let rest = small(rest);
        parts.push(if joined { format!("e {rest}") } else { rest });
    }
    parts.join(" ")
}

fn money(reais: u64, centavos: u64) -> String {
    let reais_words = match reais {
        0 => None,
        1 => Some("um real".to_owned()),
        _ if reais.is_multiple_of(1_000_000) => Some(format!("{} de reais", in_words(reais))),
        _ => Some(format!("{} reais", in_words(reais))),
    };
    let centavos_words = match centavos {
        0 => None,
        1 => Some("um centavo".to_owned()),
        _ => Some(format!("{} centavos", small(centavos))),
    };
    match (reais_words, centavos_words) {
        (Some(reais), Some(centavos)) => format!("{reais} e {centavos}"),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => "zero real".to_owned(),
    }
}

/// How `text` should be read: digits in words, so that the voice says them right.
pub fn spoken(text: &str) -> String {
    let mut text = text.to_owned();
    for (written, said) in LETTERS {
        text = text.replace(written, said);
    }
    let letters: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < letters.len() {
        if letters[at] == 'R' && letters.get(at + 1) == Some(&'$') {
            let mut start = at + 2;
            while letters.get(start).is_some_and(|letter| letter.is_whitespace()) {
                start += 1;
            }
            if let Some((reais, centavos, end)) = amount(&letters, start) {
                out.push_str(&money(reais, centavos));
                at = end;
                continue;
            }
        }
        if letters[at].is_ascii_digit() {
            let (number, end) = digits(&letters, at);
            if let Some((date, end)) = date(&letters, number, end) {
                out.push_str(&date);
                at = end;
            } else {
                out.push_str(&in_words(number));
                // "10x" is how installments are written.
                let installments = letters.get(end) == Some(&'x')
                    && !letters.get(end + 1).is_some_and(|letter| letter.is_alphanumeric());
                if installments {
                    out.push_str(if number == 1 { " vez" } else { " vezes" });
                }
                at = end + usize::from(installments);
            }
            continue;
        }
        out.push(letters[at]);
        at += 1;
    }
    out
}

fn digits(letters: &[char], start: usize) -> (u64, usize) {
    let mut end = start;
    let mut number = 0_u64;
    while let Some(digit) = letters.get(end).and_then(|letter| letter.to_digit(10)) {
        number = number.saturating_mul(10).saturating_add(u64::from(digit));
        end += 1;
    }
    (number, end)
}

/// `1.234,56` from `start`: the reais, the centavos and where it ends.
fn amount(letters: &[char], start: usize) -> Option<(u64, u64, usize)> {
    if !letters.get(start)?.is_ascii_digit() {
        return None;
    }
    let (mut reais, mut end) = digits(letters, start);
    while letters.get(end) == Some(&'.') && letters.get(end + 1).is_some_and(char::is_ascii_digit) {
        let (group, after) = digits(letters, end + 1);
        reais = reais.saturating_mul(1000).saturating_add(group);
        end = after;
    }
    if letters.get(end) == Some(&',') && letters.get(end + 1).is_some_and(char::is_ascii_digit) {
        let (centavos, after) = digits(letters, end + 1);
        return Some((reais, centavos, after));
    }
    Some((reais, 0, end))
}

/// `15/11`, or `15/11/2026`, after the day: in words, and where it ends.
fn date(letters: &[char], day: u64, end: usize) -> Option<(String, usize)> {
    if letters.get(end) != Some(&'/') || !(1..=31).contains(&day) {
        return None;
    }
    let (month, after) = digits(letters, end + 1);
    if after == end + 1 || !(1..=12).contains(&month) {
        return None;
    }
    let name = MONTHS[month as usize - 1];
    let day = if day == 1 { "primeiro".to_owned() } else { small(day) };
    if letters.get(after) == Some(&'/') {
        let (year, last) = digits(letters, after + 1);
        if last > after + 1 {
            return Some((format!("{day} de {name} de {}", in_words(year)), last));
        }
    }
    Some((format!("{day} de {name}"), after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_as_people_say_them() {
        assert_eq!(in_words(0), "zero");
        assert_eq!(in_words(21), "vinte e um");
        assert_eq!(in_words(100), "cem");
        assert_eq!(in_words(101), "cento e um");
        assert_eq!(in_words(1000), "mil");
        assert_eq!(in_words(1234), "mil duzentos e trinta e quatro");
        assert_eq!(in_words(2005), "dois mil e cinco");
        assert_eq!(in_words(1_000_000), "um milhão");
        assert_eq!(in_words(2_500_000), "dois milhões quinhentos mil");
    }

    #[test]
    fn money_is_read_in_reais_and_centavos() {
        assert_eq!(
            spoken("Você tem R$ 1.234,56 nas contas."),
            "Você tem mil duzentos e trinta e quatro reais e cinquenta e seis centavos nas contas."
        );
        assert_eq!(spoken("R$ 1,00"), "um real");
        assert_eq!(spoken("R$ 0,50"), "cinquenta centavos");
        assert_eq!(spoken("R$ 50,00"), "cinquenta reais");
        assert_eq!(spoken("R$ 1.000.000,00"), "um milhão de reais");
    }

    #[test]
    fn dates_and_plain_numbers_are_in_words() {
        assert_eq!(spoken("Vence em 15/11."), "Vence em quinze de novembro.");
        assert_eq!(spoken("Em 01/02/2026"), "Em primeiro de fevereiro de dois mil e vinte e seis");
        assert_eq!(spoken("Dia 3, em 10x"), "Dia três, em dez vezes");
    }

    #[test]
    fn letter_names_are_said_letter_by_letter() {
        assert_eq!(spoken("Em PDF ou CSV"), "Em pê dê éfe ou cê ésse vê");
    }

    #[test]
    fn what_is_not_a_number_stays() {
        assert_eq!(spoken("Sem números, R$ sozinho."), "Sem números, R$ sozinho.");
        assert_eq!(spoken("99/99"), "noventa e nove/noventa e nove");
    }
}
