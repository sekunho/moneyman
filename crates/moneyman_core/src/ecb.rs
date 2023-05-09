use std::path::PathBuf;

use bytes::Bytes;
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
};

use crate::{persistence, Error};

const ECB_HISTORY_URL: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";

/// Syncs the local data files with the currency exchange history from the ECB.
pub fn sync_ecb_history(data_dir: &PathBuf) -> Result<(), Error> {
    if !data_dir.exists() {
        std::fs::create_dir(data_dir)?;
    }

    download_latest_history(data_dir)?;
    persistence::setup_db(data_dir)?;

    Ok(())
}

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
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
