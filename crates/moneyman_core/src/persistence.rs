use chrono::NaiveDate;
use rusqlite::{Connection, vtab::csvtab};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rusty_money::{iso::{Currency, self}, ExchangeRate};

use crate::error::Error;

/// Finds the rates of the given currencies to one EUR. This will ignore EUR.
pub(crate) fn find_rates_of_currencies(
    currencies: Vec<&Currency>,
    on: NaiveDate,
) -> Result<Vec<ExchangeRate<Currency>>, Error> {
    let conn = Connection::open("eurofxref-hist.db3").expect("failed conn");
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
        let rate: String = row.get(0).expect("failed to get row column");
        // FIXME: This can fail to parse because ECB doesn't have the
        // exchange rates of all of its listed currencies.
        let rate = Decimal::from_str_exact(rate.as_ref()).expect("not decimal");

        let rates: Result<Vec<_>, _> = currencies
            .into_iter()
            .fold(Vec::new(), |mut acc, currency| {
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
pub fn setup_db() -> Result<(), rusqlite::Error> {
    let conn = Connection::open("eurofxref-hist.db3")?;

    seed_db(&conn)?;
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
fn seed_db(conn: &Connection) -> Result<(), rusqlite::Error> {
    csvtab::load_module(conn)?;

    // FIXME: Remove file path hardcoding
    let script = "
        BEGIN;

        DROP TABLE IF EXISTS rates;
        DROP TABLE IF EXISTS vrates;

        CREATE VIRTUAL TABLE vrates
            USING csv
                ( filename=/home/sekun/.moneyman/eurofxref-hist.csv
                , header=yes
                );

        CREATE TABLE rates AS SELECT * FROM vrates;

        DROP TABLE vrates;

        COMMIT;
    ";

    conn.execute_batch(script)
}
