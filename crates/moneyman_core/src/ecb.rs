use std::path::PathBuf;

use bytes::Bytes;
use reqwest::{
    blocking::{Client, Response},
    header::CONTENT_TYPE,
};

use crate::{persistence, Error};

/// Syncs the currency exchange history from the ECB.
pub fn sync_ecb_history() -> Result<PathBuf, Error> {
    let data_dir = dirs::home_dir().and_then(|mut home_dir| {
        home_dir.push(".moneyman");

        Some(home_dir)
    });

    match data_dir {
        Some(data_dir) => {
            if !data_dir.exists() {
                std::fs::create_dir(data_dir.clone())?;
            }

            download_latest_history(data_dir.clone())?;
            persistence::setup_db(data_dir.clone())?;

            Ok(data_dir)
        }

        None => Err(Error::NoHomeDirectory),
    }
}

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
fn download_latest_history(data_dir: PathBuf) -> Result<(), Error> {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";
    let client = Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/zip");

    let res: Response = client.send()?;
    let content: Bytes = res.bytes()?;
    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader)?;

    zip.extract(data_dir)?;

    Ok(())
}
