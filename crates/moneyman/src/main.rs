use chrono::NaiveDate;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

fn main() {
    let date = chrono::naive::NaiveDate::parse_from_str("2022-05-02", "%Y-%m-%d").unwrap();
    // println!("Hello, world!");
    // moneyman_core::blocking::download_latest_history();
    // let history = moneyman_core::history::History::read("./eurofxref-hist.csv");

    dbg!(moneyman_core::blocking::sync_ecb_history());

    let from = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(2023, 5, 4).expect("ok date");

    dbg!(moneyman_core::blocking::convert_on_date(from, iso::EUR, date));

    let from = Money::from_decimal(dec!(1000), iso::EUR);
    dbg!(moneyman_core::blocking::convert_on_date(from, iso::JPY, date));

    // moneyman_core::convert()

    // history.get_rate_on_date(
    //     &rusty_money::iso::USD,
    //     &rusty_money::iso::EUR,
    //     date,
    // )
}
