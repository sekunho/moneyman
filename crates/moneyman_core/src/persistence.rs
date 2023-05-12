use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{vtab::csvtab, Connection, Row};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rusty_money::{
    iso::{self, Currency},
    Exchange, ExchangeRate, Money,
};

struct Neighbors<'c> {
    prev_rates: Vec<ExchangeRate<'c, Currency>>,
    prev_date: NaiveDate,
    next_rates: Vec<ExchangeRate<'c, Currency>>,
    next_date: NaiveDate,
    missing_date: NaiveDate,
}

/// Finds the rates of the given currencies to one EUR on a given date. This
/// will ignore EUR.
pub(crate) fn find_rates<'c>(
    conn: &Connection,
    currencies: Vec<&'c Currency>,
    on: NaiveDate,
) -> Result<Vec<ExchangeRate<'c, Currency>>, rusqlite::Error> {
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
        .prepare(format!("SELECT Date, {selectable_columns} FROM rates WHERE date = ?1").as_ref())
        .expect("oh no");

    stmt.query_row([on.to_string()], |row| {
        row_to_exchange_rates(row, currencies.as_slice())
    })
}

pub(crate) enum FallbackRateError {
    Db(rusqlite::Error),
    Interpolation(InterpolationError),
}

impl From<rusqlite::Error> for FallbackRateError {
    fn from(err: rusqlite::Error) -> Self {
        FallbackRateError::Db(err)
    }
}

/// Like `find_rates` but uses linear interpolation to fill in the missing
/// rates as long as the requested date is not out of bounds. It is considered
/// out of bounds if it predates the earliest possible date, or exceeds the
/// latest row.
pub(crate) fn find_rates_with_fallback<'c>(
    conn: &Connection,
    currencies: Vec<&'c Currency>,
    on: NaiveDate,
) -> Result<Vec<ExchangeRate<'c, Currency>>, FallbackRateError> {
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
        .prepare(format!("SELECT Date, {selectable_columns} FROM rates WHERE date = ?1").as_ref())
        .expect("oh no");

    match stmt.query_row([on.to_string()], |row| {
        row_to_exchange_rates(row, currencies.as_slice())
    }) {
        Ok(currs) => Ok(currs),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let neighbors = fetch_neighboring_rates(conn, currencies.as_slice(), on)?;
            interpolate_rates(currencies.as_slice(), neighbors)
                .map_err(FallbackRateError::Interpolation)
        }
        Err(e) => Err(FallbackRateError::Db(e)),
    }
}

// Fetches the neighboring rates (previous and next) of the missing date.
fn fetch_neighboring_rates<'c>(
    conn: &Connection,
    currencies: &[&'c Currency],
    on: NaiveDate,
) -> Result<Neighbors<'c>, rusqlite::Error> {
    let selectable_columns = currencies
        .iter()
        .map(|c| c.iso_alpha_code.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let mut prev_neighbor_stmt = conn.prepare(
        format!(
            "SELECT Date, {selectable_columns} FROM rates WHERE Date < ?1 ORDER BY Date DESC LIMIT 1"
        )
        .as_ref(),
    )?;

    let mut next_neighbor_stmt = conn.prepare(
        format!("SELECT Date, {selectable_columns} FROM rates WHERE Date > ?1 ORDER BY Date ASC LIMIT 1")
            .as_ref(),
    )?;

    let (prev_date, prev_rates) = prev_neighbor_stmt.query_row([on.to_string()], |row| {
        let row = dbg!(row);
        row_to_exchange_rates(row, currencies).and_then(|rates| {
            let date = NaiveDate::parse_from_str(row.get::<usize, String>(0)?.as_str(), "%Y-%m-%d")
                .expect("not a date oh no");
            Ok((date, rates))
        })
    })?;
    let (next_date, next_rates) = next_neighbor_stmt.query_row([on.to_string()], |row| {
        let row = dbg!(row);
        row_to_exchange_rates(row, currencies).and_then(|rates| {
            let date = NaiveDate::parse_from_str(row.get::<usize, String>(0)?.as_str(), "%Y-%m-%d")
                .expect("not a date oh no");
            Ok((date, rates))
        })
    })?;

    Ok(Neighbors {
        prev_rates,
        prev_date,
        next_rates,
        next_date,
        missing_date: on,
    })
}

pub(crate) enum InterpolationError {
    MissingRate,
    SameCurrency,
}

fn interpolate_rates<'c>(
    currencies: &[&'c Currency],
    neighbors: Neighbors<'c>,
) -> Result<Vec<ExchangeRate<'c, Currency>>, InterpolationError> {
    let prev_date_exchange =
        neighbors
            .prev_rates
            .iter()
            .fold(Exchange::new(), |mut exchange, rate| {
                exchange.set_rate(rate);
                exchange
            });

    let next_date_exchange =
        neighbors
            .next_rates
            .iter()
            .fold(Exchange::new(), |mut exchange, rate| {
                exchange.set_rate(rate);
                exchange
            });

    currencies
        .iter()
        .fold(Vec::new(), |mut exchange_rates, currency| {
            let prev_date_rate = prev_date_exchange
                .get_rate(currency, iso::EUR)
                .ok_or(InterpolationError::MissingRate);
            let next_date_rate = next_date_exchange
                .get_rate(currency, iso::EUR)
                .ok_or(InterpolationError::MissingRate);

            match (prev_date_rate, next_date_rate) {
                (Ok(prev_date_rate), Ok(next_date_rate)) => {
                    let y1 = dec!(1)
                        / *prev_date_rate
                            .convert(Money::from_decimal(dec!(1), currency))
                            .unwrap()
                            .amount();
                    let y2 = dec!(1)
                        / *next_date_rate
                            .convert(Money::from_decimal(dec!(1), currency))
                            .unwrap()
                            .amount();
                    let x1 = dbg!(Decimal::new(
                        neighbors
                            .prev_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0
                    ));
                    let x3 = dbg!(Decimal::new(
                        neighbors
                            .missing_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0
                    ));
                    let x2 = dbg!(Decimal::new(
                        neighbors
                            .next_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0
                    ));

                    let slope = (y2 - y1) / (x2 - x1);
                    let y3 = y1 + slope * (x3 - x1);
                    let from_eur_rate = ExchangeRate::new(iso::EUR, currency, y3)
                        .map_err(|_| InterpolationError::SameCurrency);
                    let to_eur_rate = ExchangeRate::new(*currency, iso::EUR, dec!(1) / y3)
                        .map_err(|_| InterpolationError::SameCurrency);

                    exchange_rates.push(from_eur_rate);
                    exchange_rates.push(to_eur_rate);

                    exchange_rates
                }
                (Ok(_), Err(_)) => {
                    exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
                (Err(_), Ok(_)) => {
                    exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
                (Err(_), Err(_)) => {
                    exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
            }
        })
        .into_iter()
        .collect()
}

/// Parses a row into bidirectional exchange rates for all of the given
/// currencies.
fn row_to_exchange_rates<'c>(
    row: &Row,
    currencies: &[&'c Currency],
) -> Result<Vec<ExchangeRate<'c, Currency>>, rusqlite::Error> {
    let currs: Result<Vec<ExchangeRate<Currency>>, rusqlite::Error> = currencies
        .iter()
        .enumerate()
        .fold(Vec::new(), |mut rates, (index, currency)| {
            // Plus one because we want to ignore the date in this case.
            match row.get::<usize, String>(index + 1) {
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
}

/// Seeds the DB with the history of exchange rates
pub(crate) fn seed_db(conn: &Connection, data_dir: &Path) -> Result<(), rusqlite::Error> {
    let csv_path = data_dir.join("eurofxref-hist.csv");

    copy_from_csv(conn, &csv_path).and_then(|_| clean_up_na(conn))
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

/// Creates a virtual table `vrates` from the CSV
fn copy_from_csv(conn: &Connection, csv_path: &Path) -> Result<(), rusqlite::Error> {
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
        csv_path.to_str().expect("expected a UTF-8 path")
    );

    conn.execute_batch(script.as_str())
}

/// Sets rows with "N/A" to actual NULL values
fn clean_up_na(conn: &Connection) -> Result<(), rusqlite::Error> {
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
