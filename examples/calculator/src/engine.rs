//! What a pocket calculator does, without any interface. Operations run in the order they
//! are pressed, so `2 + 3 × 4 =` is 20.

/// Digits the display takes while typing and when showing a result.
const DIGITS: usize = 12;

/// A key of the calculator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// A digit from 0 to 9.
    Digit(u8),
    /// The decimal point.
    Point,
    /// An operation between two numbers.
    Op(Op),
    /// Runs the pending operation; pressed again, repeats it.
    Equals,
    /// Turns the number on display into a percentage.
    Percent,
    /// Changes the sign of the number on display.
    Negate,
    /// Erases the last digit typed.
    Backspace,
    /// Starts over.
    Clear,
}

/// An operation between two numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
}

impl Op {
    fn apply(self, a: f64, b: f64) -> Option<f64> {
        let result = match self {
            Self::Add => a + b,
            Self::Subtract => a - b,
            Self::Multiply => a * b,
            Self::Divide if b == 0.0 => return None,
            Self::Divide => a / b,
        };
        result.is_finite().then_some(result)
    }
}

/// The calculator's memory.
#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    /// What the display shows.
    display: String,
    /// The number before the pending operation.
    stored: f64,
    pending: Option<Op>,
    /// The last operation and its right side, for `=` to repeat.
    last: Option<(Op, f64)>,
    /// The next digit starts a new number.
    fresh: bool,
    failed: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self { display: "0".into(), stored: 0.0, pending: None, last: None, fresh: true, failed: false }
    }
}

impl Engine {
    /// What the display shows.
    pub fn display(&self) -> &str {
        if self.failed { "Error" } else { &self.display }
    }

    /// The operation waiting for its right side.
    pub fn pending(&self) -> Option<Op> {
        self.pending
    }

    /// Takes a key.
    pub fn press(&mut self, press: Press) {
        if self.failed {
            // After an error, only something that starts a new number does anything.
            if !matches!(press, Press::Digit(_) | Press::Point | Press::Clear) {
                return;
            }
            *self = Self::default();
        }
        match press {
            Press::Digit(digit) => self.digit(digit),
            Press::Point => self.point(),
            Press::Op(op) => self.operation(op),
            Press::Equals => self.equals(),
            Press::Percent => self.percent(),
            Press::Negate => self.negate(),
            Press::Backspace => self.backspace(),
            Press::Clear => *self = Self::default(),
        }
    }

    fn value(&self) -> f64 {
        self.display.parse().unwrap_or(0.0)
    }

    fn digit(&mut self, digit: u8) {
        let digit = char::from(b'0' + digit.min(9));
        if self.fresh {
            self.display = digit.to_string();
            self.fresh = false;
        } else if self.display == "0" {
            self.display = digit.to_string();
        } else if self.display == "-0" {
            self.display = format!("-{digit}");
        } else if self.display.chars().filter(char::is_ascii_digit).count() < DIGITS {
            self.display.push(digit);
        }
    }

    fn point(&mut self) {
        if self.fresh {
            self.display = "0.".into();
            self.fresh = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    fn operation(&mut self, op: Op) {
        // A second operation in a row only replaces the first.
        if self.pending.is_some() && !self.fresh {
            self.run_pending();
            if self.failed {
                return;
            }
        } else if self.pending.is_none() {
            self.stored = self.value();
        }
        self.pending = Some(op);
        self.fresh = true;
    }

    fn run_pending(&mut self) {
        if let Some(op) = self.pending.take() {
            let right = self.value();
            self.last = Some((op, right));
            self.finish(op.apply(self.stored, right));
        }
    }

    fn equals(&mut self) {
        if self.pending.is_some() {
            // `5 + =` adds the 5 to itself, as pocket calculators do.
            self.run_pending();
        } else if let Some((op, right)) = self.last {
            self.finish(op.apply(self.value(), right));
        }
        self.fresh = true;
    }

    fn finish(&mut self, result: Option<f64>) {
        match result {
            Some(result) => {
                self.stored = result;
                self.display = format_number(result);
            }
            None => self.failed = true,
        }
        self.fresh = true;
    }

    fn percent(&mut self) {
        let shown = self.value();
        // `200 + 10 %` is 10% of 200; `200 × 10 %` is 10% as a fraction.
        let result = match self.pending {
            Some(Op::Add | Op::Subtract) => self.stored * shown / 100.0,
            _ => shown / 100.0,
        };
        self.display = format_number(result);
        self.fresh = false;
    }

    fn negate(&mut self) {
        match self.display.strip_prefix('-') {
            Some(rest) => self.display = rest.to_owned(),
            None => self.display.insert(0, '-'),
        }
        if self.pending.is_none() && self.fresh {
            self.stored = self.value();
        }
    }

    fn backspace(&mut self) {
        if self.fresh {
            return;
        }
        self.display.pop();
        if self.display.is_empty() || self.display == "-" {
            self.display = "0".into();
        }
    }
}

/// Shows a number the way a person expects: without the noise of binary fractions, so
/// `0.1 + 0.2` is `0.3`, and in scientific notation when it does not fit.
pub fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    let size = value.abs().log10().floor();
    if !(-6.0..DIGITS as f64).contains(&size) {
        let text = format!("{:.*e}", DIGITS - 6, value);
        let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
        return format!("{}e{exponent}", trim_zeros(mantissa));
    }
    let decimals = (DIGITS as f64 - 1.0 - size).max(0.0) as usize;
    trim_zeros(&format!("{value:.decimals$}")).to_owned()
}

fn trim_zeros(text: &str) -> &str {
    if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.') } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Presses keys written as text, and says what the display shows.
    fn run(keys: &str) -> String {
        let mut engine = Engine::default();
        for key in keys.chars() {
            engine.press(match key {
                '0'..='9' => Press::Digit(key as u8 - b'0'),
                '.' => Press::Point,
                '+' => Press::Op(Op::Add),
                '-' => Press::Op(Op::Subtract),
                '*' => Press::Op(Op::Multiply),
                '/' => Press::Op(Op::Divide),
                '=' => Press::Equals,
                '%' => Press::Percent,
                'n' => Press::Negate,
                'b' => Press::Backspace,
                'c' => Press::Clear,
                other => panic!("no key {other}"),
            });
        }
        engine.display().to_owned()
    }

    #[test]
    fn does_the_four_operations() {
        assert_eq!(run("2+3="), "5");
        assert_eq!(run("9-12="), "-3");
        assert_eq!(run("6*7="), "42");
        assert_eq!(run("1/4="), "0.25");
    }

    #[test]
    fn runs_in_the_order_pressed() {
        assert_eq!(run("2+3*4="), "20");
    }

    #[test]
    fn hides_the_noise_of_binary_fractions() {
        assert_eq!(run("0.1+0.2="), "0.3");
        assert_eq!(run("1/3="), "0.333333333333");
    }

    #[test]
    fn dividing_by_zero_is_an_error_until_a_new_number() {
        assert_eq!(run("5/0="), "Error");
        assert_eq!(run("5/0=+"), "Error");
        assert_eq!(run("5/0=7"), "7");
        assert_eq!(run("5/0=c"), "0");
    }

    #[test]
    fn equals_again_repeats_the_operation() {
        assert_eq!(run("2+3=="), "8");
        assert_eq!(run("2+3==="), "11");
    }

    #[test]
    fn a_second_operation_replaces_the_first() {
        assert_eq!(run("8+-2="), "6");
    }

    #[test]
    fn a_result_is_the_start_of_the_next_operation() {
        assert_eq!(run("2+3=*2="), "10");
    }

    #[test]
    fn a_digit_after_a_result_starts_over() {
        assert_eq!(run("2+3=7"), "7");
        assert_eq!(run("2+3=7+1="), "8");
    }

    #[test]
    fn typing_keeps_to_one_point_and_twelve_digits() {
        assert_eq!(run("1.2.3"), "1.23");
        assert_eq!(run(".5"), "0.5");
        assert_eq!(run("007"), "7");
        assert_eq!(run("12345678901234"), "123456789012");
    }

    #[test]
    fn percent_is_of_the_left_side_when_adding() {
        assert_eq!(run("200+10%="), "220");
        assert_eq!(run("200-10%="), "180");
        assert_eq!(run("200*10%="), "20");
        assert_eq!(run("50%"), "0.5");
    }

    #[test]
    fn negate_and_backspace_edit_what_is_shown() {
        assert_eq!(run("5n"), "-5");
        assert_eq!(run("5nn"), "5");
        assert_eq!(run("3n+5="), "2");
        assert_eq!(run("123b"), "12");
        assert_eq!(run("1b"), "0");
        assert_eq!(run("5nb"), "0");
        assert_eq!(run("2+3=b"), "5");
    }

    #[test]
    fn numbers_out_of_range_go_scientific_or_fail() {
        assert_eq!(run("999999999999*999999999999="), "1e24");
        assert_eq!(run("1/10000000="), "1e-7");
        assert_eq!(format_number(f64::MAX), "1.797693e308");
        let mut engine = Engine {
            stored: f64::MAX,
            pending: Some(Op::Multiply),
            display: "10".into(),
            fresh: false,
            ..Engine::default()
        };
        engine.press(Press::Equals);
        assert_eq!(engine.display(), "Error");
    }
}
