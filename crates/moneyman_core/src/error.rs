#[derive(Debug)]
pub enum Error {
    HttpError(reqwest::Error),
    ZipError(zip::result::ZipError),
    DbError(rusqlite::Error),
    IoError(std::io::Error),
    MoneyError(rusty_money::MoneyError),
    RateNotFound,
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
