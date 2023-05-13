use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rusty_money::{
    iso::{self, Currency},
    Exchange, ExchangeRate, Money,
};

use super::exchange_rate::row_to_exchange_rates;

pub(crate) struct Neighbors<'c> {
    pub prev_rates: Vec<ExchangeRate<'c, Currency>>,
    pub prev_date: NaiveDate,
    pub next_rates: Vec<ExchangeRate<'c, Currency>>,
    pub next_date: NaiveDate,
    pub missing_date: NaiveDate,
}

// Fetches the neighboring rates (previous and next) of the missing date.
pub(crate) fn fetch_neighboring_rates<'c>(
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
            "SELECT Date, {selectable_columns} FROM rates WHERE Date < ?1 AND Interpolated = false ORDER BY Date DESC LIMIT 1"
        )
        .as_ref(),
    )?;

    let mut next_neighbor_stmt = conn.prepare(
        format!("SELECT Date, {selectable_columns} FROM rates WHERE Date > ?1 AND Interpolated = false ORDER BY Date ASC LIMIT 1")
            .as_ref(),
    )?;

    let (prev_date, prev_rates) = prev_neighbor_stmt.query_row([on.to_string()], |row| {
        row_to_exchange_rates(row, currencies).and_then(|rates| {
            let date = NaiveDate::parse_from_str(row.get::<usize, String>(0)?.as_str(), "%Y-%m-%d")
                .expect("not a date oh no");
            Ok((date, rates))
        })
    })?;
    let (next_date, next_rates) = next_neighbor_stmt.query_row([on.to_string()], |row| {
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

#[derive(Debug)]
pub(crate) enum InterpolationError {
    MissingRate,
    SameCurrency,
}

pub(crate) fn interpolate_rates<'c>(
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
                    let x1 = Decimal::new(
                        neighbors
                            .prev_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0,
                    );
                    let x3 = Decimal::new(
                        neighbors
                            .missing_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0,
                    );
                    let x2 = Decimal::new(
                        neighbors
                            .next_date
                            .signed_duration_since(NaiveDate::from_ymd_opt(1999, 1, 4).unwrap())
                            .num_days(),
                        0,
                    );

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
                    // exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
                (Err(_), Ok(_)) => {
                    // exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
                (Err(_), Err(_)) => {
                    // exchange_rates.push(Err(InterpolationError::MissingRate));
                    exchange_rates
                }
            }
        })
        .into_iter()
        .collect()
}
