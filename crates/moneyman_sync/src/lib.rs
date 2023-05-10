use std::path::PathBuf;

use bytes::Bytes;
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("oh no")]
    MoneymanCoreError(moneyman_core::Error),
    #[error("failed to fetch historical ECB data")]
    HttpError(reqwest::Error),
    #[error("failed to unzip ECB archive")]
    ZipError(zip::result::ZipError),
}

impl From<moneyman_core::Error> for Error {
    fn from(err: moneyman_core::Error) -> Self {
        Error::MoneymanCoreError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::MoneymanCoreError(moneyman_core::Error::IoError(err))
    }
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

const ECB_HISTORY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";

/// Syncs the local data files with the currency exchange history from the ECB.
pub fn sync_ecb_history(data_dir: &PathBuf) -> Result<(), Error> {
    if !data_dir.exists() {
        std::fs::create_dir(data_dir)?;
    }

    download_latest_history(data_dir)?;
    moneyman_core::setup_db(data_dir)?;

    Ok(())
}

/// Downloads the latest ECB historical data, and unzips it to `data_dir`
fn download_latest_history(data_dir: &PathBuf) -> Result<(), Error> {
    let client = Client::new()
        .get(ECB_HISTORY_URL)
        .header(CONTENT_TYPE, "application/zip");

    let res: Response = client.send()?;
    let content: Bytes = res.bytes()?;
    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader)?;

    zip.extract(data_dir)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use rand::distributions::{Alphanumeric, DistString};

    use super::*;

    #[test]
    fn it_syncs_with_ecb_history() {
        let rand_str = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
        let data_dir = PathBuf::new()
            .join("/tmp")
            .join(format!("moneyman_{}", rand_str));

        std::fs::create_dir(&data_dir).expect("failed to create test directory");

        assert_eq!((), sync_ecb_history(&data_dir).unwrap());
        assert!(data_dir.join("eurofxref-hist.csv").exists());
        assert!(data_dir.join("eurofxref-hist.db3").exists());
    }
}
