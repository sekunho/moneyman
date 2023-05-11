use std::path::{Path, PathBuf};

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
pub(crate) fn find_rates_of_currencies<'c>(
    data_dir: &Path,
    currencies: Vec<&'c Currency>,
    on: NaiveDate,
) -> Result<Vec<ExchangeRate<'c, Currency>>, Error> {
    let db_path = data_dir.join("eurofxref-hist.db3");
    let conn = Connection::open(db_path).expect("failed conn");
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

    // FIXME: Refactor this spaghetti
    stmt.query_row([on.to_string()], |row| {
        let currs: Result<Vec<ExchangeRate<Currency>>, rusqlite::Error> = currencies
            .into_iter()
            .enumerate()
            .fold(Vec::new(), |mut rates, (index, currency)| {
                match row.get::<usize, String>(index) {
                    Ok(rate) => {
                        let (to_eur, from_eur) = parse_rate(currency, rate);
                        rates.push(Ok(to_eur));
                        rates.push(Ok(from_eur));

                        rates
                    }
                    Err(e) => {
                        rates.push(Err(e));

                        rates
                    }
                }
            })
            .into_iter()
            .collect();

        currs
    })
    .map_err(|e| match e {
        rusqlite::Error::InvalidColumnType(num, col, rusqlite::types::Type::Null) => {
            match col.as_str() {
                "Date" => Error::DbError(rusqlite::Error::InvalidColumnType(
                    num,
                    col,
                    rusqlite::types::Type::Null,
                )),
                currency_col => Error::RateNotFound(String::from(currency_col), on),
            }
        }
        e => Error::DbError(e),
    })
}

/// Parses a currency rate into bidirectional exchange rates
fn parse_rate(
    currency: &Currency,
    rate: String,
) -> (ExchangeRate<Currency>, ExchangeRate<Currency>) {
    let rate: Decimal =
        Decimal::from_str_exact(rate.as_ref()).expect("Rate in local DB is not a decimal");

    let to_eur = ExchangeRate::new(currency, iso::EUR, dec!(1) / rate).unwrap();
    let from_eur = ExchangeRate::new(iso::EUR, currency, rate).unwrap();

    (to_eur, from_eur)
}

/// Sets up an SQLite database with the exchange rate history
pub fn setup_db(data_dir: &Path) -> Result<(), Error> {
    // CSV file path
    let csv_path = data_dir.join("eurofxref-hist.csv");

    // DB file path
    let db_path = data_dir.join("eurofxref-hist.db3");
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

    let script = format!(
        "
        BEGIN;

        DROP TABLE IF EXISTS rates;
        DROP TABLE IF EXISTS vrates;

        CREATE VIRTUAL TABLE vrates
            USING csv
                ( filename={}
                , header=yes
                );

        CREATE TABLE rates AS SELECT * FROM vrates;

        CREATE UNIQUE INDEX date_index ON rates(Date);

        DROP TABLE vrates;

        COMMIT;
    ",
        csv_path.to_str().expect("Expected a UTF-8 path")
    );

    conn.execute_batch(script.as_str())
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use rusty_money::{iso, ExchangeRate};

    use super::parse_rate;

    #[test]
    fn it_parses_rate_into_bidirectional_rates() {
        let (rate1, rate2) = parse_rate(iso::USD, "1.1037".to_string());
        let expected1 = ExchangeRate::new(iso::USD, iso::EUR, dec!(1) / dec!(1.1037)).unwrap();
        let expected2 = ExchangeRate::new(iso::EUR, iso::USD, dec!(1.1037)).unwrap();

        assert_eq!(rate1, expected1);
        assert_eq!(rate2, expected2);
    }

    #[test]
    fn it_panics_if_rate_is_invalid_when_parsing() {
        let result = std::panic::catch_unwind(|| parse_rate(iso::USD, "1a.1037".to_string()));

        assert!(result.is_err());
    }
}
