//! The calls about the session: `boot`, `connect`, `view` and `refresh`.

use std::collections::HashMap;
use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issues::{Snapshot, snapshot};
use super::{Failure, call};
use crate::linear::{self, Key, Scope};
use crate::session::Session;
use crate::state::credentials::{self, Source};

/// The longest filter taken, in characters, and the most groups folded one way or the other.
const FILTER_MOST: usize = 200;
const CLOSED_MOST: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nothing {}

#[derive(Serialize)]
struct Boot {
    version: &'static str,
    /// Where the key is from: `none`, `environment` or `file`.
    key: &'static str,
    /// Why the key there is cannot be used, where there is one that cannot.
    problem: Option<String>,
}

fn boot(session: &Session, _: Nothing) -> Result<Boot, Failure> {
    let (key, problem) = {
        let state = session.state();
        let key = match &state.source {
            None => "none",
            Some(Source::Environment) => "environment",
            Some(Source::File(_)) => "file",
        };
        (key, state.problem.clone())
    };
    // The lists are asked for as the page opens, if there is a key to ask with.
    session.begin();
    Ok(Boot {
        version: env!("CARGO_PKG_VERSION"),
        key,
        problem,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connect {
    key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Connected {
    organization: String,
    /// The key is kept in the file of the config directory.
    saved: bool,
    /// Why it is not, where it is not: the key is used for this run only.
    save_problem: Option<String>,
}

fn connect(session: &Session, params: Connect) -> Result<Connected, Failure> {
    let key = Key::new(&params.key).map_err(|why| Failure::BadParams(why.to_string()))?;
    let viewer = linear::verify(session.transport(), &key).map_err(Failure::Linear)?;
    // Linear took the key: it is kept where the next run finds it.
    let (saved, save_problem, source) = match session.paths.config.as_deref() {
        Some(config) => match credentials::save(config, &key) {
            Ok(file) => (true, None, Source::File(file)),
            Err(why) => (false, Some(why), Source::Environment),
        },
        None => (
            false,
            Some("there is no folder for the key".to_owned()),
            Source::Environment,
        ),
    };
    session.use_key(key, source);
    Ok(Connected {
        organization: viewer.organization.name,
        saved,
        save_problem,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewParams {
    scope: Option<String>,
    filter: Option<String>,
    closed: Option<HashMap<String, bool>>,
}

fn view(session: &Session, params: ViewParams) -> Result<Snapshot, Failure> {
    let scope = match params.scope.as_deref() {
        None => None,
        Some("mine") => Some(Scope::Mine),
        Some("all") => Some(Scope::All),
        Some(other) => {
            return Err(Failure::BadParams(format!(
                "scope is \"mine\" or \"all\", not {other:?}"
            )));
        }
    };
    if params
        .filter
        .as_ref()
        .is_some_and(|filter| filter.chars().count() > FILTER_MOST)
    {
        return Err(Failure::BadParams(format!(
            "the filter is at most {FILTER_MOST} characters"
        )));
    }
    if params
        .closed
        .as_ref()
        .is_some_and(|closed| closed.len() > CLOSED_MOST)
    {
        return Err(Failure::BadParams(format!(
            "closed names at most {CLOSED_MOST} groups"
        )));
    }
    session.set_view(|view| {
        if let Some(scope) = scope {
            view.scope = scope;
        }
        if let Some(filter) = params.filter {
            view.filter = filter;
        }
        if let Some(closed) = params.closed {
            view.closed = closed;
        }
    });
    Ok(snapshot(&session.state()))
}

fn refresh(session: &Session, _: Nothing) -> Result<Done, Failure> {
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    session.refresh();
    Ok(Done {})
}

#[derive(Serialize)]
struct Done {}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "boot", boot);
    let options = call(options, session, "connect", connect);
    let options = call(options, session, "view", view);
    call(options, session, "refresh", refresh)
}
