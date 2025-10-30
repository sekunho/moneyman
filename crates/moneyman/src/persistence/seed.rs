use std::path::Path;

#[cfg(feature = "chrono")]
use chrono::NaiveDate;

#[cfg(feature = "time")]
use time::{Date, Month};


use rusqlite::{types::FromSql, vtab::csvtab, Connection};
use rust_decimal::Decimal;
use rusty_money::{
    iso::{self, Currency},
    Exchange, Money,
};

use crate::persistence::{self, fallback::fetch_neighboring_rates};

const CURRENCIES: [&'static Currency; 33] = [
    iso::USD,
    iso::JPY,
    iso::BGN,
    iso::CZK,
    iso::DKK,
    iso::GBP,
    iso::HUF,
    iso::PLN,
    iso::RON,
    iso::SEK,
    iso::SKK,
    iso::CHF,
    iso::ISK,
    iso::NOK,
    iso::HRK,
    iso::RUB,
    iso::TRY,
    iso::AUD,
    iso::BRL,
    iso::CAD,
    iso::CNY,
    iso::HKD,
    iso::IDR,
    iso::ILS,
    iso::INR,
    iso::KRW,
    iso::MXN,
    iso::MYR,
    iso::NZD,
    iso::PHP,
    iso::SGD,
    iso::THB,
    iso::ZAR,
];

/// Seeds the DB with the history of exchange rates
pub(crate) fn seed_db(conn: &Connection, data_dir: &Path) -> Result<(), rusqlite::Error> {
    let csv_path = data_dir.join("eurofxref-hist.csv");
    let interpolation_start_date = copy_from_csv(conn, &csv_path)?;
    clean_up_na(conn)?;
    precompute_interpolated_rates(conn, interpolation_start_date)
}

const LATEST_DATE_SCRIPT: &str = "
    SELECT Date
        FROM rates
        ORDER BY Date DESC
        LIMIT 1
";

fn get_latest_date<T>(conn: &Connection) -> Result<T, rusqlite::Error>
where
    T: FromSql,
{
    conn.prepare_cached(LATEST_DATE_SCRIPT)
        .and_then(|mut stmt| stmt.query_row((), |row| row.get::<usize, T>(0)))
}

fn execute_copy_from_csv<T>(conn: &Connection, latest_date: T, csv_path: &Path) -> Result<(), rusqlite::Error> where T: std::fmt::Display {
    let script = format!(
        "
        BEGIN;
            DROP TABLE IF EXISTS vrates;
            CREATE VIRTUAL TABLE vrates USING csv ( filename={}, header=yes);

            INSERT INTO rates
                SELECT Date
                     , USD
                     , JPY
                     , BGN
                     , CYP
                     , CZK
                     , DKK
                     , EEK
                     , GBP
                     , HUF
                     , LTL
                     , LVL
                     , MTL
                     , PLN
                     , ROL
                     , RON
                     , SEK
                     , SIT
                     , SKK
                     , CHF
                     , ISK
                     , NOK
                     , HRK
                     , RUB
                     , TRL
                     , TRY
                     , AUD
                     , BRL
                     , CAD
                     , CNY
                     , HKD
                     , IDR
                     , ILS
                     , INR
                     , KRW
                     , MXN
                     , MYR
                     , NZD
                     , PHP
                     , SGD
                     , THB
                     , ZAR
                     , false
                    FROM vrates
                    WHERE Date >= '{}'
                    ORDER BY Date DESC;
        COMMIT;
        ",
        csv_path.to_str().expect("expected a UTF-8 path"),
        latest_date,
    );
    conn.execute_batch(script.as_str())
}

fn execute_cleanup(conn: &Connection, csv_path: &Path) -> Result<(), rusqlite::Error> {
    let script = format!(
        "
        BEGIN;
            DROP TABLE IF EXISTS vrates;
            DROP TABLE IF EXISTS rates;

            CREATE VIRTUAL TABLE vrates USING csv (filename={}, header=yes);
            CREATE TABLE rates AS SELECT * FROM vrates;

            ALTER TABLE rates ADD COLUMN Interpolated BOOLEAN;
            ALTER TABLE rates DROP COLUMN \"\";

            UPDATE rates SET Interpolated = false;

            CREATE UNIQUE INDEX date_index ON rates(Date);
            CREATE INDEX date_interpolated_index ON rates(Date, Interpolated);

            DROP TABLE vrates;
        COMMIT;
        ",
        csv_path.to_str().expect("expected a UTF-8 path")
    );

    conn.execute_batch(script.as_str())
}

/// Creates a virtual table `vrates` from the CSV
#[cfg(feature = "chrono")]
fn copy_from_csv(conn: &Connection, csv_path: &Path) -> Result<NaiveDate, rusqlite::Error> {
    csvtab::load_module(conn)?;
    let latest_entry = get_latest_date::<NaiveDate>(&conn);

    match latest_entry {
        Ok(latest_date) => {
            execute_copy_from_csv(&conn, latest_date.succ_opt().unwrap(), csv_path)?;
            Ok(latest_date)
        },
        Err(err @ rusqlite::Error::QueryReturnedNoRows)
        | Err(err @ rusqlite::Error::SqliteFailure(_, _)) => {
            if let rusqlite::Error::SqliteFailure(error1, Some(err_str)) = err {
                match err_str.as_str() {
                    "no such table: rates" => {
                        execute_cleanup(&conn, csv_path)?;
                        Ok(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                    }

                    _ => Err(rusqlite::Error::SqliteFailure(error1, Some(err_str))),
                }
            } else {
                Err(err)
            }
        }
        Err(err) => Err(err),
    }
}
#[cfg(feature = "time")]
fn copy_from_csv(conn: &Connection, csv_path: &Path) -> Result<Date, rusqlite::Error> {
    csvtab::load_module(conn)?;
    let latest_entry = get_latest_date::<Date>(&conn);

    match latest_entry {
        Ok(latest_date) => {
            execute_copy_from_csv(&conn, latest_date.next_day().unwrap(), csv_path)?;
            Ok(latest_date)
        },
        Err(err @ rusqlite::Error::QueryReturnedNoRows)
        | Err(err @ rusqlite::Error::SqliteFailure(_, _)) => {
            if let rusqlite::Error::SqliteFailure(error1, Some(err_str)) = err {
                match err_str.as_str() {
                    "no such table: rates" => {
                        execute_cleanup(&conn, csv_path)?;
                        Ok(Date::from_calendar_date(1999, Month::January, 4).unwrap())
                    }

                    _ => Err(rusqlite::Error::SqliteFailure(error1, Some(err_str))),
                }
            } else {
                Err(err)
            }
        }
        Err(err) => Err(err),
    }
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

#[cfg(feature = "chrono")]
fn precompute_interpolated_rates(
    conn: &Connection,
    start_date: NaiveDate,
) -> Result<(), rusqlite::Error> {
    let selectable_columns = CURRENCIES.map(|c| c.iso_alpha_code).join(", ");
    let mut latest_date_statement =
        conn.prepare("SELECT Date FROM rates ORDER BY Date DESC LIMIT 1")?;

    let latest_date = latest_date_statement.query_row((), |row| row.get::<usize, NaiveDate>(0))?;

    start_date
        .iter_days()
        // Skip the first date since the first date should always have a rate
        .skip(1)
        // Take until before the latest date since it also should always have
        // a rate
        .take_while(|date| *date < latest_date)
        .map(|date| {
            let neighbors = fetch_neighboring_rates(conn, &CURRENCIES, date)?;

            // FIXME: Need to find a way to get rid of this `.expect()`
            let rates = persistence::fallback::interpolate_rates(&CURRENCIES, neighbors)
                .expect("Unable to interpolate rates");

            let exchange = rates.iter().fold(Exchange::new(), |mut exchange, rate| {
                exchange.set_rate(rate);
                exchange
            });

            let currency_values_str = CURRENCIES
                .iter()
                .map(|currency| {
                    let rate = exchange
                        .get_rate(iso::EUR, currency)
                        .and_then(|rate| {
                            rate.convert(Money::from_decimal(Decimal::from(1), iso::EUR))
                                .ok()
                        })
                        .map(|money| *money.amount());

                    match rate {
                        Some(rate) => rate.to_string(),
                        None => String::from("null"),
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            let script = format!(
                "
                INSERT INTO rates(Date, Interpolated, {selectable_columns})
                    VALUES ('{}', true, {})
                    ON CONFLICT DO NOTHING
                ",
                date, currency_values_str
            );

            conn.execute_batch(script.as_str())
        })
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;

    Ok(())
}

#[cfg(feature = "time")]
fn precompute_interpolated_rates(
    conn: &Connection,
    start_date: time::Date,
) -> Result<(), rusqlite::Error> {
    let selectable_columns = CURRENCIES.map(|c| c.iso_alpha_code).join(", ");
    let mut latest_date_statement =
        conn.prepare("SELECT Date FROM rates ORDER BY Date DESC LIMIT 1")?;

    let latest_date = latest_date_statement.query_row((), |row| row.get::<usize, time::Date>(0))?;
    let mut current_date = start_date.next_day().unwrap();

    while current_date < latest_date {
        let neighbors = fetch_neighboring_rates(conn, &CURRENCIES, current_date)?;

        // FIXME: Need to find a way to get rid of this `.expect()`
        let rates = persistence::fallback::interpolate_rates(&CURRENCIES, neighbors)
            .expect("Unable to interpolate rates");

        let exchange = rates.iter().fold(Exchange::new(), |mut exchange, rate| {
            exchange.set_rate(rate);
            exchange
        });

        let currency_values_str = CURRENCIES
            .iter()
            .map(|currency| {
                let rate = exchange
                    .get_rate(iso::EUR, currency)
                    .and_then(|rate| {
                        rate.convert(Money::from_decimal(Decimal::from(1), iso::EUR))
                            .ok()
                    })
                    .map(|money| *money.amount());

                match rate {
                    Some(rate) => rate.to_string(),
                    None => String::from("null"),
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let script = format!(
            "
        INSERT INTO rates(Date, Interpolated, {selectable_columns})
            VALUES ('{}', true, {})
            ON CONFLICT DO NOTHING
        ",
            current_date, currency_values_str
        );

        conn.execute_batch(script.as_str())?;
        current_date = current_date.next_day().unwrap();
    }

    Ok(())
}
