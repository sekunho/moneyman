use chrono::NaiveDate;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

fn main() {
    let _ = moneyman_core::sync_ecb_history();

    let from = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(2023, 5, 4).expect("ok date");

    // Convert 6,500.00 USD to EUR
    let _ = moneyman_core::convert_on_date(from, iso::EUR, date);

    let from = Money::from_decimal(dec!(1000), iso::EUR);

    // Convert 1,000.00 EUR to JPY
    let _ = moneyman_core::convert_on_date(from, iso::JPY, date);
}
