use chrono::NaiveDate;
use rusty_money::Exchange;
use rusty_money::{
    iso::{self, Currency},
    Money,
};

pub use crate::ecb::sync_ecb_history;
pub use crate::error::Error;

pub(crate) mod ecb;
pub(crate) mod error;
pub(crate) mod persistence;

/// Converts money to a given currency.
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
            let rates = persistence::find_rates_of_currencies(dbg!(currencies), on)?;
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
