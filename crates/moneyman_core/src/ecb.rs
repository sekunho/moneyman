use std::path::PathBuf;

use bytes::Bytes;
use reqwest::{blocking::{Client, Response}, header::CONTENT_TYPE};

use crate::{Error, persistence};

/// Syncs the currency exchange history from the ECB
pub fn sync_ecb_history() -> Result<(), Error> {
    let dir = PathBuf::from(DEFAULT_DATA_DIR);
    download_latest_history(dir)?;
    persistence::setup_db()?;

    Ok(())
}

// FIXME: Remove file path hardcoding (maybe use `/var/lib/moneyman`?)
const DEFAULT_DATA_DIR: &str = "/home/sekun/.moneyman/";

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
fn download_latest_history(dir: PathBuf) -> Result<(), Error> {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";
    let client = Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/zip");

    let res: Response = client.send()?;
    let content: Bytes = res.bytes()?;
    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader)?;

    if !dir.exists() {
        std::fs::create_dir(dir.clone())?;
        zip.extract(dir)?;
    } else {
        zip.extract(dir)?;
    }

    Ok(())
}
