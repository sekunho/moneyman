use std::path::Path;

use bytes::Bytes;
use reqwest::blocking::*;
use reqwest::header::CONTENT_TYPE;
use rusqlite::{vtab::csvtab, Connection};

#[derive(Debug)]
pub enum Error {
    HttpError(reqwest::Error),
    ZipError(zip::result::ZipError),
    DbError(rusqlite::Error),
    IoError(std::io::Error),
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

/// Syncs the currency exchange history from the ECB
pub fn sync_ecb_history() -> Result<(), Error> {
    download_latest_history()?;
    setup_db()?;

    Ok(())
}

// TODO: Refactor to /var/lib/moneyman
const DEFAULT_DATA_DIR: &str = "/home/sekun/.moneyman/";

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
fn download_latest_history() -> Result<(), Error> {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";
    let client = Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/zip");

    let res: Response = client.send()?;
    let content: Bytes = res.bytes()?;
    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader)?;
    let path = Path::new(DEFAULT_DATA_DIR);

    if !path.exists() {
        std::fs::create_dir(DEFAULT_DATA_DIR)?;
        zip.extract(path)?;
    } else {
        zip.extract(path)?;
    }

    Ok(())
}

/// Sets up an SQLite database with the exchange rate history
fn setup_db() -> Result<(), rusqlite::Error> {
    let conn = Connection::open("eurofxref-hist.db3")?;
    csvtab::load_module(&conn)?;

    let script = "
        BEGIN;

        DROP TABLE IF EXISTS rates;
        DROP TABLE IF EXISTS vrates;

        CREATE VIRTUAL TABLE vrates
            USING csv
                ( filename=/home/sekun/.moneyman/eurofxref-hist.csv
                , header=yes
                , columns=42
                , schema='
                    CREATE TABLE rates
                        ( date DATE
                        , usd  DECIMAL(19,4)
                        , jpy  DECIMAL(19,4)
                        , bgn  DECIMAL(19,4)
                        , cyp  DECIMAL(19,4)
                        , czk  DECIMAL(19,4)
                        , dkk  DECIMAL(19,4)
                        , eek  DECIMAL(19,4)
                        , gbp  DECIMAL(19,4)
                        , huf  DECIMAL(19,4)
                        , ltl  DECIMAL(19,4)
                        , lvl  DECIMAL(19,4)
                        , mtl  DECIMAL(19,4)
                        , pln  DECIMAL(19,4)
                        , rol  DECIMAL(19,4)
                        , ron  DECIMAL(19,4)
                        , sek  DECIMAL(19,4)
                        , sit  DECIMAL(19,4)
                        , skk  DECIMAL(19,4)
                        , chf  DECIMAL(19,4)
                        , isk  DECIMAL(19,4)
                        , nok  DECIMAL(19,4)
                        , hrk  DECIMAL(19,4)
                        , rub  DECIMAL(19,4)
                        , trl  DECIMAL(19,4)
                        , try  DECIMAL(19,4)
                        , aud  DECIMAL(19,4)
                        , brl  DECIMAL(19,4)
                        , cad  DECIMAL(19,4)
                        , cny  DECIMAL(19,4)
                        , hkd  DECIMAL(19,4)
                        , idr  DECIMAL(19,4)
                        , ils  DECIMAL(19,4)
                        , inr  DECIMAL(19,4)
                        , krw  DECIMAL(19,4)
                        , mxn  DECIMAL(19,4)
                        , myr  DECIMAL(19,4)
                        , nzd  DECIMAL(19,4)
                        , php  DECIMAL(19,4)
                        , sgd  DECIMAL(19,4)
                        , thb  DECIMAL(19,4)
                        , zar  DECIMAL(19,4)
                        )
                  '
                );

        CREATE TABLE rates AS SELECT * FROM vrates;

        DROP TABLE vrates;

        COMMIT;
    ";

    conn.execute_batch(script)?;

    Ok(())
}