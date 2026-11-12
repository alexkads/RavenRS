//! Dates as `YYYY-MM-DD` text, which compare in the right order as they are. The clock stays outside,
//! so the checks work the same in tests and in the browser.

/// Whether a text is a date that exists in the calendar.
pub(crate) fn data_valida(texto: &str) -> bool {
    let bytes = texto.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let numero = |faixa: std::ops::Range<usize>| {
        let parte = texto.get(faixa)?;
        if parte.bytes().all(|b| b.is_ascii_digit()) { parte.parse::<u32>().ok() } else { None }
    };
    let (Some(ano), Some(mes), Some(dia)) = (numero(0..4), numero(5..7), numero(8..10)) else {
        return false;
    };
    let bissexto = ano % 4 == 0 && (ano % 100 != 0 || ano % 400 == 0);
    let dias = match mes {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if bissexto => 29,
        2 => 28,
        _ => return false,
    };
    (1..=dias).contains(&dia)
}

/// A count of days since 1970-01-01 as a date.
#[cfg(any(test, not(target_arch = "wasm32")))]
fn data_de_dias(dias: i64) -> String {
    // The civil-from-days algorithm, on 400-year eras.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + i64::from(mes <= 2);
    format!("{ano:04}-{mes:02}-{dia:02}")
}

/// Today's date in UTC, from the system clock.
#[cfg(not(target_arch = "wasm32"))]
pub fn hoje_utc() -> String {
    let segundos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duracao| duracao.as_secs());
    data_de_dias((segundos / 86_400) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_dates_pass() {
        for boa in ["2026-11-05", "2024-02-29", "2000-02-29", "2026-12-31"] {
            assert!(data_valida(boa), "{boa}");
        }
        for ruim in [
            "",
            "2026-13-01",
            "2026-02-29",
            "1900-02-29",
            "2026-1-05",
            "2026/11/05",
            "abcd-ef-gh",
            "2026-00-10",
        ] {
            assert!(!data_valida(ruim), "{ruim}");
        }
    }

    #[test]
    fn days_become_the_right_date() {
        assert_eq!(data_de_dias(0), "1970-01-01");
        assert_eq!(data_de_dias(19_782), "2024-02-29");
        assert_eq!(data_de_dias(11_016), "2000-02-29");
        assert!(data_valida(&hoje_utc()));
    }
}
