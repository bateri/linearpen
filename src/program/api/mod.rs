//! Everything the page talks to the program about: the calls and the topic of `docs/calls.md`. It
//! knows the page; `linear/`, `model/`, `state/` and `text/` do not, and what the page is told is made
//! here from what they hold.
//!
//! A call's params are read before it runs (what does not fit is `bad-params`, and the work never
//! runs). A call that panics is `internal`, by pencere.

mod change;
mod create;
mod issue;
mod issues;
mod params;
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

/// The calls and the topic the page may use, answered from `session`.
pub fn serve(options: Options, session: &Arc<Session>) -> Options {
    let options = session::calls(options, session);
    let options = issue::calls(options, session);
    let options = work::calls(options, session);
    let options = create::calls(options, session);
    let options = change::calls(options, session);
    issues::topic(options, session)
}
