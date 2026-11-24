//! Everything the page talks to the program about: the calls and the topic of `docs/calls.md`. It
//! knows the page; `linear/`, `model/`, `state/` and `text/` do not, and what the page is told is made
//! here from what they hold.
//!
//! A call's params are read before it runs (what does not fit is `bad-params`, and the work never
//! runs). A call that panics is `internal`, by pencere.

mod change;
mod comment;
mod create;
mod edit;
mod filters;
mod inbox;
mod issue;
mod issues;
mod params;
mod pictures;
mod session;
mod work;

use std::sync::Arc;

use pencere::{Call, Options, ViewError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::linear::Error;
use crate::session::Session;

/// What a call could not do.
#[derive(Debug)]
enum Failure {
    /// The params are not what the call takes.
    BadParams(String),
    /// There is no key to ask Linear with.
    NoKey(String),
    /// Linear is not reached, did not accept the key, or answered what was not asked.
    Linear(Error),
    /// An agent cannot be started from here: bateri or the agent is not there.
    Unavailable(String),
    /// An agent was to be started, and was not.
    StartFailed(String),
    /// An issue was to be made, and Linear did not make it: the words say why.
    CreateFailed(String),
    /// A change was to be made to issues, and Linear did not make it: the words say why.
    ChangeFailed(String),
    /// A comment was to be posted, and Linear did not post it (or it is not known whether it did): the
    /// words say why.
    CommentFailed(String),
    /// A file was to be sent with a comment, an issue or a description, and was not: nothing was
    /// sent after it, and the words say which and why.
    UploadFailed(String),
    /// The files were to be added to a description and Linear did not write it (or it is not
    /// known whether it did): the words say why.
    DescribeFailed(String),
    /// The words of a description or of a comment were to be written over it, and were not (or it is
    /// not known whether they were): the words say why.
    EditFailed(String),
    /// A file of Linear was to be saved, and was not: the words say why.
    SaveFailed(String),
    /// A PDF could not be read for its pages and words: the words say why.
    PdfFailed(String),
    /// The keyboard was to go to an agent's pane, and did not: the words say why.
    GoFailed(String),
}

impl From<Failure> for ViewError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::BadParams(words) => Self::with_message("bad-params", words),
            Failure::NoKey(words) => Self::with_message("no-key", words),
            Failure::Unavailable(words) => Self::with_message("unavailable", words),
            Failure::StartFailed(words) => Self::with_message("start-failed", words),
            Failure::CreateFailed(words) => Self::with_message("create-failed", words),
            Failure::ChangeFailed(words) => Self::with_message("change-failed", words),
            Failure::CommentFailed(words) => Self::with_message("comment-failed", words),
            Failure::UploadFailed(words) => Self::with_message("upload-failed", words),
            Failure::DescribeFailed(words) => Self::with_message("describe-failed", words),
            Failure::EditFailed(words) => Self::with_message("edit-failed", words),
            Failure::SaveFailed(words) => Self::with_message("save-failed", words),
            Failure::PdfFailed(words) => Self::with_message("pdf-failed", words),
            Failure::GoFailed(words) => Self::with_message("go-failed", words),
            Failure::Linear(error) => Self::with_message(error.kind(), error.to_string()),
        }
    }
}

/// The work of a call: the session and the params it read.
type Work<P, T> = fn(&Session, P) -> Result<T, Failure>;

/// Answers the calls of the page to `name` with `work`, given the params as `P` reads them. Calls
/// run at the same time as each other.
fn call<P: DeserializeOwned + 'static, T: Serialize + 'static>(
    options: Options,
    session: &Arc<Session>,
    name: &'static str,
    work: Work<P, T>,
) -> Options {
    let session = Arc::clone(session);
    options.call(name, move |params: Value, _: &Call| {
        let params =
            params::parse::<P>(params).map_err(|why| ViewError::with_message("bad-params", why))?;
        work(&session, params).map_err(ViewError::from)
    })
}

/// The work of a call that can be given up: the session, the params it read, and whether the page gave
/// the call up (a `cancel`), which work that takes a while looks at.
type Stoppable<P, T> = fn(&Session, P, &dyn Fn() -> bool) -> Result<T, Failure>;

/// As `call`, for work that stops when the page gives the call up.
fn call_stoppable<P: DeserializeOwned + 'static, T: Serialize + 'static>(
    options: Options,
    session: &Arc<Session>,
    name: &'static str,
    work: Stoppable<P, T>,
) -> Options {
    let session = Arc::clone(session);
    options.call(name, move |params: Value, call: &Call| {
        let params =
            params::parse::<P>(params).map_err(|why| ViewError::with_message("bad-params", why))?;
        work(&session, params, &|| call.is_cancelled()).map_err(ViewError::from)
    })
}

/// The calls and the topic the page may use, answered from `session`.
pub fn serve(options: Options, session: &Arc<Session>) -> Options {
    let options = session::calls(options, session);
    let options = issue::calls(options, session);
    let options = work::calls(options, session);
    let options = create::calls(options, session);
    let options = change::calls(options, session);
    let options = comment::calls(options, session);
    let options = edit::calls(options, session);
    let options = pictures::calls(options, session);
    let options = filters::calls(options, session);
    let options = inbox::calls(options, session);
    issues::topic(options, session)
}
