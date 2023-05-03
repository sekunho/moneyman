fn main() {
    // let date = chrono::naive::NaiveDate::parse_from_str("2022-05-02", "%Y-%m-%d").unwrap();
    // println!("Hello, world!");
    // moneyman_core::blocking::download_latest_history();
    moneyman_core::get_rate(
        *rusty_money::iso::find("USD").unwrap(),
        *rusty_money::iso::find("USD").unwrap(),
        date,
    )
}
