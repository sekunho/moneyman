use chrono::NaiveDate;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to fetch ECB historical data")]
    HttpError(reqwest::Error),
    #[error("failed to unzip ECB historical data archive")]
    ZipError(zip::result::ZipError),
    #[error("unknown data store error")]
    DbError(rusqlite::Error),
    #[error("unknown IO error")]
    IoError(std::io::Error),
    #[error("")]
    MoneyError(rusty_money::MoneyError),
    #[error("rate not found: {0} has no rate for {1}")]
    RateNotFound(String, NaiveDate),
    #[error("home directory does not exist")]
    NoHomeDirectory,
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::HttpError(err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::ZipError(err)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Error::DbError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<rusty_money::MoneyError> for Error {
    fn from(err: rusty_money::MoneyError) -> Self {
        Error::MoneyError(err)
    }
}
