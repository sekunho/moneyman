use chrono::NaiveDate;
use rusqlite::{Connection, Row};
use rust_decimal::Decimal;
use rusty_money::{
    iso::{self, Currency},
    ExchangeRate,
};

pub(crate) fn get_latest_date(conn: &Connection) -> Result<NaiveDate, rusqlite::Error> {
    let mut stmt = conn.prepare_cached("SELECT Date FROM rates ORDER BY Date DESC LIMIT 1")?;

    stmt.query_row((), |row| row.get::<usize, NaiveDate>(0))
}

/// Finds the rates of the given currencies to one EUR on a given date. This
/// will ignore EUR.
pub(crate) fn find_rates<'c>(
    conn: &Connection,
    currencies: &[&'c Currency],
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
        .prepare(
            format!(
                "
                SELECT Date, {selectable_columns}
                    FROM rates
                    WHERE Date = ?1
                        AND Interpolated = false
                "
            )
            .as_ref(),
        )
        .expect("oh no");

    stmt.query_row([on.to_string()], |row| {
        row_to_exchange_rates(row, currencies)
    })
}

/// Like `find_rates` but uses linear interpolation to fill in the missing
/// rates as long as the requested date is not out of bounds. It is considered
/// out of bounds if it predates the earliest possible date, or exceeds the
/// latest row.
pub(crate) fn find_rates_with_fallback<'c>(
    conn: &Connection,
    currencies: &[&'c Currency],
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
        .prepare(format!("SELECT Date, {selectable_columns} FROM rates WHERE Date = ?1").as_ref())
        .expect("oh no");

    stmt.query_row([on.to_string()], |row| {
        row_to_exchange_rates(row, currencies)
    })
}

/// Parses a row into bidirectional exchange rates for all of the given
/// currencies.
pub(crate) fn row_to_exchange_rates<'c>(
    row: &Row,
    currencies: &[&'c Currency],
) -> Result<Vec<ExchangeRate<'c, Currency>>, rusqlite::Error> {
    let currs: Result<Vec<ExchangeRate<Currency>>, rusqlite::Error> = currencies
        .iter()
        .enumerate()
        .fold(Vec::new(), |mut rates, (index, currency)| {
            // Plus one because we want to ignore the date in this case.
            match row.get::<usize, Option<String>>(index + 1) {
                Ok(Some(rate)) => {
                    let (to_eur, from_eur) = parse_rate(currency, rate);
                    rates.push(Ok(to_eur));
                    rates.push(Ok(from_eur));

                    rates
                }
                Ok(None) => rates,
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

/// Parses a currency rate into bidirectional exchange rates
pub(crate) fn parse_rate(
    currency: &Currency,
    rate: String,
) -> (ExchangeRate<Currency>, ExchangeRate<Currency>) {
    let rate: Decimal =
        Decimal::from_str_exact(rate.as_ref()).expect("Rate in local DB is not a decimal");

    let to_eur = ExchangeRate::new(currency, iso::EUR, Decimal::from(1) / rate).unwrap();
    let from_eur = ExchangeRate::new(iso::EUR, currency, rate).unwrap();

    (to_eur, from_eur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_money::{iso, ExchangeRate};

    #[test]
    fn it_parses_rate_into_bidirectional_rates() {
        let (rate1, rate2) = parse_rate(iso::USD, "1.1037".to_string());
        let expected1 = ExchangeRate::new(
            iso::USD,
            iso::EUR,
            Decimal::from(1) / Decimal::from_f64_retain(1.1037).unwrap(),
        )
        .unwrap();
        let expected2 = ExchangeRate::new(
            iso::EUR,
            iso::USD,
            Decimal::from_f64_retain(1.1037).unwrap(),
        )
        .unwrap();

        assert_eq!(rate1, expected1);
        assert_eq!(rate2, expected2);
    }

    #[test]
    fn it_panics_if_rate_is_invalid_when_parsing() {
        let result = std::panic::catch_unwind(|| parse_rate(iso::USD, "1a.1037".to_string()));

        assert!(result.is_err());
    }
}
