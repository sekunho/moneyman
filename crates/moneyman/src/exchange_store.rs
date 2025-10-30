#[cfg(feature = "chrono")]
use chrono::NaiveDate;

use std::path::PathBuf;

use rusqlite::Connection;
use rusty_money::{
    iso::{self, Currency},
    Exchange, ExchangeRate, Money,
};

use crate::{ecb, persistence};

/// Represents the local data store of moneyman
pub struct ExchangeStore {
    /// The SQLite connection established
    conn: Connection,
    /// Directory of the local data store
    data_dir: PathBuf,
}

/// Possible errors that may occur when syncing the local data store with the
/// European Central Bank's history.
#[derive(Debug)]
pub enum SyncError {
    /// If the ECB history (CSV) isn't present in the same directory as the
    /// local data store.
    NoEcbHistory,
    /// Can't establish a connection with the local data store
    CouldNotRead,
    /// Failed to seed the local data store
    Seed,
    /// Unable to download the latest exchange history from the ECB
    Download,
}

/// Possible errors that may happen when attempting to read the local data store
#[derive(Debug)]
pub enum InitError {
    /// Can't establish a connection with the local data store
    CouldNotRead,
}

#[non_exhaustive]
#[derive(Debug)]
pub enum ConversionError {
    /// Unable to parse the data from the store due to it potentially having
    /// an unexpected format.
    MalformedExchangeStore,
    /// There's no record in the local data store that has the exchange rate on
    /// the given date.
    #[cfg(feature = "chrono")]
    NoExchangeRate(NaiveDate),
    #[cfg(feature = "time")]
    NoExchangeRateDate(time::Date),
    /// If the currency is not recorded by ECB, or if it doesn't exist at all
    InvalidCurrency(Currency),
    /// If they're trying to convert a currency to itself
    SameCurrency,
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::NoEcbHistory => {
                write!(f, "ECB history is not present in the given data directory")
            }
            SyncError::CouldNotRead => write!(f, "unable to open the exchange store"),
            SyncError::Seed => write!(f, "unable to complete seeding the exchange store"),
            SyncError::Download => {
                write!(f, "failed to donwload currency exchange history from ECB")
            }
        }
    }
}

impl std::error::Error for SyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::CouldNotRead => write!(f, "unable to open the exchange store"),
        }
    }
}

impl std::error::Error for InitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::MalformedExchangeStore => write!(f, "contains malformed data"),
            #[cfg(feature = "chrono")]
            ConversionError::NoExchangeRate(naive_date) => write!(f, "could not find the relevant exchange rate on date {naive_date}"),
            #[cfg(feature = "time")]
            ConversionError::NoExchangeRateDate(date) => write!(f, "could not find the relevant exchange rate on date {date}"),
            ConversionError::InvalidCurrency(currency) => write!(f, "either {currency} is not a valid currency, or it's not recorded in the European Central Bank"),
            ConversionError::SameCurrency => write!(f, "there's no need to convert anything. it's the same currency."),
        }
    }
}

impl std::error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        self.source()
    }
}

impl ExchangeStore {
    /// Syncs the local data store's currency exchange data with the European
    /// Central Bank.
    #[cfg(feature = "chrono")]
    pub fn sync(data_dir: PathBuf) -> Result<(Self, NaiveDate), SyncError> {
        ecb::download_latest_history(&data_dir).map_err(|_e| SyncError::Download)?;

        let db_path = data_dir.join("eurofxref-hist.db3");
        let conn = Connection::open(db_path).map_err(|_| SyncError::CouldNotRead)?;
        let store = ExchangeStore { conn, data_dir };

        persistence::seed::seed_db(&store.conn, &store.data_dir).map_err(|_e| SyncError::Seed)?;

        let latest_date = store.get_latest_date().ok_or(SyncError::CouldNotRead)?;

        Ok((store, latest_date))
    }

    #[cfg(feature = "time")]
    pub fn sync(data_dir: PathBuf) -> Result<(Self, time::Date), SyncError> {
        ecb::download_latest_history(&data_dir).map_err(|_e| SyncError::Download)?;

        let db_path = data_dir.join("eurofxref-hist.db3");
        let conn = Connection::open(db_path).map_err(|_| SyncError::CouldNotRead)?;
        let store = ExchangeStore { conn, data_dir };

        persistence::seed::seed_db(&store.conn, &store.data_dir).map_err(|_e| SyncError::Seed)?;

        let latest_date = store.get_latest_date().ok_or(SyncError::CouldNotRead)?;

        Ok((store, latest_date))
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

    /// This is the "generic" version of the convert function. Along with the
    /// usual data needed to convert two currencies, it also needs you to
    /// provide a closure that returns the exchange rates.
    fn convert<'c, F>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        #[cfg(feature = "chrono")] on_date: NaiveDate,
        #[cfg(feature = "time")] on_date: time::Date,
        find_rates: F,
    ) -> Result<Money<'c, Currency>, ConversionError>
    where
        F: FnOnce(Vec<&'c Currency>) -> Result<Vec<ExchangeRate<'c, Currency>>, rusqlite::Error>,
    {
        let from_currency = from_amount.currency();
        match (from_currency, to_currency) {
            (from, to) if from == to => Err(ConversionError::SameCurrency),
            // FIXME: Split OR pattern, and factor out to/from EUR conversion
            (from @ iso::EUR, to) | (from, to @ iso::EUR) => {
                let currencies = match to {
                    iso::EUR => Vec::from([from]),
                    _ => Vec::from([to]),
                };
                let non_eur_currency = if from == iso::EUR { to } else { from };
                let rates = find_rates(currencies).map_err(|err| match err {
                    rusqlite::Error::QueryReturnedNoRows => {
                        #[cfg(feature = "chrono")]
                        {
                            ConversionError::NoExchangeRate(on_date)
                        }

                        #[cfg(feature = "time")]
                        {
                            ConversionError::NoExchangeRateDate(on_date)
                        }
                    }
                    rusqlite::Error::SqlInputError { .. } => {
                        ConversionError::InvalidCurrency(*non_eur_currency)
                    }
                    _ => ConversionError::MalformedExchangeStore,
                })?;
                let exchange = rates_to_exchange(rates.as_slice());

                let rate = match to {
                    iso::EUR => exchange.get_rate(from, iso::EUR),
                    _ => exchange.get_rate(iso::EUR, to),
                };
                #[cfg(feature = "chrono")]
                let rate = rate.ok_or(ConversionError::NoExchangeRate(on_date))?;

                #[cfg(feature = "time")]
                let rate = rate.ok_or(ConversionError::NoExchangeRateDate(on_date))?;

                let to_money = rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::SameCurrency)?;

                Ok(Money::from_decimal(*(to_money.amount()), to))
            }
            // FIXME: Factor out non-EUR to non-EUR conversion
            (from, to) => {
                let currencies = Vec::from([from, to]);
                let rates = find_rates(currencies).map_err(|err| match err {
                    rusqlite::Error::QueryReturnedNoRows => {
                        #[cfg(feature = "chrono")]
                        {
                            ConversionError::NoExchangeRate(on_date)
                        }

                        #[cfg(feature = "time")]
                        {
                            ConversionError::NoExchangeRateDate(on_date)
                        }
                    }
                    rusqlite::Error::SqlInputError { msg, .. } => {
                        // I uhh.. I think this is fine?
                        let currency = msg.split(": ").nth(1).and_then(iso::find).unwrap();
                        ConversionError::InvalidCurrency(*currency)
                    }
                    _ => ConversionError::MalformedExchangeStore,
                })?;
                let exchange = rates_to_exchange(rates.as_slice());

                // Use EUR as the bridge between currencies
                let from_curr_to_eur_rate = exchange.get_rate(from, iso::EUR).ok_or({
                    #[cfg(feature = "chrono")]
                    {
                        ConversionError::NoExchangeRate(on_date)
                    }

                    #[cfg(feature = "time")]
                    {
                        ConversionError::NoExchangeRateDate(on_date)
                    }
                })?;
                let eur = from_curr_to_eur_rate
                    .convert(from_amount)
                    .map_err(|_| ConversionError::SameCurrency)?;
                let from_eur_to_target_curr_rate = exchange.get_rate(iso::EUR, to).ok_or({
                    #[cfg(feature = "chrono")]
                    {
                        ConversionError::NoExchangeRate(on_date)
                    }

                    #[cfg(feature = "time")]
                    {
                        ConversionError::NoExchangeRateDate(on_date)
                    }
                })?;
                let target_money = from_eur_to_target_curr_rate
                    .convert(eur)
                    .map_err(|_| ConversionError::SameCurrency)?;

                Ok(Money::from_decimal(*(target_money.amount()), to))
            }
        }
    }

    /// Converts currencies using a date's specific rate but it also uses
    /// interpolated values in the event that the date is not on record. If the
    /// date is out of bounds, e.g before the first record's date or after the
    /// last record's date, then it will fail to convert. To use the fallback,
    /// one must stay within the bounds of the store's dates.
    #[cfg(feature = "chrono")]
    pub fn convert_on_date_with_fallback<'c>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        on_date: NaiveDate,
    ) -> Result<Money<'c, Currency>, ConversionError> {
        let find_rates = |currencies: Vec<&'c Currency>| {
            persistence::exchange_rate::find_rates_with_fallback(
                &self.conn,
                currencies.as_slice(),
                on_date,
            )
        };

        self.convert(from_amount, to_currency, on_date, find_rates)
    }

    #[cfg(feature = "time")]
    pub fn convert_on_date_with_fallback<'c>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        on_date: time::Date,
    ) -> Result<Money<'c, Currency>, ConversionError> {
        let find_rates = |currencies: Vec<&'c Currency>| {
            persistence::exchange_rate::find_rates_with_fallback(
                &self.conn,
                currencies.as_slice(),
                on_date,
            )
        };

        self.convert(from_amount, to_currency, on_date, find_rates)
    }

    /// Converts currencies using the rate on the given date. If the requested
    /// date doesn't exist, then it'll return with the error
    /// `ConversionError::NoExchangeRate`.
    #[cfg(feature = "chrono")]
    pub fn convert_on_date<'c>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        on_date: NaiveDate,
    ) -> Result<Money<'c, Currency>, ConversionError> {
        let find_rates = |currencies: Vec<&'c Currency>| {
            persistence::exchange_rate::find_rates(&self.conn, currencies.as_slice(), on_date)
        };

        self.convert(from_amount, to_currency, on_date, find_rates)
    }

    #[cfg(feature = "time")]
    pub fn convert_on_date<'c>(
        &self,
        from_amount: Money<'c, Currency>,
        to_currency: &'c Currency,
        on_date: time::Date,
    ) -> Result<Money<'c, Currency>, ConversionError> {
        let find_rates = |currencies: Vec<&'c Currency>| {
            persistence::exchange_rate::find_rates(&self.conn, currencies.as_slice(), on_date)
        };

        self.convert(from_amount, to_currency, on_date, find_rates)
    }

    #[cfg(feature = "chrono")]
    pub fn get_latest_date(&self) -> Option<NaiveDate> {
        persistence::exchange_rate::get_latest_date::<NaiveDate>(&self.conn).ok()
    }

    #[cfg(feature = "time")]
    pub fn get_latest_date(&self) -> Option<time::Date> {
        persistence::exchange_rate::get_latest_date::<time::Date>(&self.conn).ok()
    }
}

/// Creates an `Exchange`, and sets it with all the given rates.
fn rates_to_exchange<'c>(rates: &'c [ExchangeRate<'c, Currency>]) -> Exchange<'c, Currency> {
    rates.iter().fold(Exchange::new(), |mut exchange, rate| {
        exchange.set_rate(rate);

        exchange
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rand::distributions::{Alphanumeric, DistString};
    use rust_decimal::Decimal;
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
        let amount_in_eur = Money::from_decimal(Decimal::from(1000), iso::EUR);

        #[cfg(feature = "chrono")]
        let date = chrono::NaiveDate::from_ymd_opt(2023, 5, 4).unwrap();

        #[cfg(feature = "time")]
        let date = time::Date::from_calendar_date(2023, time::Month::May, 4).unwrap();

        let amount_in_usd = store
            .convert_on_date(amount_in_eur, iso::USD, date)
            .unwrap();
        let expected_amount = dbg!(Money::from_decimal(
            Decimal::from(1000) * Decimal::from_i128_with_scale(11074, 4),
            iso::USD
        ));

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
        let amount_in_eur = Money::from_decimal(Decimal::from(1000), iso::EUR);
        #[cfg(feature = "chrono")]
        let date = chrono::NaiveDate::from_ymd_opt(2023, 5, 6).unwrap();

        #[cfg(feature = "time")]
        let date = time::Date::from_calendar_date(2023, time::Month::May, 6).unwrap();

        match dbg!(store.convert_on_date(amount_in_eur, iso::USD, date)) {
            Ok(_) => panic!("expected to fail"),
            #[cfg(feature = "chrono")]
            Err(ConversionError::NoExchangeRate { .. }) => (),
            #[cfg(feature = "time")]
            Err(ConversionError::NoExchangeRateDate { .. }) => (),
            Err(_) => panic!("expected db not to have any results, not fail cause of other cases"),
        }
    }
}
