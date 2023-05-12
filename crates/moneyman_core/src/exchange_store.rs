use std::path::PathBuf;

use chrono::NaiveDate;
use rusqlite::Connection;
use rusty_money::{
    iso::{self, Currency},
    Exchange, Money,
};
use thiserror::Error;

use crate::{ecb, persistence::{self, FallbackRateError}};

pub struct ExchangeStore {
    conn: Connection,
    data_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("ECB history is not present in the given data directory")]
    NoEcbHistory,
    #[error("unable to open the exchange store")]
    CouldNotRead,
    #[error("unable to complete seeding the exchange store")]
    Seed,
    #[error("failed to download currency exchange history from ECB")]
    Download,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("unable to open the exchange store")]
    CouldNotRead,
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("contains malformed data")]
    MalformedExchangeStore,
    #[error("could not find the relevant exchange rate on date {0}")]
    NoExchangeRate(NaiveDate),
    #[error("not a valid currency")]
    InvalidCurrency,
}

impl ExchangeStore {
    /// Syncs the local data store's currency exchange data with the European
    /// Central Bank.
    pub fn sync(data_dir: PathBuf) -> Result<Self, SyncError> {
        ecb::download_latest_history(&data_dir).map_err(|_| SyncError::Download)?;

        let db_path = data_dir.join("eurofxref-hist.db3");
        let conn = Connection::open(db_path).map_err(|_| SyncError::CouldNotRead)?;
        let store = ExchangeStore { conn, data_dir };

        persistence::seed_db(&store.conn, &store.data_dir).map_err(|_| SyncError::Seed)?;

        Ok(store)
    }

    /// Creates a new instance based on the existing data store. If you need
    /// to initialize a data store for the first time, hence need to sync the
    /// history with the European Central Bank, use `ExchangeStore::sync`
    /// instead.
    pub fn open(data_dir: PathBuf) -> Result<Self, InitError> {
        let db_path = data_dir.join("eurofxref-hist.db3");
        let conn = Connection::open(db_path).map_err(|_| InitError::CouldNotRead)?;

        Ok(ExchangeStore { conn, data_dir })
    }

    pub fn convert_on_date_with_fallback<'c>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        on_date: NaiveDate,
    ) -> Result<Money<'c, Currency>, ConversionError> {
        let from_currency = from_amount.currency();
        match (from_currency, to_currency) {
            (from, to) if from == to => Ok(from_amount),
            (from @ iso::EUR, to) | (from, to @ iso::EUR) => {
                let currencies = match to {
                    iso::EUR => Vec::from([from]),
                    _ => Vec::from([to]),
                };
                let rates = persistence::find_rates_with_fallback(&self.conn, currencies, on_date)
                    .map_err(|err| match err {
                        FallbackRateError::Db(rusqlite::Error::QueryReturnedNoRows)
                        | FallbackRateError::Db(rusqlite::Error::InvalidColumnType(_, _, _)) => {
                            ConversionError::NoExchangeRate(on_date)
                        }

                        _ => ConversionError::MalformedExchangeStore,
                    })?;

                let mut exchange = Exchange::new();

                rates.iter().for_each(|rate| exchange.set_rate(rate));

                let rate = match to {
                    iso::EUR => exchange.get_rate(from, iso::EUR),
                    _ => exchange.get_rate(iso::EUR, to),
                };
                let rate = rate.ok_or(ConversionError::NoExchangeRate(on_date))?;
                let to_money = rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::InvalidCurrency)?;

                Ok(Money::from_decimal(*(to_money.amount()), to))
            }
            (from, to) => {
                let currencies = Vec::from([from, to]);
                let rates =
                    persistence::find_rates(&self.conn, currencies, on_date).map_err(|err| {
                        match err {
                            rusqlite::Error::QueryReturnedNoRows => {
                                ConversionError::NoExchangeRate(on_date)
                            }
                            _ => ConversionError::MalformedExchangeStore,
                        }
                    })?;
                let mut exchange = Exchange::new();

                rates.iter().for_each(|rate| exchange.set_rate(rate));

                // Use EUR as the bridge between currencies
                let from_curr_to_eur_rate = exchange
                    .get_rate(from, iso::EUR)
                    .ok_or(ConversionError::NoExchangeRate(on_date))?;
                let eur = from_curr_to_eur_rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::InvalidCurrency)?;
                let from_eur_to_target_curr_rate = exchange
                    .get_rate(iso::EUR, to)
                    .ok_or(ConversionError::NoExchangeRate(on_date))?;
                let target_money = from_eur_to_target_curr_rate
                    .convert(eur)
                    .map_err(|_| ConversionError::InvalidCurrency)?;

                Ok(Money::from_decimal(*(target_money.amount()), to))
            }
        }
    }

    pub fn convert_on_date<'a>(
        &self,
        from_amount: Money<'a, Currency>,
        to_currency: &'a Currency,
        on_date: NaiveDate,
    ) -> Result<Money<'a, Currency>, ConversionError> {
        let from_currency = from_amount.currency();

        match (from_currency, to_currency) {
            (from, to) if from == to => Ok(from_amount),
            (from @ iso::EUR, to) | (from, to @ iso::EUR) => {
                let currencies = match to {
                    iso::EUR => Vec::from([from]),
                    _ => Vec::from([to]),
                };
                let rates =
                    persistence::find_rates(&self.conn, currencies, on_date).map_err(|err| {
                        match err {
                            rusqlite::Error::QueryReturnedNoRows => {
                                ConversionError::NoExchangeRate(on_date)
                            }
                            _ => ConversionError::MalformedExchangeStore,
                        }
                    })?;
                let mut exchange = Exchange::new();

                rates.iter().for_each(|rate| exchange.set_rate(rate));

                let rate = match to {
                    iso::EUR => exchange.get_rate(from, iso::EUR),
                    _ => exchange.get_rate(iso::EUR, to),
                };
                let rate = rate.ok_or(ConversionError::NoExchangeRate(on_date))?;
                let to_money = rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::InvalidCurrency)?;

                Ok(Money::from_decimal(*(to_money.amount()), to))
            }
            (from, to) => {
                let currencies = Vec::from([from, to]);
                let rates =
                    persistence::find_rates(&self.conn, currencies, on_date).map_err(|err| {
                        match err {
                            rusqlite::Error::QueryReturnedNoRows => {
                                ConversionError::NoExchangeRate(on_date)
                            }
                            _ => ConversionError::MalformedExchangeStore,
                        }
                    })?;
                let mut exchange = Exchange::new();

                rates.iter().for_each(|rate| exchange.set_rate(rate));

                // Use EUR as the bridge between currencies
                let from_curr_to_eur_rate = exchange
                    .get_rate(from, iso::EUR)
                    .ok_or(ConversionError::NoExchangeRate(on_date))?;
                let eur = from_curr_to_eur_rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::InvalidCurrency)?;
                let from_eur_to_target_curr_rate = exchange
                    .get_rate(iso::EUR, to)
                    .ok_or(ConversionError::NoExchangeRate(on_date))?;
                let target_money = from_eur_to_target_curr_rate
                    .convert(eur)
                    .map_err(|_| ConversionError::InvalidCurrency)?;

                Ok(Money::from_decimal(*(target_money.amount()), to))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::NaiveDate;
    use rand::distributions::{Alphanumeric, DistString};
    use rust_decimal_macros::dec;
    use rusty_money::{iso, Money};

    use crate::exchange_store::{ConversionError, ExchangeStore};

    #[test]
    fn it_syncs_with_ecb() {
        let rand_str = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let data_dir = PathBuf::new()
            .join("/tmp")
            .join(format!("moneyman_{}", rand_str));

        std::fs::create_dir(&data_dir).expect("failed to create test directory");

        ExchangeStore::sync(data_dir.clone()).unwrap();

        assert!(data_dir.join("eurofxref-hist.db3").exists());
    }

    #[test]
    /// This should succeed since there's a rate on this date
    fn it_converts_currencies_on_available_dates() {
        let data_dir = dbg!(PathBuf::new()
            .join("..")
            .join("..")
            .join("test_data")
            .join("indexed"));

        assert!(data_dir.exists());

        let store = ExchangeStore::open(data_dir).unwrap();
        let amount_in_eur = Money::from_decimal(dec!(1000), iso::EUR);
        let date = NaiveDate::from_ymd_opt(2023, 05, 04).unwrap();
        let amount_in_usd = store
            .convert_on_date(amount_in_eur, iso::USD, date)
            .unwrap();
        let expected_amount = Money::from_decimal(dec!(1000) * dec!(1.1074), iso::USD);

        assert_eq!(expected_amount, amount_in_usd);
    }

    #[test]
    /// Expect this to give an error since there's no fallback implementation
    fn it_fails_to_convert_if_no_rate_on_given_date() {
        let data_dir = dbg!(PathBuf::new()
            .join("..")
            .join("..")
            .join("test_data")
            .join("indexed"));

        assert!(data_dir.exists());

        let store = ExchangeStore::open(data_dir).unwrap();
        let amount_in_eur = Money::from_decimal(dec!(1000), iso::EUR);
        let date = NaiveDate::from_ymd_opt(2023, 05, 06).unwrap();

        match dbg!(store.convert_on_date(amount_in_eur, iso::USD, date)) {
            Ok(_) => panic!("expected to fail"),
            Err(ConversionError::NoExchangeRate { .. }) => (),
            Err(_) => panic!("expected db not to have any results, not fail cause of other cases"),
        }
    }
}
