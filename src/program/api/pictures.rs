//! The calls about files: `picture` and `release` (the pictures and PDFs of Linear the page draws,
//! given to the terminal for an issue and let go when it is not shown), `save` (a file of Linear saved
//! where the person chose) and `describe` (the files the person adds at the end of the description of
//! an issue). The files that go with a comment or a new issue are the `files` of those calls, read and
//! sent here. What happens is `pictures/`'s and `linear/`'s; this reads what the page asks and tells
//! the answer.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::issue::check_id;
use super::{Failure, call, call_stoppable};
use crate::linear::images::{self, FILE_MOST_MIB, SEND_MOST};
use crate::linear::{Error, Step};
use crate::pictures::{Document, Given, QUERY_MOST, Query, Saved, SendFailed};
use crate::session::Session;

/// A file the person gave the page, as the page names it: the handle the terminal made of it, and
/// the name of the file (words of the person's: they only name the file).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Attached {
    handle: String,
    name: String,
}

/// The most characters of a file's name that are taken.
const NAME_MOST: usize = 255;

/// The files the params name, or why they cannot be sent: at most ten, each a handle (hex digits)
/// and a name.
pub(super) fn given(files: &[Attached]) -> Result<Vec<Given>, Failure> {
    if files.len() > SEND_MOST {
        return Err(Failure::BadParams(format!("files are at most {SEND_MOST}")));
    }
    files
        .iter()
        .map(|image| {
            if image.handle.is_empty()
                || image.handle.len() > 64
                || !image.handle.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(Failure::BadParams(
                    "a file has the handle the terminal made of it".to_owned(),
                ));
            }
            if image.name.chars().count() > NAME_MOST {
                return Err(Failure::BadParams(format!(
                    "the name of a file is at most {NAME_MOST} characters"
                )));
            }
            Ok(Given {
                handle: image.handle.clone(),
                name: image.name.clone(),
            })
        })
        .collect()
}

/// Puts the files in Linear's storage and gives the lines that name them; none is sent where one
/// cannot be. `what` is what is to be done with them (`posted`, `made`, `added`).
pub(super) fn send(session: &Session, given: &[Given], what: &str) -> Result<Vec<String>, Failure> {
    if given.is_empty() {
        return Ok(Vec::new());
    }
    session
        .send_pictures(given, what)
        .map_err(|failed| match failed {
            SendFailed::Refused => Failure::Linear(Error::Refused),
            SendFailed::Words(words) => Failure::UploadFailed(words),
        })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Show {
    src: String,
    owner: String,
}

/// The longest address that is asked about.
const SRC_MOST: usize = 4096;

/// What `picture` tells: the picture or the PDF (its name for the page to draw it by, and its size:
/// in pixels, for a picture; in points, for a PDF's first page, with how many pages where it says), or
/// why it is not drawn.
#[derive(Serialize)]
#[serde(untagged)]
enum Shown {
    Drawn {
        name: String,
        width: u32,
        height: u32,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        pdf: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pages: Option<u32>,
    },
    Refused {
        refused: &'static str,
        /// What the bytes say it is (`gif`, `webp`), where they say.
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<&'static str>,
        /// The size in pixels of a picture that is too large to draw, where the header says.
        #[serde(skip_serializing_if = "Option::is_none")]
        width: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        height: Option<u32>,
    },
}

fn picture(session: &Session, params: Show) -> Result<Shown, Failure> {
    check_id(&params.owner)?;
    if params.src.is_empty() || params.src.len() > SRC_MOST {
        return Err(Failure::BadParams(format!(
            "src is the address of a file, up to {SRC_MOST} characters"
        )));
    }
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    Ok(match session.picture(&params.src, &params.owner) {
        Ok(shown) => Shown::Drawn {
            name: shown.name,
            width: shown.width,
            height: shown.height,
            pdf: shown.pdf,
            pages: shown.pages,
        },
        Err(refusal) => Shown::Refused {
            refused: refusal.why.word(),
            kind: refusal.kind,
            width: refusal.size.map(|(width, _)| width),
            height: refusal.size.map(|(_, height)| height),
        },
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    owner: String,
}

fn release(session: &Session, params: Release) -> Result<Value, Failure> {
    check_id(&params.owner)?;
    session.release(&params.owner);
    Ok(json!({}))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    src: String,
    handle: String,
}

/// What `save` tells: how many bytes were saved.
#[derive(Serialize)]
struct Size {
    size: u64,
}

fn save(session: &Session, params: Save) -> Result<Size, Failure> {
    if params.src.is_empty() || params.src.len() > SRC_MOST {
        return Err(Failure::BadParams(format!(
            "src is the address of a file, up to {SRC_MOST} characters"
        )));
    }
    if params.handle.is_empty()
        || params.handle.len() > 64
        || !params.handle.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(Failure::BadParams(
            "handle is the one the terminal made of the place the person chose".to_owned(),
        ));
    }
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    match session.save(&params.src, &params.handle) {
        Ok(size) => Ok(Size { size }),
        Err(why) => Err(Failure::SaveFailed(match why {
            Saved::NotLinear => "the file is not one of Linear's".to_owned(),
            Saved::TooLarge => format!("the file is over {FILE_MOST_MIB} MiB"),
            Saved::Unreachable => "Linear did not give the file".to_owned(),
            Saved::Refused => "the place did not take the file".to_owned(),
            Saved::NoView => "this terminal cannot save files".to_owned(),
        })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Describe {
    id: String,
    files: Vec<Attached>,
}

/// What a call that added files to a description tells of a failure, in three kinds that are not said
/// alike. The description could not be read: nothing was written. The files would make it too long:
/// nothing was written. The write was sent and failed: what is said depends on whether it may have been
/// made. A key Linear does not take is said as it is for every call.
fn told(step: Step) -> Failure {
    match step {
        Step::Read(Error::Refused) | Step::Write(Error::Refused) => {
            Failure::Linear(Error::Refused)
        }
        Step::Read(error) => Failure::DescribeFailed(format!(
            "the description could not be read, so nothing was written ({error})"
        )),
        Step::TooLong => Failure::DescribeFailed(
            "the description would be too long with the files, so nothing was written".to_owned(),
        ),
        Step::Write(error) => match error {
            Error::Refused => Failure::Linear(error),
            Error::Forbidden => Failure::DescribeFailed(
                "Linear did not change it: this key cannot change issues".to_owned(),
            ),
            Error::Unreachable(_) => Failure::DescribeFailed(format!(
                "{error}; look at the description before adding the files again"
            )),
            Error::Unsure => Failure::DescribeFailed(
                "Linear did not answer whole, so it may have added them: look at the description before adding them again"
                    .to_owned(),
            ),
            Error::Failed(why) => Failure::DescribeFailed(format!(
                "Linear did not change it: {}",
                why.strip_prefix("Linear said: ").unwrap_or(&why)
            )),
        },
    }
}

fn describe(session: &Session, params: Describe) -> Result<Value, Failure> {
    check_id(&params.id)?;
    let given = given(&params.files)?;
    if given.is_empty() {
        return Err(Failure::BadParams(
            "files are the files to add, from one to ten".to_owned(),
        ));
    }
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let lines = send(session, &given, "added")?;
    session.describe(&params.id, &lines).map_err(told)?;
    Ok(json!({}))
}

/// How much of a message a payload may take: a quarter of the 1 MiB one may have, for the time the page
/// takes to read it; what is around it has the rest.
const WIRE_BUDGET: usize = 256 * 1024;

/// How many of the first `items` go in a message together, each taking `size(item)` bytes: at least one,
/// where there is one.
fn fitting<T: Serialize>(items: &[T]) -> usize {
    let mut used = 0;
    for (index, item) in items.iter().enumerate() {
        used += serde_json::to_string(item).map_or(0, |text| text.len());
        if used > WIRE_BUDGET {
            return index.max(1);
        }
    }
    items.len()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfOpen {
    src: String,
    owner: String,
}

/// What `pdf-open` tells of the PDF: its pages, their sizes as runs, and whether it has words at all.
#[derive(Serialize)]
struct PdfOpened {
    pages: usize,
    sizes: Vec<(usize, f64, f64)>,
    text: bool,
}

fn pdf_open(session: &Session, params: PdfOpen) -> Result<PdfOpened, Failure> {
    check_id(&params.owner)?;
    if params.src.is_empty() || params.src.len() > SRC_MOST {
        return Err(Failure::BadParams(format!(
            "src is the address of a file, up to {SRC_MOST} characters"
        )));
    }
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let document = session
        .open_pdf(&params.src, &params.owner)
        .map_err(|why| Failure::PdfFailed(why.words().to_owned()))?;
    Ok(PdfOpened {
        pages: document.sizes().len(),
        sizes: document.runs(),
        text: document.has_text(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfClose {
    owner: String,
}

fn pdf_close(session: &Session, params: PdfClose) -> Result<Value, Failure> {
    check_id(&params.owner)?;
    session.close_pdf(&params.owner);
    Ok(json!({}))
}

/// The PDF `owner` has open.
fn document(session: &Session, owner: &str) -> Result<Arc<Document>, Failure> {
    check_id(owner)?;
    session
        .document(owner)
        .ok_or_else(|| Failure::PdfFailed("That PDF is not open".to_owned()))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfText {
    owner: String,
    /// From 1.
    page: usize,
}

/// The words of one page: its lines, from the top down, as many as fit one message.
#[derive(Serialize)]
struct PageText {
    page: usize,
    lines: Vec<crate::pictures::document::Line>,
    /// Lines are left out that did not fit.
    truncated: bool,
}

fn pdf_text(session: &Session, params: PdfText, _: &dyn Fn() -> bool) -> Result<PageText, Failure> {
    let document = document(session, &params.owner)?;
    let text = params
        .page
        .checked_sub(1)
        .and_then(|index| document.text(index))
        .ok_or_else(|| Failure::PdfFailed(format!("This PDF has no page {}", params.page)))?;
    let count = fitting(&text.lines);
    let truncated = count < text.lines.len();
    let lines = text.lines[..count].to_vec();
    Ok(PageText {
        page: params.page,
        lines,
        truncated,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfFind {
    owner: String,
    query: String,
    #[serde(default, rename = "caseSensitive")]
    case_sensitive: bool,
}

/// What a search through a PDF found: the places, in the order of the pages.
#[derive(Serialize)]
struct PdfFound {
    matches: Vec<crate::pictures::document::PageMatch>,
    searched: usize,
    truncated: bool,
    ms: u64,
}

fn pdf_find(
    session: &Session,
    params: PdfFind,
    gone: &dyn Fn() -> bool,
) -> Result<PdfFound, Failure> {
    if params.query.chars().count() > QUERY_MOST {
        return Err(Failure::BadParams(format!(
            "query is at most {QUERY_MOST} characters"
        )));
    }
    let document = document(session, &params.owner)?;
    let mut found = document.find(
        &Query {
            text: params.query,
            case_sensitive: params.case_sensitive,
        },
        gone,
    );
    let count = fitting(&found.matches);
    if count < found.matches.len() {
        found.matches.truncate(count);
        found.truncated = true;
    }
    Ok(PdfFound {
        matches: found.matches,
        searched: found.searched,
        truncated: found.truncated,
        ms: found.ms,
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "picture", picture);
    let options = call(options, session, "release", release);
    let options = call(options, session, "save", save);
    let options = call(options, session, "pdf-open", pdf_open);
    let options = call(options, session, "pdf-close", pdf_close);
    let options = call_stoppable(options, session, "pdf-text", pdf_text);
    let options = call_stoppable(options, session, "pdf-find", pdf_find);
    call(options, session, "describe", describe)
}

/// The words of a comment or a description with the lines of the files after them.
pub(super) fn with_lines(words: &str, lines: &[String]) -> String {
    images::with_lines(words, lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read<P: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<P, String> {
        super::super::params::parse(value)
    }

    #[test]
    fn a_file_is_asked_by_its_address_for_an_owner_and_saved_by_the_handle_of_a_place() {
        assert!(
            read::<Show>(json!({"src": "https://uploads.linear.app/a", "owner": "issue-1"}))
                .is_ok()
        );
        assert!(read::<Show>(json!({"src": "x"})).is_err());
        assert!(
            read::<Show>(json!({"src": "x", "owner": "o", "key": "k"})).is_err(),
            "no member the call does not name"
        );
        assert!(
            read::<Save>(json!({"src": "https://uploads.linear.app/a", "handle": "0a1b"})).is_ok()
        );
        assert!(
            read::<Save>(json!({"src": "x", "handle": "0a", "path": "/etc/passwd"})).is_err(),
            "no place but the one the terminal made a handle of"
        );
        assert!(read::<Release>(json!({"owner": "o"})).is_ok());
        assert!(read::<Release>(json!({})).is_err());
    }

    #[test]
    fn a_pdf_is_opened_by_its_address_and_asked_for_by_its_issue_and_a_page() {
        assert!(
            read::<PdfOpen>(json!({"src": "https://uploads.linear.app/a", "owner": "o"})).is_ok()
        );
        assert!(read::<PdfOpen>(json!({"src": "x", "owner": "o", "path": "/etc/passwd"})).is_err());
        assert!(read::<PdfClose>(json!({"owner": "o"})).is_ok());
        assert!(read::<PdfText>(json!({"owner": "o", "page": 3})).is_ok());
        assert!(read::<PdfText>(json!({"owner": "o", "page": -1})).is_err());
        assert!(read::<PdfText>(json!({"owner": "o"})).is_err());
        let find = read::<PdfFind>(json!({"owner": "o", "query": "limit"})).unwrap();
        assert!(!find.case_sensitive, "capitals do not count unless asked");
        let exact = read::<PdfFind>(json!({"owner": "o", "query": "limit", "caseSensitive": true}));
        assert!(exact.unwrap().case_sensitive);
        assert!(
            read::<PdfFind>(json!({"owner": "o", "query": "a", "regex": true})).is_err(),
            "no pattern: the words are found as they are written"
        );
    }

    #[test]
    fn as_many_go_in_a_message_as_fit_and_one_at_least() {
        let big = "x".repeat(WIRE_BUDGET / 2 - 8);
        assert_eq!(fitting(&[big.clone(), big.clone(), big.clone()]), 2);
        assert_eq!(fitting(&["x".repeat(WIRE_BUDGET * 2)]), 1);
        assert_eq!(fitting::<String>(&[]), 0);
        assert_eq!(fitting(&["a", "b"]), 2);
    }

    #[test]
    fn a_line_of_the_most_characters_is_a_small_part_of_the_message_and_a_page_of_them_is_cut() {
        // The worst line: the most characters, each of four bytes and with the widest place across.
        let line = crate::pictures::document::Line {
            x: 16_384.12,
            y: 16_384.12,
            w: 16_384.12,
            h: 16_384.12,
            text: "\u{1F600}".repeat(crate::pictures::document::LINE_MOST),
            xs: vec![16_384.12; crate::pictures::document::LINE_MOST + 1],
        };
        let size = serde_json::to_string(&line).unwrap().len();
        assert!(size * 4 < WIRE_BUDGET, "{size}");
        // A page of lines of the kind is cut to the budget: the message is the budget and one line at
        // most, a long way under the 1 MiB a message may have.
        let lines = vec![line; 100];
        let count = fitting(&lines);
        assert!(count >= 1 && count < lines.len(), "{count}");
        let message = serde_json::to_string(&lines[..count]).unwrap().len();
        assert!(message < 1024 * 1024 / 2, "{message}");
    }

    #[test]
    fn a_description_that_was_not_read_one_that_would_be_too_long_and_a_write_that_failed_are_told_apart()
     {
        let words = |step| match told(step) {
            Failure::DescribeFailed(words) => words,
            other => panic!("{other:?}"),
        };
        let unread = words(Step::Read(Error::Unreachable("down".to_owned())));
        let long = words(Step::TooLong);
        let written = words(Step::Write(Error::Unreachable("timed out".to_owned())));
        let unsure = words(Step::Write(Error::Unsure));
        let refused = words(Step::Write(Error::Failed("Linear said: no".to_owned())));
        // Where nothing was written, it says so; where a write was sent, it does not say it was not.
        assert!(unread.contains("could not be read") && unread.contains("nothing was written"));
        assert!(long.contains("too long") && long.contains("nothing was written"));
        assert!(
            !written.contains("nothing was written") && written.contains("look at the description")
        );
        assert!(unsure.contains("may have added them"));
        assert!(
            refused.starts_with("Linear did not change it") && !refused.contains("Linear said")
        );
        let all = [&unread, &long, &written, &unsure, &refused];
        for (at, one) in all.iter().enumerate() {
            assert!(all.iter().skip(at + 1).all(|other| other != one));
        }
        // A key that is not taken is said as it is, whether it failed the reading or the writing.
        assert!(matches!(
            told(Step::Read(Error::Refused)),
            Failure::Linear(Error::Refused)
        ));
        assert!(matches!(
            told(Step::Write(Error::Refused)),
            Failure::Linear(Error::Refused)
        ));
    }

    #[test]
    fn the_files_given_are_handles_and_names_at_most_ten() {
        let image = |handle: &str, name: &str| Attached {
            handle: handle.to_owned(),
            name: name.to_owned(),
        };
        let handle = "0123456789abcdef0123456789abcdef";
        assert_eq!(given(&[image(handle, "a.png")]).unwrap().len(), 1);
        assert!(given(&[image("", "a.png")]).is_err());
        assert!(given(&[image("../etc/passwd", "a.png")]).is_err());
        assert!(given(&[image(handle, &"n".repeat(256))]).is_err());
        let eleven: Vec<Attached> = (0..11).map(|_| image(handle, "a.png")).collect();
        assert!(given(&eleven).is_err());
        assert!(given(&eleven[..10]).is_ok());
        assert!(read::<Describe>(json!({"id": "i", "files": [], "description": "no"})).is_err());
        assert!(
            read::<Attached>(json!({"handle": handle, "name": "a", "path": "/etc/passwd"}))
                .is_err(),
            "a path is not taken: the terminal's handle is the way to the file"
        );
    }
}
