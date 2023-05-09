use std::path::PathBuf;

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

fn main() {
    let data_dir: PathBuf = dirs::home_dir()
        .and_then(|mut home_dir| {
            home_dir.push(".moneyman");

            Some(home_dir)
        })
        .expect("need a home directory");

    moneyman_core::sync_ecb_history(&data_dir).expect("failed ze sync");

    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let amount_in_eur = Money::from_decimal(dec!(1000), iso::EUR);

    let date = NaiveDate::from_ymd_opt(2023, 5, 4).expect("ok date");

    // Convert 6,500.00 USD to EUR
    let _ = moneyman_core::convert_on_date(&data_dir, amount_in_usd.clone(), iso::EUR, date);

    // Convert 1,000.00 EUR to JPY
    let _ = moneyman_core::convert_on_date(&data_dir, amount_in_eur.clone(), iso::JPY, date);

    // Convert 500.00 USD to JPY
    let _ = dbg!(moneyman_core::convert_on_date(
        &data_dir,
        amount_in_usd,
        iso::JPY,
        date
    ));

    // Convert EUR to BRL on a date with no historical data
    let date = NaiveDate::from_ymd_opt(2007, 12, 31).expect("ok date");
    let _ = dbg!(
        "{}",
        moneyman_core::convert_on_date(&data_dir, amount_in_eur, iso::BRL, date)
    );
}
