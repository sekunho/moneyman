use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use rusty_money::{
    iso::{self, Currency},
    Exchange, ExchangeRate, Money,
};

use super::exchange_rate::row_to_exchange_rates;

/// Any error that may happen when trying to interpolate rates from its
/// neighboring dates.
#[derive(Debug)]
pub(crate) enum InterpolationError {
    /// If a rate is missing, and thus cannot complete the interpolation
    MissingRate,
    /// If the currency being converted is the same. In this case, I don't
    /// think it's ever possible but is here because of `rusty_money`.
    SameCurrency,
}

/// Represents the previous, and next neighboring dates, with their
/// respective rates, of the missing date to be interpolated.
#[derive(Debug)]
pub(crate) struct Neighbors<'c> {
    /// The nearest previous dates' OTHER_CURRENCIES to EUR rates
    pub prev_rates: Vec<ExchangeRate<'c, Currency>>,
    /// The nearest previous date to the date being interpolated
    pub prev_date: NaiveDate,
    /// The nearest next dates' OTHER_CURRENCIES to EUR rates
    pub next_rates: Vec<ExchangeRate<'c, Currency>>,
    /// The nearest next date to the date being interpolated
    pub next_date: NaiveDate,
    /// The date being interpolated
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
            "
            SELECT Date, {selectable_columns}
                FROM rates
                WHERE Date < ?1
                    AND Interpolated = false
                ORDER BY Date DESC LIMIT 1
        "
        )
        .as_ref(),
    )?;

    let mut next_neighbor_stmt = conn.prepare(
        format!(
            "
            SELECT Date, {selectable_columns}
                FROM rates
                WHERE Date > ?1
                    AND Interpolated = false
                ORDER BY Date ASC
                LIMIT 1
            "
        )
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
                    let y1 = Decimal::from(1)
                        / *prev_date_rate
                            .convert(Money::from_decimal(Decimal::from(1), *currency))
                            .unwrap()
                            .amount();
                    let y2 = Decimal::from(1)
                        / *next_date_rate
                            .convert(Money::from_decimal(Decimal::from(1), *currency))
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
                    let to_eur_rate = ExchangeRate::new(*currency, iso::EUR, Decimal::from(1) / y3)
                        .map_err(|_| InterpolationError::SameCurrency);

                    exchange_rates.push(from_eur_rate);
                    exchange_rates.push(to_eur_rate);

                    exchange_rates
                }
                // NOTE: It's fine to ignore the errors since missing rates are
                // considered `null` in SQLite. Plus, these are for currencies
                // that don't exist anymore, or only recently existed so it
                // doesn't make much sense to interpolate anyway.
                (Err(InterpolationError::MissingRate), _)
                | (_, Err(InterpolationError::MissingRate)) => exchange_rates,
                _ => panic!("This was not supposed to happen I think"),
            }
        })
        .into_iter()
        .collect()
}
