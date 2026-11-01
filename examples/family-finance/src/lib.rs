//! A desktop app for a family's expenses and bills to pay, stored in SQLite.
//! It is also a tour of RavenRS: see `README.md` for what each screen uses.

#![allow(
    clippy::inconsistent_digit_grouping,
    reason = "amounts read as reais_cents: 4_200_00 is R$ 4.200,00"
)]

pub mod db;
pub mod filter;
pub mod model;
pub mod money;
pub mod paths;
pub mod reports;
pub mod seed;
pub mod ui;
pub mod update;

use ravenrs::prelude::Date;

/// Today on this computer's calendar. Due dates are days people live by, so
/// this is the local date and not UTC, which would be tomorrow every evening in Brazil.
pub fn today() -> Date {
    let now = jiff::Zoned::now().date();
    Date::new(i32::from(now.year()), now.month() as u8, now.day() as u8).unwrap_or_else(|| Date::from_days(0))
}
