use chrono::NaiveDate;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown data store error")]
    Db(rusqlite::Error),
    #[error("unknown IO error")]
    Io(std::io::Error),
    #[error("")]
    Money(rusty_money::MoneyError),
    #[error("rate not found: {0} has no rate for {1}")]
    RateNotFound(String, NaiveDate),
    #[error("home directory does not exist")]
    NoHomeDirectory,
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::Db(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<rusty_money::MoneyError> for Error {
    fn from(err: rusty_money::MoneyError) -> Self {
        Error::Money(err)
    }
}
