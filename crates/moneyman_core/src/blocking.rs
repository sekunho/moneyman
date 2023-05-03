use std::path::Path;

use bytes::Bytes;
use reqwest::blocking::*;
use reqwest::header::CONTENT_TYPE;
use reqwest::Error;

// TODO: Refactor this mess cause this is only for experimenting
/// Downloads and saves the latest forex history
pub fn download_latest_history() {
    let url = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-hist.zip";
    let client = Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/zip");
    let res: Result<Response, Error> = client.send();
    let content: Bytes = res.expect("huh").bytes().expect("bytes");

    let reader = std::io::Cursor::new(content.as_ref());
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let path = Path::new(".");
    let _ = zip.extract(path);
}
