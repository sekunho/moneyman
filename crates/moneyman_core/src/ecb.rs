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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use rusty_money::{iso, Money};

    use super::super::convert_on_date;
    use super::*;

    #[test]
    fn it_syncs_with_ecb_history() {
        let data_dir = std::env::temp_dir();

        assert_eq!((), sync_ecb_history(&data_dir).unwrap());
        assert!(data_dir.join("eurofxref-hist.csv").exists());
        assert!(data_dir.join("eurofxref-hist.db3").exists());

        let amount_in_eur = Money::from_decimal(dec!(1000), iso::EUR);
        let date = NaiveDate::from_ymd_opt(2023, 05, 04).unwrap();
        let amount_in_usd = convert_on_date(&data_dir, amount_in_eur, iso::USD, date).unwrap();
        let expected_amount = Money::from_decimal(dec!(1000) * dec!(1.1074), iso::USD);

        assert_eq!(expected_amount, amount_in_usd);
    }
}
