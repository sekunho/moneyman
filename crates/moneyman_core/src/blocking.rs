use std::path::PathBuf;

use bytes::Bytes;
use chrono::NaiveDate;
use reqwest::blocking::*;
use reqwest::header::CONTENT_TYPE;
use rusqlite::{vtab::csvtab, Connection};
use rust_decimal::Decimal;

use rust_decimal_macros::dec;
use rusty_money::{
    iso::{self, Currency},
    Exchange, ExchangeRate, Money, MoneyError,
};

#[derive(Debug)]
pub enum Error {
    HttpError(reqwest::Error),
    ZipError(zip::result::ZipError),
    DbError(rusqlite::Error),
    IoError(std::io::Error),
    MoneyError(MoneyError),
    RateNotFound,
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::HttpError(err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::ZipError(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::DbError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<MoneyError> for Error {
    fn from(err: MoneyError) -> Self {
        Error::MoneyError(err)
    }
}

pub fn convert_on_date<'a>(
    from_amount: Money<'a, Currency>,
    to: &'a Currency,
    on: NaiveDate,
) -> Result<Money<'a, Currency>, Error> {
    let from_currency = from_amount.currency();

    match (from_currency, to) {
        (from, to) if from == to => Ok(from_amount),
        (from @ iso::EUR, to) | (from, to @ iso::EUR) => {
            println!("EUR involved");
            let currencies = match to {
                iso::EUR => Vec::from([from]),
                _ => Vec::from([to]),
            };
            let rates = find_rates_of_currencies(dbg!(currencies), on)?;
            let mut exchange = Exchange::new();

            rates.iter().for_each(|rate| exchange.set_rate(rate));

            let rate = match to {
                iso::EUR => exchange.get_rate(from, iso::EUR).ok_or(Error::RateNotFound),
                _ => exchange.get_rate(iso::EUR, to).ok_or(Error::RateNotFound),
            };
            let to_money = rate?.convert(from_amount)?;

            Ok(Money::from_decimal(*(to_money.amount()), to))
        }
        _ => todo!(),
    }
}

/// Finds the rates of the given currencies to one EUR. This will ignore EUR.
fn find_rates_of_currencies(
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

    rates.map_err(|e| Error::MoneyError(e))
}

/// Syncs the currency exchange history from the ECB
pub fn sync_ecb_history() -> Result<(), Error> {
    let dir = PathBuf::from(DEFAULT_DATA_DIR);
    download_latest_history(dir.clone())?;
    setup_db()?;

    Ok(())
}

// FIXME: Remove file path hardcoding (maybe use `/var/lib/moneyman`?)
const DEFAULT_DATA_DIR: &str = "/home/sekun/.moneyman/";

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
fn download_latest_history(dir: PathBuf) -> Result<(), Error> {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";
    let client = Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/zip");

    let res: Response = client.send()?;
    let content: Bytes = res.bytes()?;
    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader)?;

    if !dir.exists() {
        std::fs::create_dir(dir.clone())?;
        zip.extract(dir)?;
    } else {
        zip.extract(dir)?;
    }

    Ok(())
}

/// Sets up an SQLite database with the exchange rate history
fn setup_db() -> Result<(), rusqlite::Error> {
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
