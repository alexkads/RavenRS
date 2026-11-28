//! Numbers, money, dates and times written in words, in Brazilian Portuguese, since a voice model
//! misreads digits or skips them. [`phonemes_pt_br`](crate::phonemes) reads text through this
//! first, and an app that hands text to another voice can call [`spoken_pt_br`] itself.

/// Names a voice would read as a word, or misread.
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

/// `number` in words, as far as billions: "mil duzentos e trinta e quatro".
pub fn number_in_words_pt_br(number: u64) -> String {
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

/// `number` counting something feminine, such as hours: "uma", "duas", "vinte e duas".
fn feminine(number: u64) -> String {
    let words = number_in_words_pt_br(number);
    if number % 100 == 11 || number % 100 == 12 {
        return words;
    }
    match number % 10 {
        1 => format!("{}uma", words.strip_suffix("um").unwrap_or(&words)),
        2 => format!("{}duas", words.strip_suffix("dois").unwrap_or(&words)),
        _ => words,
    }
}

fn money(reais: u64, centavos: u64) -> String {
    let reais_words = match reais {
        0 => None,
        1 => Some("um real".to_owned()),
        _ if reais.is_multiple_of(1_000_000) => Some(format!("{} de reais", number_in_words_pt_br(reais))),
        _ => Some(format!("{} reais", number_in_words_pt_br(reais))),
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

/// How `text` should be read aloud, with digits in words: money (`R$ 1.234,56`), percentages
/// (`12,5%`), dates (`15/12`, `15/12/2026`), times (`14h30`, `14:30`), installments (`10x`),
/// negative numbers, thousands (`1.234`) and decimals (`3,75`). What is not a number stays as
/// written.
///
/// ```
/// use ravenrs_ai::spoken_pt_br;
///
/// assert_eq!(spoken_pt_br("R$ 1.234,56"), "mil duzentos e trinta e quatro reais e cinquenta e seis centavos");
/// assert_eq!(spoken_pt_br("Às 14h30, 15% off"), "Às quatorze horas e trinta, quinze por cento off");
/// ```
pub fn spoken_pt_br(text: &str) -> String {
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
        // A minus sign right before a number, not a dash between words or digits.
        let after_word = at > 0 && letters[at - 1].is_alphanumeric();
        if matches!(letters[at], '-' | '−')
            && !after_word
            && letters.get(at + 1).is_some_and(char::is_ascii_digit)
        {
            out.push_str("menos ");
            at += 1;
            continue;
        }
        if letters[at].is_ascii_digit() && !after_word {
            let (said, end) = number(&letters, at);
            out.push_str(&said);
            at = end;
            continue;
        }
        out.push(letters[at]);
        at += 1;
    }
    out
}

/// The number that starts at `at`, in words, and where it ends.
fn number(letters: &[char], at: usize) -> (String, usize) {
    let (whole, end) = digits(letters, at);
    if let Some(said) = date(letters, whole, end) {
        return said;
    }
    if let Some(said) = time(letters, whole, end) {
        return said;
    }
    let (whole, end) = thousands(letters, whole, end);
    let mut said = number_in_words_pt_br(whole);
    let mut end = end;
    if letters.get(end) == Some(&',') && letters.get(end + 1).is_some_and(char::is_ascii_digit) {
        let (_, after) = digits(letters, end + 1);
        // "3,05" is "três vírgula zero cinco": the zeros in front are said.
        let fraction: String = letters[end + 1..after].iter().collect();
        let zeros = fraction.chars().take_while(|digit| *digit == '0').count().min(fraction.len() - 1);
        let rest: u64 = fraction[zeros..].parse().unwrap_or(0);
        said.push_str(" vírgula");
        for _ in 0..zeros {
            said.push_str(" zero");
        }
        said.push(' ');
        said.push_str(&number_in_words_pt_br(rest));
        end = after;
    }
    let mut space = end;
    while letters.get(space) == Some(&' ') {
        space += 1;
    }
    if letters.get(space) == Some(&'%') {
        said.push_str(" por cento");
        return (said, space + 1);
    }
    // "10x" is how installments are written.
    let installments = letters.get(end) == Some(&'x')
        && !letters.get(end + 1).is_some_and(|letter| letter.is_alphanumeric());
    if installments {
        said.push_str(if whole == 1 { " vez" } else { " vezes" });
        end += 1;
    } else if letters.get(end).is_some_and(|letter| letter.is_alphabetic()) {
        // "10kg" is "dez kg": a unit glued to the number is read on its own.
        said.push(' ');
    }
    (said, end)
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

/// `1.234.567` after its first group: groups of three digits after a dot are thousands.
fn thousands(letters: &[char], mut number: u64, mut end: usize) -> (u64, usize) {
    if end - thousands_start(letters, end) > 3 {
        return (number, end);
    }
    loop {
        let group = letters.get(end + 1..end + 4);
        let is_group = letters.get(end) == Some(&'.')
            && group.is_some_and(|group| group.iter().all(char::is_ascii_digit))
            && !letters.get(end + 4).is_some_and(char::is_ascii_digit);
        if !is_group {
            return (number, end);
        }
        let (group, after) = digits(letters, end + 1);
        number = number.saturating_mul(1000).saturating_add(group);
        end = after;
    }
}

/// Where the run of digits that ends at `end` starts.
fn thousands_start(letters: &[char], end: usize) -> usize {
    let mut start = end;
    while start > 0 && letters[start - 1].is_ascii_digit() {
        start -= 1;
    }
    start
}

/// `1.234,56` from `start`: the reais, the centavos and where it ends.
fn amount(letters: &[char], start: usize) -> Option<(u64, u64, usize)> {
    if !letters.get(start)?.is_ascii_digit() {
        return None;
    }
    let (reais, end) = digits(letters, start);
    let (reais, end) = thousands(letters, reais, end);
    if letters.get(end) == Some(&',') && letters.get(end + 1).is_some_and(char::is_ascii_digit) {
        let (centavos, after) = digits(letters, end + 1);
        // "R$ 1,5" is fifty centavos, and a third digit is past the centavo.
        let centavos = match after - end - 1 {
            1 => centavos * 10,
            2 => centavos,
            places => centavos / 10_u64.pow(places as u32 - 2),
        };
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
            return Some((format!("{day} de {name} de {}", number_in_words_pt_br(year)), last));
        }
    }
    Some((format!("{day} de {name}"), after))
}

/// `14h`, `14h30` or `14:30`, after the hour: in words, and where it ends.
fn time(letters: &[char], hour: u64, end: usize) -> Option<(String, usize)> {
    if hour > 23 || end - thousands_start(letters, end) > 2 {
        return None;
    }
    let mark = *letters.get(end)?;
    let (minute, after) = match mark {
        'h' => {
            let (minute, after) = digits(letters, end + 1);
            if after == end + 1 {
                // "14h", but not "14horas" or "2hz".
                if letters.get(end + 1).is_some_and(|letter| letter.is_alphanumeric()) {
                    return None;
                }
                (0, after)
            } else {
                (minute, after)
            }
        }
        ':' => {
            let (minute, after) = digits(letters, end + 1);
            if after != end + 3 {
                return None;
            }
            (minute, after)
        }
        _ => return None,
    };
    if minute > 59 || (mark == 'h' && after > end + 1 && after != end + 3) {
        return None;
    }
    let hours = match hour {
        0 => "zero hora".to_owned(),
        1 => "uma hora".to_owned(),
        _ => format!("{} horas", feminine(hour)),
    };
    if minute == 0 {
        return Some((hours, after));
    }
    Some((format!("{hours} e {}", small(minute)), after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_as_people_say_them() {
        let words = number_in_words_pt_br;
        assert_eq!(words(0), "zero");
        assert_eq!(words(21), "vinte e um");
        assert_eq!(words(100), "cem");
        assert_eq!(words(101), "cento e um");
        assert_eq!(words(1000), "mil");
        assert_eq!(words(1234), "mil duzentos e trinta e quatro");
        assert_eq!(words(2005), "dois mil e cinco");
        assert_eq!(words(1_000_000), "um milhão");
        assert_eq!(words(2_500_000), "dois milhões quinhentos mil");
        assert_eq!(feminine(1), "uma");
        assert_eq!(feminine(22), "vinte e duas");
        assert_eq!(feminine(12), "doze");
    }

    #[test]
    fn money_is_read_in_reais_and_centavos() {
        assert_eq!(
            spoken_pt_br("Você tem R$ 1.234,56 nas contas."),
            "Você tem mil duzentos e trinta e quatro reais e cinquenta e seis centavos nas contas."
        );
        assert_eq!(spoken_pt_br("R$ 1,00"), "um real");
        assert_eq!(spoken_pt_br("R$ 0,50"), "cinquenta centavos");
        assert_eq!(spoken_pt_br("R$ 1,5"), "um real e cinquenta centavos");
        assert_eq!(spoken_pt_br("R$ 50,00"), "cinquenta reais");
        assert_eq!(spoken_pt_br("R$ 1.000.000,00"), "um milhão de reais");
        assert_eq!(spoken_pt_br("R$12"), "doze reais");
    }

    #[test]
    fn dates_and_plain_numbers_are_in_words() {
        assert_eq!(spoken_pt_br("Vence em 15/11."), "Vence em quinze de novembro.");
        assert_eq!(spoken_pt_br("Em 01/02/2026"), "Em primeiro de fevereiro de dois mil e vinte e seis");
        assert_eq!(spoken_pt_br("Dia 3, em 10x"), "Dia três, em dez vezes");
        assert_eq!(spoken_pt_br("São 1.234 pessoas"), "São mil duzentos e trinta e quatro pessoas");
    }

    #[test]
    fn decimals_percentages_and_signs_are_said() {
        assert_eq!(spoken_pt_br("3,75"), "três vírgula setenta e cinco");
        assert_eq!(spoken_pt_br("3,05"), "três vírgula zero cinco");
        assert_eq!(spoken_pt_br("15%"), "quinze por cento");
        assert_eq!(spoken_pt_br("12,5 %"), "doze vírgula cinco por cento");
        assert_eq!(spoken_pt_br("Caiu -2%."), "Caiu menos dois por cento.");
        assert_eq!(spoken_pt_br("de 2 a 5"), "de dois a cinco");
        assert_eq!(spoken_pt_br("2-3"), "dois-três");
    }

    #[test]
    fn times_are_said_in_hours_and_minutes() {
        assert_eq!(spoken_pt_br("às 14h"), "às quatorze horas");
        assert_eq!(spoken_pt_br("às 14h30"), "às quatorze horas e trinta");
        assert_eq!(spoken_pt_br("às 9:05"), "às nove horas e cinco");
        assert_eq!(spoken_pt_br("1h"), "uma hora");
        assert_eq!(spoken_pt_br("22:00"), "vinte e duas horas");
        assert_eq!(spoken_pt_br("0h15"), "zero hora e quinze");
    }

    #[test]
    fn letter_names_are_said_letter_by_letter() {
        assert_eq!(spoken_pt_br("Em PDF ou CSV"), "Em pê dê éfe ou cê ésse vê");
    }

    #[test]
    fn what_is_not_a_number_stays() {
        assert_eq!(spoken_pt_br("Sem números, R$ sozinho."), "Sem números, R$ sozinho.");
        assert_eq!(spoken_pt_br("99/99"), "noventa e nove/noventa e nove");
        assert_eq!(spoken_pt_br("25:99"), "vinte e cinco:noventa e nove");
        assert_eq!(spoken_pt_br("mp3 e h264"), "mp3 e h264");
        assert_eq!(spoken_pt_br("2hz"), "dois hz");
        assert_eq!(spoken_pt_br("sem dígitos"), "sem dígitos");
    }

    #[test]
    fn what_was_said_says_the_same_again() {
        for text in ["R$ 1.234,56 em 10x às 14h30, dia 15/11, 12,5%", "-3 e 1.000"] {
            let once = spoken_pt_br(text);
            assert_eq!(spoken_pt_br(&once), once);
        }
    }
}
