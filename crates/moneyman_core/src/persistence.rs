use chrono::NaiveDate;
use rusqlite::Connection;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rusty_money::{
    iso::{self, Currency},
    ExchangeRate,
};

/// Finds the rates of the given currencies to one EUR. This will ignore EUR.
pub(crate) fn find_rates_of_currencies<'c>(
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
