fn main() {
    let date = chrono::naive::NaiveDate::parse_from_str("2022-05-02", "%Y-%m-%d").unwrap();
    // println!("Hello, world!");
    // moneyman_core::blocking::download_latest_history();
    // let history = moneyman_core::history::History::read("./eurofxref-hist.csv");

    dbg!(moneyman_core::blocking::sync_ecb_history());

    // moneyman_core::convert()

    // history.get_rate_on_date(
    //     &rusty_money::iso::USD,
    //     &rusty_money::iso::EUR,
    //     date,
    // )
}
