# 💱 moneyman

A crusty currency converter

## Example

```rust
use std::path::PathBuf;

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use rusty_money::{iso, Money};

fn main() {
    // Choose where to save the historical data files.
    let data_dir: PathBuf = dirs::home_dir()
        .and_then(|home_dir| Some(home_dir.join(".moneyman")))
        .expect("need a home directory");

    // Fetches the historical data from European Central Bank, and saves it
    // in the data directory.
    moneyman_sync::sync_ecb_history(&data_dir).expect("failed ze sync");

    let amount_in_usd = Money::from_decimal(dec!(6500), iso::USD);
    let date = NaiveDate::from_ymd_opt(2023, 5, 4).expect("ok date");

    // Converts 6,500.00 USD to EUR
    let actual = moneyman_core::convert_on_date(&data_dir, amount_in_usd, iso::EUR, date).unwrap();
    let expected = Money::from_decimal(dec!(5869.6044789597254831135994221), iso::EUR);

    assert_eq!(actual, expected);
}
```

## Details

`moneyman` extends on `rusty-money` as it already provides a lot of the things like
`Money`, `Currency`, `ExchangeRate`, and `Exchange`. However, it does not
provide any data to actually convert currency. For historical data, `moneyman`
uses the European Central Bank, and saves its data to its own local data store.
