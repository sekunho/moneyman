use std::path::PathBuf;

use chrono::NaiveDate;
use rusqlite::{vtab::csvtab, Connection};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rusty_money::{
    iso::{self, Currency},
    ExchangeRate,
};

use crate::error::Error;

/// Finds the rates of the given currencies to one EUR. This will ignore EUR.
pub(crate) fn find_rates_of_currencies(
    mut data_dir: PathBuf,
    currencies: Vec<&Currency>,
    on: NaiveDate,
) -> Result<Vec<ExchangeRate<Currency>>, Error> {
    data_dir.push("eurofxref-hist.db3");
    let conn = Connection::open(data_dir).expect("failed conn");
    let filtered_currencies: Vec<String> = currencies
        .iter()
        .filter_map(|c| {
            if *c == iso::EUR {
                None
            } else {
                Some(c.iso_alpha_code.to_string())
            }
        })
        .collect();
    let selectable_columns = filtered_currencies.join(", ");

    let mut stmt = conn
        .prepare(format!("SELECT {selectable_columns} FROM rates WHERE date = ?1").as_ref())
        .expect("oh no");

    let rates = stmt.query_row([on.to_string()], |row| {
        let rates: Result<Vec<_>, _> = currencies
            .into_iter()
            .enumerate()
            .fold(Vec::new(), |mut acc, (index, currency)| {
                let rate: String = row.get(index).expect("failed to get row column");
                // FIXME: This can fail to parse because ECB doesn't have the
                // exchange rates of all of its listed currencies.
                let rate = Decimal::from_str_exact(rate.as_ref()).expect("not decimal");

                acc.push(ExchangeRate::new(currency, iso::EUR, dec!(1) / rate));
                acc.push(ExchangeRate::new(iso::EUR, currency, rate));

                acc
            })
            // Oh no why would I do this ;(
            .into_iter()
            .collect();

        Ok(rates)
    })?;

    rates.map_err(Error::MoneyError)
}

/// Sets up an SQLite database with the exchange rate history
pub fn setup_db(mut data_dir: PathBuf) -> Result<(), rusqlite::Error> {
    // CSV file path
    let mut other_data_dir = data_dir.clone();
    other_data_dir.push("eurofxref-hist.csv");
    let csv_path = other_data_dir;

    // DB file path
    data_dir.push("eurofxref-hist.db3");
    let db_path = dbg!(data_dir);
    let conn = Connection::open(db_path)?;

    seed_db(csv_path, &conn)?;
    sqlite_and_its_dynamic_typing_what_a_good_idea_lol(&conn)?;

    Ok(())
}

/// Sets rows with "N/A" to actual NULL values
fn sqlite_and_its_dynamic_typing_what_a_good_idea_lol(
    conn: &Connection,
) -> Result<(), rusqlite::Error> {
    let currencies = [
        "USD", "JPY", "BGN", "CYP", "CZK", "DKK", "EEK", "GBP", "HUF", "LTL", "LVL", "MTL", "PLN",
        "ROL", "RON", "SEK", "SIT", "SKK", "CHF", "ISK", "NOK", "HRK", "RUB", "TRL", "TRY", "AUD",
        "BRL", "CAD", "CNY", "HKD", "IDR", "ILS", "INR", "KRW", "MXN", "MYR", "NZD", "PHP", "SGD",
        "THB", "ZAR",
    ];

    let statements = currencies
        .map(|c| format!("UPDATE rates SET {c} = null WHERE {c} = 'N/A';"))
        .join("\n");

    let statements = format!("BEGIN; \n{statements}\nCOMMIT;");
    (*conn).execute_batch(statements.as_ref())
}

/// Seeds the DB with the history of exchange rates
fn seed_db(csv_path: PathBuf, conn: &Connection) -> Result<(), rusqlite::Error> {
    csvtab::load_module(conn)?;

    // FIXME: Remove file path hardcoding
    let script = format!("
        BEGIN;

        DROP TABLE IF EXISTS rates;
        DROP TABLE IF EXISTS vrates;

        CREATE VIRTUAL TABLE vrates
            USING csv
                ( filename={}
                , header=yes
                );

        CREATE TABLE rates AS SELECT * FROM vrates;

        DROP TABLE vrates;

        COMMIT;
    ", csv_path.to_str().expect("Expected a UTF-8 path"));

    conn.execute_batch(script.as_str())
}
