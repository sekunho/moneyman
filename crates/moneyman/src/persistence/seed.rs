use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{vtab::csvtab, Connection};
use rust_decimal_macros::dec;
use rusty_money::{iso, Exchange, Money};

use crate::persistence::{self, fallback::fetch_neighboring_rates};

/// Seeds the DB with the history of exchange rates
pub(crate) fn seed_db(conn: &Connection, data_dir: &Path) -> Result<(), rusqlite::Error> {
    let csv_path = data_dir.join("eurofxref-hist.csv");
    let interpolation_start_date = copy_from_csv(conn, &csv_path)?;
    clean_up_na(conn)?;
    precompute_interpolated_rates(conn, interpolation_start_date)
}

/// Creates a virtual table `vrates` from the CSV
fn copy_from_csv(conn: &Connection, csv_path: &Path) -> Result<NaiveDate, rusqlite::Error> {
    csvtab::load_module(conn)?;

    let latest_date_script = "
        SELECT Date
            FROM rates
            ORDER BY Date DESC
            LIMIT 1
    ";

    let latest_entry = conn
        .prepare_cached(latest_date_script)
        .and_then(|mut stmt| stmt.query_row((), |row| row.get::<usize, NaiveDate>(0)));

    match latest_entry {
        Ok(latest_date) => {
            let script = format!(
                "
                BEGIN;
                    DROP TABLE IF EXISTS vrates;

                    CREATE VIRTUAL TABLE vrates
                        USING csv
                            ( filename={}
                            , header=yes
                            );

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
                latest_date.succ_opt().unwrap()
            );

            conn.execute_batch(script.as_str())?;
            Ok(latest_date)
        }
        Err(err @ rusqlite::Error::QueryReturnedNoRows)
        | Err(err @ rusqlite::Error::SqliteFailure(_, _)) => {
            if let rusqlite::Error::SqliteFailure(error1, Some(err_str)) = err {
                match err_str.as_str() {
                    "no such table: rates" => {
                        let script = format!(
                            "
                            BEGIN;
                                DROP TABLE IF EXISTS vrates;
                                DROP TABLE IF EXISTS rates;

                                CREATE VIRTUAL TABLE vrates
                                    USING csv
                                        ( filename={}
                                        , header=yes
                                        );

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

                        conn.execute_batch(script.as_str())?;
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

fn precompute_interpolated_rates(
    conn: &Connection,
    start_date: NaiveDate,
) -> Result<(), rusqlite::Error> {
    let currencies = [
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

    let selectable_columns = currencies.map(|c| c.iso_alpha_code).join(", ");
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
            let neighbors = fetch_neighboring_rates(conn, &currencies, date)?;

            // FIXME: Need to find a way to get rid of this `.expect()`
            let rates = persistence::fallback::interpolate_rates(&currencies, neighbors)
                .expect("Unable to interpolate rates");

            let exchange = rates.iter().fold(Exchange::new(), |mut exchange, rate| {
                exchange.set_rate(rate);
                exchange
            });

            let currency_values_str = currencies
                .iter()
                .map(|currency| {
                    let rate = exchange
                        .get_rate(iso::EUR, currency)
                        .and_then(|rate| rate.convert(Money::from_decimal(dec!(1), iso::EUR)).ok())
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
