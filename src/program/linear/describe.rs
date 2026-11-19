//! The one write to a description: files at its end. The pen writes no description (`Change`
//! has no member for one); to put a file in the description of an issue that is there it reads the
//! description whole from Linear at that moment, adds the lines of the files after a blank line,
//! and sends that and nothing else, to that one issue. A description that could not be read is not
//! written. Sent once, whatever comes back; what is done of an answer that did not come is
//! `Session::describe`'s.

use serde::Serialize;

use super::images;
use super::types::Issue;
use super::{Error, Key, Transport, fetch_issue, send_batch};

/// The `input` of the `IssueBatchUpdate` this sends: the description, and no other member (it is the
/// only way the pen makes one with a description, and nothing else makes this).
#[derive(Debug, Serialize)]
struct Description<'a> {
    description: &'a str,
}

/// Where `add_files` failed: before anything was sent (the description could not be read, or the
/// files do not fit in it), or in the write, which may or may not have been made.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing was sent: the description could not be read, so it was not written.
    Read(Error),
    /// Nothing was sent: the description with the files at its end would be over what Linear holds.
    TooLong,
    /// The write was sent, and what came back is `Error` (see `Session::describe` for what is done).
    Write(Error),
}

/// Adds the Markdown `lines` of files at the end of the description of the issue with Linear's id
/// (or identifier) `issue`, as Linear holds it now, and gives the issue as Linear says it is.
pub fn add_files(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    lines: &[String],
) -> Result<Issue, Step> {
    if lines.is_empty() {
        return Err(Step::Read(Error::Failed(
            "there is no file to add".to_owned(),
        )));
    }
    let read = fetch_issue(transport, key, issue).map_err(Step::Read)?;
    let old = read.description.clone().unwrap_or_default();
    let new = images::description_with(&old, lines);
    if new.chars().count() > images::DESCRIPTION_MOST {
        return Err(Step::TooLong);
    }
    let ids = [read.id.clone()];
    let written =
        send_batch(transport, key, &ids, Description { description: &new }).map_err(Step::Write)?;
    Ok(written.into_iter().next().unwrap_or(read))
}
