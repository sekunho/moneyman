use std::path::PathBuf;

use bytes::Bytes;
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
};
use thiserror::Error;

const ECB_HISTORY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";

#[derive(Debug, Error)]
pub(crate) enum DownloadError {
    #[error("failed to unzip archive")]
    Unzip(zip::result::ZipError),
    #[error("failed to download archive")]
    Http(reqwest::Error),
}

impl From<reqwest::Error> for DownloadError {
    fn from(err: reqwest::Error) -> Self {
        DownloadError::Http(err)
    }
}

impl From<zip::result::ZipError> for DownloadError {
    fn from(err: zip::result::ZipError) -> Self {
        DownloadError::Unzip(err)
    }
}

/// Downloads the latest ECB historical data, and unzips it to `data_dir`
pub(crate) fn download_latest_history(data_dir: &PathBuf) -> Result<(), DownloadError> {
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

        assert_eq!((), download_latest_history(&data_dir).unwrap());
        assert!(data_dir.join("eurofxref-hist.csv").exists());
        assert!(data_dir.join("eurofxref-hist.db3").exists());
    }
}
