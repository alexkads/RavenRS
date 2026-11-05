//! Moments as plain seconds, with no clock read: tests and rules with a date stay deterministic.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// A moment, in seconds since 1970-01-01 UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instante(pub i64);

/// Days since 1970-01-01 of a civil date.
fn dias_de(ano: i64, mes: i64, dia: i64) -> i64 {
    let ano = if mes <= 2 { ano - 1 } else { ano };
    let era = ano.div_euclid(400);
    let ano_da_era = ano - era * 400;
    let mes_deslocado = (mes + 9) % 12;
    let dia_do_ano = (153 * mes_deslocado + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    era * 146_097 + dia_da_era - 719_468
}

/// The civil date of a number of days since 1970-01-01.
fn data_de(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let dia_da_era = z - era * 146_097;
    let ano_da_era = (dia_da_era - dia_da_era / 1460 + dia_da_era / 36_524 - dia_da_era / 146_096) / 365;
    let dia_do_ano = dia_da_era - (365 * ano_da_era + ano_da_era / 4 - ano_da_era / 100);
    let m = (5 * dia_do_ano + 2) / 153;
    let dia = dia_do_ano - (153 * m + 2) / 5 + 1;
    let mes = if m < 10 { m + 3 } else { m - 9 };
    let ano = ano_da_era + era * 400 + i64::from(mes <= 2);
    (ano, mes, dia)
}

fn numero(texto: &str, inicio: usize, fim: usize) -> Option<i64> {
    let parte = texto.get(inicio..fim)?;
    if parte.bytes().all(|b| b.is_ascii_digit()) { parte.parse().ok() } else { None }
}

impl Instante {
    /// A moment from date and time in UTC.
    #[must_use]
    pub fn utc(ano: i64, mes: i64, dia: i64, hora: i64, minuto: i64, segundo: i64) -> Self {
        Self(dias_de(ano, mes, dia) * 86_400 + hora * 3600 + minuto * 60 + segundo)
    }

    /// Reads `2026-11-05T14:30:00-03:00`, the form of the layout.
    #[must_use]
    pub fn de_xml(texto: &str) -> Option<Self> {
        if texto.len() != 25 {
            return None;
        }
        let base = Self::utc(
            numero(texto, 0, 4)?,
            numero(texto, 5, 7)?,
            numero(texto, 8, 10)?,
            numero(texto, 11, 13)?,
            numero(texto, 14, 16)?,
            numero(texto, 17, 19)?,
        );
        let fuso = numero(texto, 20, 22)? * 3600 + numero(texto, 23, 25)? * 60;
        match texto.as_bytes().get(19) {
            Some(b'-') => Some(Self(base.0 + fuso)),
            Some(b'+') => Some(Self(base.0 - fuso)),
            _ => None,
        }
    }

    /// Writes the moment in the form of the layout, in the -03:00 offset.
    #[must_use]
    pub fn para_xml(self) -> String {
        let local = self.0 - 3 * 3600;
        let (ano, mes, dia) = data_de(local.div_euclid(86_400));
        let resto = local.rem_euclid(86_400);
        format!(
            "{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}-03:00",
            resto / 3600,
            (resto % 3600) / 60,
            resto % 60
        )
    }

    /// The moment some time after this one.
    #[must_use]
    pub fn depois(self, tempo: Duration) -> Self {
        Self(self.0.saturating_add(i64::try_from(tempo.as_secs()).unwrap_or(i64::MAX)))
    }

    /// Date and time in UTC, as the parts an X.509 time needs.
    pub(crate) fn partes_utc(self) -> (i64, i64, i64, i64, i64, i64) {
        let (ano, mes, dia) = data_de(self.0.div_euclid(86_400));
        let resto = self.0.rem_euclid(86_400);
        (ano, mes, dia, resto / 3600, (resto % 3600) / 60, resto % 60)
    }
}

/// A clock that only moves when told to, for rules with a date and for injected latency.
#[derive(Clone, Debug)]
pub struct RelogioControlado(Arc<Mutex<Instante>>);

impl RelogioControlado {
    /// A clock standing at this moment.
    #[must_use]
    pub fn em(instante: Instante) -> Self {
        Self(Arc::new(Mutex::new(instante)))
    }

    /// The current moment.
    #[must_use]
    pub fn agora(&self) -> Instante {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Moves the clock forward.
    pub fn avancar(&self, tempo: Duration) {
        let mut agora = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *agora = agora.depois(tempo);
    }

    /// Puts the clock at this moment, even backward.
    pub fn ir_para(&self, instante: Instante) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = instante;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_form_goes_there_and_back() {
        let texto = "2026-11-05T14:30:00-03:00";
        let instante = Instante::de_xml(texto).expect("moment");
        assert_eq!(instante.para_xml(), texto);
        assert_eq!(Instante::de_xml("2026-11-05T17:30:00+00:00"), Some(instante));
        assert_eq!(instante, Instante::utc(2026, 11, 5, 17, 30, 0));
    }

    #[test]
    fn known_epochs_and_leap_days() {
        assert_eq!(Instante::utc(1970, 1, 1, 0, 0, 0).0, 0);
        assert_eq!(Instante::utc(2000, 3, 1, 0, 0, 0).0, 951_868_800);
        assert_eq!(Instante::utc(2028, 2, 29, 12, 0, 0).partes_utc(), (2028, 2, 29, 12, 0, 0));
    }

    #[test]
    fn bad_text_is_refused() {
        assert_eq!(Instante::de_xml("ontem"), None);
        assert_eq!(Instante::de_xml("2026-11-05T14:30:00Z"), None);
    }

    #[test]
    fn clock_only_moves_when_told() {
        let relogio = RelogioControlado::em(Instante(100));
        relogio.avancar(Duration::from_secs(5));
        assert_eq!(relogio.agora(), Instante(105));
        relogio.ir_para(Instante(1));
        assert_eq!(relogio.agora(), Instante(1));
    }
}
