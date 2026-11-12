//! The calls about the session: `boot`, `connect`, `view` and `refresh`.

use std::collections::HashMap;
use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issue::check_id;
use super::issues::{Snapshot, snapshot};
use super::work::{Remembered, remembered};
use super::{Failure, call};
use crate::linear::{self, Filters, Key, Scope};
use crate::session::Session;
use crate::state::credentials::{self, Source};

/// The longest filter (or the longest words of a search) taken, in characters, and the most groups
/// folded one way or the other.
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
    /// What the form that hands issues to an agent was last left at, to open it with.
    prefs: Remembered,
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
    // A page that opens (again, under `make dev`) numbers its `view` calls from one.
    session.forget_view_order();
    // The lists are asked for as the page opens, if there is a key to ask with.
    session.begin();
    Ok(Boot {
        version: env!("CARGO_PKG_VERSION"),
        key,
        problem,
        prefs: remembered(session),
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
    list: Option<ListParams>,
    /// What Linear narrows the list by; all of it, or the call does not set it.
    filters: Option<FiltersParams>,
    filter: Option<String>,
    closed: Option<HashMap<String, bool>>,
    /// The number of this call among the page's `view` calls.
    seq: Option<u64>,
}

/// What the lists are narrowed by: each kind is a list of values (none: it does not narrow).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FiltersParams {
    #[serde(default)]
    status: Vec<String>,
    #[serde(default)]
    priority: Vec<String>,
    #[serde(default)]
    assignee: Vec<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    project: Vec<String>,
}

/// A list, by what it is (`mine`, `all`, `team`, `cycle`, `view`, `project`, `label`, `search`) and,
/// for the ones that are of something, Linear's id of it (for a search, the words).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    kind: String,
    id: Option<String>,
}

fn view(session: &Session, params: ViewParams) -> Result<Snapshot, Failure> {
    let scope = match params.list {
        None => None,
        Some(list) => {
            if list.kind == "search" {
                // The words are not an id: any words, but not too long, and not a control character.
                if let Some(words) = &list.id
                    && (words.trim().is_empty()
                        || words.chars().count() > FILTER_MOST
                        || words.chars().any(char::is_control))
                {
                    return Err(Failure::BadParams(format!(
                        "a search is for up to {FILTER_MOST} characters of words"
                    )));
                }
            } else if let Some(id) = &list.id {
                check_id(id)?;
            }
            let id = list.id.as_deref().map(str::trim);
            Some(Scope::of(&list.kind, id).ok_or_else(|| {
                Failure::BadParams(
                    "list is { kind, id }: \"mine\" or \"all\" with no id, or \"team\", \"cycle\", \"view\", \"project\" or \"label\" with the id of one, or \"search\" with the words".to_owned(),
                )
            })?)
        }
    };
    let filters = match params.filters {
        None => None,
        Some(given) => Some(
            Filters::new(
                given.status,
                given.priority,
                given.assignee,
                given.labels,
                given.project,
            )
            .map_err(|why| Failure::BadParams(why.0))?,
        ),
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
    session.set_view(params.seq, scope, filters, params.filter, params.closed);
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
