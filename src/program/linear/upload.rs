//! A file put in Linear's storage: the fourth operation the program sends (`FileUpload`), which
//! says where the file goes and what its address will be, and the `PUT` of the file to that place.
//! The key goes to Linear with the first and is not sent with the second: the address of the
//! storage is signed, and it is not told to anyone (not in a log, not in an error, not to the page).

use std::io::Read;
use std::path::Path;

use serde::Deserialize;

use super::images;
use super::query::{self, Upload};
use super::{Error, Key, Transport, run};

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "fileUpload")]
    upload: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    success: bool,
    #[serde(default, rename = "uploadFile")]
    file: Option<Place>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Place {
    upload_url: String,
    asset_url: String,
    #[serde(default)]
    headers: Vec<Header>,
}

#[derive(Deserialize)]
struct Header {
    key: String,
    value: String,
}

/// What `upload` says where Linear answered with data and not with a place for the file.
const NO_PLACE: &str = "Linear did not say where to put the file";

/// A file in Linear: the address it has there, which is what a description or a comment names it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub asset: String,
}

/// Most headers Linear's place may ask the `PUT` to carry.
const HEADERS_MOST: usize = 16;

/// Puts `file`, a copy the program made (`Staged`) of `size` bytes, named `name` and told to be of
/// `content_type`, in Linear's storage: asks Linear where (one request with the key), sends the file
/// there (one `PUT`, without the key) and gives the address the file has. Nothing is sent again by
/// itself; a file that was put and not used is left in Linear's storage, where it harms nothing.
pub fn upload(
    transport: &dyn Transport,
    key: &Key,
    name: &str,
    content_type: &str,
    file: &Path,
    size: u64,
) -> Result<Sent, Error> {
    if size == 0 || size > images::FILE_MOST {
        return Err(Error::Failed(format!(
            "a file is up to {} MiB and not empty",
            images::FILE_MOST_MIB
        )));
    }
    let data: Data = run(
        transport,
        key,
        &query::FILE_UPLOAD,
        Some(Upload {
            content_type,
            filename: name,
            size,
        }),
    )?;
    let place = data
        .upload
        .filter(|payload| payload.success)
        .and_then(|payload| payload.file)
        .ok_or_else(|| Error::Failed(NO_PLACE.to_owned()))?;
    // The file is read back by the address Linear gave it, which must be one of its files'.
    if !images::is_upload(&place.asset_url) {
        return Err(Error::Failed(
            "Linear named a place for the file that the pen does not use".to_owned(),
        ));
    }
    if place.headers.len() > HEADERS_MOST {
        return Err(Error::Failed(
            "Linear asked for more headers than the pen sends".to_owned(),
        ));
    }
    let mut headers: Vec<(String, String)> = place
        .headers
        .into_iter()
        .map(|header| (header.key, header.value))
        .collect();
    for (name, value) in [
        ("Content-Type", content_type),
        ("Cache-Control", "public, max-age=31536000"),
    ] {
        headers.retain(|(had, _)| !had.eq_ignore_ascii_case(name));
        headers.push((name.to_owned(), value.to_owned()));
    }
    // (Whatever is wrong with the place is said without it: the address is signed.)
    let status = transport
        .put(&place.upload_url, &headers, file, size)
        .map_err(|error| match error {
            Error::Failed(_) => Error::Failed("the file could not be sent".to_owned()),
            other => other,
        })?;
    if !(200..300).contains(&status) {
        return Err(Error::Failed(format!(
            "the storage did not take the file (HTTP {status})"
        )));
    }
    Ok(Sent {
        asset: place.asset_url,
    })
}

/// Why a file given to be sent was not.
#[derive(Debug, PartialEq, Eq)]
pub enum NotSent {
    /// It has nothing in it.
    Empty,
    /// It is more than 100 MiB.
    TooLarge,
    /// The copy could not be read.
    Unreadable,
    /// Linear, or the storage, did not take it.
    Linear(Error),
}

/// Puts the file `file` (a copy the program made: `Staged`) named `name` in Linear's storage, of any
/// kind but not over 100 MiB, and gives the Markdown that names it: a picture (PNG, JPEG, GIF, WebP,
/// by its first bytes) as `![name](address)` and any other file as `[name](address)`. This is the
/// one way a file is sent, from the page and from the command line.
pub fn send_file(
    transport: &dyn Transport,
    key: &Key,
    name: &str,
    file: &Path,
) -> Result<String, NotSent> {
    let size = std::fs::metadata(file)
        .map_err(|_| NotSent::Unreadable)?
        .len();
    if size == 0 {
        return Err(NotSent::Empty);
    }
    if size > images::FILE_MOST {
        return Err(NotSent::TooLarge);
    }
    let mut head = Vec::new();
    std::fs::File::open(file)
        .and_then(|opened| opened.take(16).read_to_end(&mut head))
        .map_err(|_| NotSent::Unreadable)?;
    let kind = images::sniff(&head);
    let sent = upload(
        transport,
        key,
        &images::clean_name(name),
        kind.content_type,
        file,
        size,
    )
    .map_err(NotSent::Linear)?;
    Ok(images::line(name, &sent.asset, kind.image))
}
