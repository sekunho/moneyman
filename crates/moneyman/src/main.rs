use std::path::PathBuf;

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

fn main() {
    let data_dir: PathBuf = dirs::home_dir()
        .map(|home_dir| home_dir.join(".moneyman"))
        .expect("need a home directory");

    let store = moneyman_core::ExchangeStore::open(data_dir).expect("failed ze sync");

    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let _amount_in_eur = Money::from_decimal(dec!(1000), iso::EUR);

//     let date = NaiveDate::from_ymd_opt(2023, 5, 4).expect("ok date");

//     // Convert 6,500.00 USD to EUR
//     let _ = store.convert_on_date(amount_in_usd.clone(), iso::EUR, date);

//     // Convert 1,000.00 EUR to JPY
//     let _ = store.convert_on_date(amount_in_eur.clone(), iso::JPY, date);

//     // Convert 500.00 USD to JPY
//     let _ = store.convert_on_date(amount_in_usd.clone(), iso::JPY, date);

//     // Convert EUR to BRL on a date with no historical data
//     let date = NaiveDate::from_ymd_opt(2007, 12, 31).expect("ok date");
//     let _ = ("{}", store.convert_on_date(amount_in_eur, iso::BRL, date));

    // Convert 500.00 USD to EUR even if ECB has no record on this date
    let date = NaiveDate::from_ymd_opt(2023, 5, 6).expect("ok date");

    for _i in 1..500 {
        let _ = store.convert_on_date_with_fallback(amount_in_usd.clone(), iso::JPY, date);
    }
}
