//! The indicator engine of StockChart (#426), with no GUI: a catalog of indicators computed over
//! a close series, each output the same length as the input with `NaN` where the indicator has no
//! value yet (its warm-up), so a series lines up with the candles by index.
//!
//! The calculations are `fin-primitives`' batch kernels, matched to TA-Lib and pandas-ta:
//!
//! | indicator | formula | first value at index |
//! |---|---|---|
//! | [`Indicator::Sma`] | mean of the last `period` closes | `period - 1` |
//! | [`Indicator::Ema`] | `alpha = 2 / (period + 1)`, seeded with the SMA of the first `period` closes | `period - 1` |
//! | [`Indicator::Rsi`] | `100 - 100 / (1 + avg_gain / avg_loss)`, Wilder smoothing (`alpha = 1 / period`) | `period` |
//! | [`Indicator::Macd`] | `EMA(fast) - EMA(slow)`, signal `EMA(signal)` of it, histogram the difference | `slow - 1`, signal `slow + signal - 2` |
//! | [`Indicator::Bollinger`] | `SMA(period) ± k` population standard deviations | `period - 1` |
//!
//! The catalog grows by adding a variant: [`Indicator::compute`] returns named lines, so a panel
//! draws any indicator the same way.

use fin_primitives::signals::batch_f64 as ta;

/// An indicator and its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Indicator {
    /// Simple moving average.
    Sma {
        /// Window length.
        period: usize,
    },
    /// Exponential moving average, SMA-seeded.
    Ema {
        /// Window length.
        period: usize,
    },
    /// Relative Strength Index, 0 to 100.
    Rsi {
        /// Window length (14 is the usual one).
        period: usize,
    },
    /// Moving Average Convergence Divergence.
    Macd {
        /// Fast EMA length (12).
        fast: usize,
        /// Slow EMA length (26).
        slow: usize,
        /// Signal EMA length (9).
        signal: usize,
    },
    /// Bollinger Bands.
    Bollinger {
        /// Window length (20).
        period: usize,
        /// Width in standard deviations (2.0).
        k: f64,
    },
}

/// One line an indicator draws.
#[derive(Debug, Clone, PartialEq)]
pub struct IndicatorLine {
    /// The line's name: `"sma"`, `"macd"`, `"signal"`, `"upper"`...
    pub name: &'static str,
    /// One value per input close, `NaN` during the warm-up.
    pub values: Vec<f64>,
}

impl Indicator {
    /// The catalog's first set with their usual parameters.
    pub fn defaults() -> [Indicator; 5] {
        [
            Indicator::Sma { period: 20 },
            Indicator::Ema { period: 20 },
            Indicator::Rsi { period: 14 },
            Indicator::Macd { fast: 12, slow: 26, signal: 9 },
            Indicator::Bollinger { period: 20, k: 2.0 },
        ]
    }

    /// A short identifier for the catalog and saved configurations.
    pub fn id(&self) -> &'static str {
        match self {
            Indicator::Sma { .. } => "sma",
            Indicator::Ema { .. } => "ema",
            Indicator::Rsi { .. } => "rsi",
            Indicator::Macd { .. } => "macd",
            Indicator::Bollinger { .. } => "bollinger",
        }
    }

    /// Whether it is drawn over the prices (`true`) or in a panel of its own.
    pub fn overlays_price(&self) -> bool {
        matches!(self, Indicator::Sma { .. } | Indicator::Ema { .. } | Indicator::Bollinger { .. })
    }

    /// The indicator's lines over `close`, each as long as `close`.
    pub fn compute(&self, close: &[f64]) -> Vec<IndicatorLine> {
        let line = |name, values| IndicatorLine { name, values };
        match *self {
            Indicator::Sma { period } => vec![line("sma", ta::sma(close, period))],
            Indicator::Ema { period } => vec![line("ema", ta::ema(close, period))],
            Indicator::Rsi { period } => vec![line("rsi", ta::rsi(close, period))],
            Indicator::Macd { fast, slow, signal } => {
                let (macd, sig, hist) = ta::macd(close, fast, slow, signal);
                vec![line("macd", macd), line("signal", sig), line("histogram", hist)]
            }
            Indicator::Bollinger { period, k } => {
                let (lower, mid, upper) = ta::bbands(close, period, k, 0);
                vec![line("lower", lower), line("middle", mid), line("upper", upper)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close() -> Vec<f64> {
        (1..=40).map(|i| 100.0 + (i as f64 * 0.7).sin() * 5.0 + i as f64 * 0.25).collect()
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn sma_and_ema_reference_values() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let sma = &Indicator::Sma { period: 3 }.compute(&x)[0].values;
        assert!(sma[0].is_nan() && sma[1].is_nan());
        assert_eq!(&sma[2..], &[2.0, 3.0, 4.0]);
        // Seeded with SMA(3) = 2, then alpha = 0.5: 3, 4.
        let ema = &Indicator::Ema { period: 3 }.compute(&x)[0].values;
        assert!(ema[1].is_nan());
        assert_eq!(&ema[2..], &[2.0, 3.0, 4.0]);
    }

    #[test]
    fn rsi_reference_values() {
        // Only gains: 100. Gains 1 and losses 1 alternating: 50 at the first value.
        let up: Vec<f64> = (0..20).map(f64::from).collect();
        let rsi = &Indicator::Rsi { period: 14 }.compute(&up)[0].values;
        assert!(rsi[13].is_nan());
        assert!(near(rsi[14], 100.0));
        let zigzag: Vec<f64> = (0..15).map(|i| if i % 2 == 0 { 10.0 } else { 11.0 }).collect();
        let rsi = &Indicator::Rsi { period: 14 }.compute(&zigzag)[0].values;
        assert!(near(rsi[14], 50.0));
    }

    #[test]
    fn macd_histogram_is_macd_minus_signal() {
        let lines = Indicator::Macd { fast: 12, slow: 26, signal: 9 }.compute(&close());
        let (macd, sig, hist) = (&lines[0].values, &lines[1].values, &lines[2].values);
        assert!(macd[24].is_nan() && !macd[25].is_nan());
        assert!(sig[32].is_nan() && !sig[33].is_nan());
        for i in 33..40 {
            assert!(near(hist[i], macd[i] - sig[i]));
        }
    }

    #[test]
    fn bollinger_reference_values() {
        // [1, 2, 3]: mean 2, population sd sqrt(2/3).
        let lines = Indicator::Bollinger { period: 3, k: 2.0 }.compute(&[1.0, 2.0, 3.0]);
        let sd = (2.0f64 / 3.0).sqrt();
        assert!(near(lines[0].values[2], 2.0 - 2.0 * sd));
        assert!(near(lines[1].values[2], 2.0));
        assert!(near(lines[2].values[2], 2.0 + 2.0 * sd));
    }

    #[test]
    fn every_line_lines_up_with_the_closes() {
        let x = close();
        for indicator in Indicator::defaults() {
            for line in indicator.compute(&x) {
                assert_eq!(line.values.len(), x.len(), "{} {}", indicator.id(), line.name);
            }
        }
        assert!(Indicator::Bollinger { period: 20, k: 2.0 }.overlays_price());
        assert!(!Indicator::Rsi { period: 14 }.overlays_price());
    }
}
