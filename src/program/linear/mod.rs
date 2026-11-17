//! Linear: the questions the program asks it and how the answers are read. It reads, and writes three
//! things, the issue the person makes, the change the person makes to issues and the comment the
//! person writes: the operations are in `query.rs`, all of them plain `query` but `IssueCreate`,
//! `IssueBatchUpdate` and `CommentCreate`, and `run` refuses anything that is not listed there.
//! `create`, `change`, `change_all` and `comment` are the only functions that send them, and each sends
//! once; the page's session and the command line are the two that call them. The way to Linear is
//! `curl` (`transport.rs`). It knows nothing of the page; it takes and gives plain Rust values.

mod change;
mod comment;
mod error;
mod filter_options;
mod filters;
mod key;
pub mod limits;
mod names;
mod new_issue;
pub mod query;
pub mod transport;
pub mod types;

pub use change::{Change, Changes};
pub use comment::{NOT_POSTED, post as comment};
pub use error::{Error, lines, plain, tidy};
pub use filter_options::{FilterOptions, filter_options};
pub use filters::{Filters, ME, NONE};
pub use key::{Key, KeyError};
pub use names::{Entry, names};
pub use new_issue::{Created, CycleChoice, NewIssue, Options, StateChoice, Team};
pub use transport::Transport;
pub use types::{Issue, Priority, Viewer};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use query::{Lookup, Operation, Page};
use types::Connection;

/// Which issues a list shows: the ones assigned to the person, all of the workspace's, the open
/// issues of one thing Linear has (by its id): a team, a cycle, a saved view, a project, a label, or
/// what a search for some words finds (the whole workspace, the closed issues too).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    Mine,
    All,
    Team(String),
    Cycle(String),
    View(String),
    Project(String),
    Label(String),
    /// The words searched for.
    Search(String),
}

impl Scope {
    /// The word the page is told: `mine`, `all`, `team`, `cycle`, `view`, `project`, `label` or
    /// `search`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::All => "all",
            Self::Team(_) => "team",
            Self::Cycle(_) => "cycle",
            Self::View(_) => "view",
            Self::Project(_) => "project",
            Self::Label(_) => "label",
            Self::Search(_) => "search",
        }
    }

    /// Linear's id of what the list is of, where it is of one; the words, for a search.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Mine | Self::All => None,
            Self::Team(id)
            | Self::Cycle(id)
            | Self::View(id)
            | Self::Project(id)
            | Self::Label(id)
            | Self::Search(id) => Some(id),
        }
    }

    /// The scope the page names by `kind` and `id`; none where it names no scope (a kind there is not,
    /// an id for a list that has none, no id for one that needs it).
    #[must_use]
    pub fn of(kind: &str, id: Option<&str>) -> Option<Self> {
        let id = id.map(str::to_owned);
        match (kind, id) {
            ("mine", None) => Some(Self::Mine),
            ("all", None) => Some(Self::All),
            ("team", Some(id)) if !id.is_empty() => Some(Self::Team(id)),
            ("cycle", Some(id)) if !id.is_empty() => Some(Self::Cycle(id)),
            ("view", Some(id)) if !id.is_empty() => Some(Self::View(id)),
            ("project", Some(id)) if !id.is_empty() => Some(Self::Project(id)),
            ("label", Some(id)) if !id.is_empty() => Some(Self::Label(id)),
            ("search", Some(words)) if !words.is_empty() => Some(Self::Search(words)),
            _ => None,
        }
    }
}

/// The most issues of one list that are taken, and how many are asked for at a time.
pub const LIMIT: usize = 500;
pub const PAGE: u32 = 100;

/// The most issues one change is made to at once (Linear's own limit of a batch).
pub const BATCH_MOST: usize = 50;

/// One list of open issues.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Listing {
    pub issues: Vec<Issue>,
    /// There are more than [`LIMIT`]: the rest are not taken.
    pub truncated: bool,
}

/// What a list is read with: the list and whose key it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    pub viewer: Viewer,
    pub listing: Listing,
}

#[derive(Serialize)]
struct Request<'a, V: Serialize> {
    query: &'a str,
    #[serde(rename = "operationName")]
    operation_name: &'a str,
    variables: Option<V>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
    #[serde(default)]
    extensions: Option<Extensions>,
}

#[derive(Deserialize)]
struct Extensions {
    #[serde(default)]
    code: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

impl Extensions {
    /// Whether Linear calls the error `forbidden`, by its code or its kind.
    fn forbidden(&self) -> bool {
        [&self.code, &self.kind]
            .into_iter()
            .flatten()
            .any(|word| word.eq_ignore_ascii_case("forbidden"))
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

/// Asks `operation` with `variables` and reads the data of the answer as `T`.
fn run<V: Serialize, T: DeserializeOwned>(
    transport: &dyn Transport,
    key: &Key,
    operation: &Operation,
    variables: Option<V>,
) -> Result<T, Error> {
    if !operation.allowed() {
        return Err(Error::Failed(format!(
            "{} is not a query, and what this program writes is a new issue, the changes of issues and comments",
            operation.name
        )));
    }
    let body = serde_json::to_string(&Request {
        query: operation.text,
        operation_name: operation.name,
        variables,
    })
    .map_err(|why| Error::Failed(format!("the request could not be made: {why}")))?;
    let reply = transport.post(key, &body)?;
    if reply.status == 401 {
        return Err(Error::Refused);
    }
    if reply.status == 403 {
        return Err(Error::Forbidden);
    }
    let Ok(answer) = serde_json::from_str::<Envelope<T>>(&reply.body) else {
        return Err(Error::Failed(if reply.status == 200 {
            "Linear's answer is not what was asked for".to_owned()
        } else {
            format!("Linear answered HTTP {}", reply.status)
        }));
    };
    let refused = answer.errors.iter().any(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|e| e.code.as_deref())
            .is_some_and(|code| code == "AUTHENTICATION_ERROR")
    });
    if refused {
        return Err(Error::Refused);
    }
    if answer
        .errors
        .iter()
        .any(|error| error.extensions.as_ref().is_some_and(Extensions::forbidden))
    {
        return Err(Error::Forbidden);
    }
    if let Some(first) = answer.errors.first() {
        return Err(Error::Failed(format!("{SAID}{}", tidy(&first.message))));
    }
    if reply.status != 200 {
        return Err(Error::Failed(format!(
            "Linear answered HTTP {}",
            reply.status
        )));
    }
    answer
        .data
        .ok_or_else(|| Error::Failed("Linear's answer has no data".to_owned()))
}

#[derive(Deserialize)]
struct VerifyData {
    viewer: Viewer,
}

/// Whose key it is: the question that tells whether a key is one.
pub fn verify(transport: &dyn Transport, key: &Key) -> Result<Viewer, Error> {
    run::<(), VerifyData>(transport, key, &query::VERIFY, None).map(|data| data.viewer)
}

#[derive(Deserialize)]
struct MineViewer {
    #[serde(flatten)]
    viewer: Viewer,
    #[serde(rename = "assignedIssues")]
    assigned_issues: Connection,
}

#[derive(Deserialize)]
struct MineData {
    viewer: MineViewer,
}

#[derive(Deserialize)]
struct AllData {
    viewer: Viewer,
    issues: Connection,
}

/// The issues of one thing that has some, whatever Linear calls it (`team`, `cycle`, `customView`,
/// `project`, `issueLabel`).
#[derive(Deserialize)]
struct OwnerIssues {
    issues: Connection,
}

#[derive(Deserialize)]
struct OwnedData {
    viewer: Viewer,
    #[serde(
        alias = "team",
        alias = "cycle",
        alias = "customView",
        alias = "project",
        alias = "issueLabel"
    )]
    owner: OwnerIssues,
}

#[derive(Deserialize)]
struct SearchData {
    viewer: Viewer,
    #[serde(rename = "searchIssues")]
    search: Connection,
}

/// The open issues of `scope` that `filters` let through (all of them where there are none), a
/// hundred a page, up to [`LIMIT`]; a search's are the closed issues too.
pub fn fetch(
    transport: &dyn Transport,
    key: &Key,
    scope: &Scope,
    filters: &Filters,
) -> Result<Fetched, Error> {
    let mut issues: Vec<Issue> = Vec::new();
    let mut after: Option<String> = None;
    let filter = filters.to_value(!matches!(scope, Scope::Search(_)));
    loop {
        let page = Page {
            first: PAGE,
            after: after.as_deref(),
            filter: filter.clone(),
        };
        let (viewer, connection) = match scope {
            Scope::Mine => {
                let data: MineData = run(transport, key, &query::MY_ISSUES, Some(page))?;
                (data.viewer.viewer, data.viewer.assigned_issues)
            }
            Scope::All => {
                let data: AllData = run(transport, key, &query::ALL_ISSUES, Some(page))?;
                (data.viewer, data.issues)
            }
            Scope::Search(term) => {
                let data: SearchData = run(
                    transport,
                    key,
                    &query::SEARCH_ISSUES,
                    Some(query::Searched { term, page }),
                )?;
                (data.viewer, data.search)
            }
            Scope::Team(id)
            | Scope::Cycle(id)
            | Scope::View(id)
            | Scope::Project(id)
            | Scope::Label(id) => {
                let operation = match scope {
                    Scope::Team(_) => &query::TEAM_ISSUES,
                    Scope::Cycle(_) => &query::CYCLE_ISSUES,
                    Scope::View(_) => &query::VIEW_ISSUES,
                    Scope::Project(_) => &query::PROJECT_ISSUES,
                    _ => &query::LABEL_ISSUES,
                };
                let data: OwnedData =
                    run(transport, key, operation, Some(query::Owned { id, page }))?;
                (data.viewer, data.owner.issues)
            }
        };
        issues.extend(connection.nodes);
        let more = connection.page_info.has_next_page;
        if issues.len() >= LIMIT {
            issues.truncate(LIMIT);
            return Ok(Fetched {
                viewer,
                listing: Listing {
                    issues,
                    truncated: more,
                },
            });
        }
        match (more, connection.page_info.end_cursor) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => {
                return Ok(Fetched {
                    viewer,
                    listing: Listing {
                        issues,
                        truncated: false,
                    },
                });
            }
        }
    }
}

#[derive(Deserialize)]
struct IssueData {
    issue: Option<Issue>,
}

/// One issue with its description and its first comments.
pub fn fetch_issue(transport: &dyn Transport, key: &Key, id: &str) -> Result<Issue, Error> {
    let data: IssueData = run(transport, key, &query::ISSUE, Some(Lookup { id }))?;
    data.issue
        .ok_or_else(|| Error::Failed("Linear has no such issue".to_owned()))
}

/// The teams an issue can be made in, by name.
pub fn teams(transport: &dyn Transport, key: &Key) -> Result<Vec<Team>, Error> {
    let data: new_issue::TeamsData = run::<(), _>(transport, key, &query::NEW_ISSUE_TEAMS, None)?;
    Ok(data.teams())
}

/// What a new issue of the team with Linear's id `team` can be given.
pub fn options(transport: &dyn Transport, key: &Key, team: &str) -> Result<Options, Error> {
    let data: new_issue::OptionsData = run(
        transport,
        key,
        &query::NEW_ISSUE_OPTIONS,
        Some(query::Team { team }),
    )?;
    data.options()
        .ok_or_else(|| Error::Failed("Linear has no such team".to_owned()))
}

/// Makes the issue: the program's one write. It is sent once, whatever comes back; what the person
/// does of an answer that did not come is `Session::create`'s.
pub fn create(transport: &dyn Transport, key: &Key, input: &NewIssue) -> Result<Created, Error> {
    let data: new_issue::CreateData = run(
        transport,
        key,
        &query::ISSUE_CREATE,
        Some(query::Create { input }),
    )?;
    data.created()
        .ok_or_else(|| Error::Failed(NOT_MADE.to_owned()))
}

#[derive(Deserialize)]
struct BatchData {
    #[serde(rename = "issueBatchUpdate")]
    batch: Option<BatchPayload>,
}

#[derive(Deserialize)]
struct BatchPayload {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    issues: Option<Vec<Issue>>,
}

/// Makes the change in the issues with Linear's ids `ids`, one field of them: the program's other
/// write. It is sent once, whatever comes back, and gives the issues as Linear says they are now;
/// what the person does of an answer that did not come is `Session::change`'s.
pub fn change(
    transport: &dyn Transport,
    key: &Key,
    ids: &[String],
    change: &Change,
) -> Result<Vec<Issue>, Error> {
    send_batch(transport, key, ids, change.input())
}

/// Makes several changes in the issues with Linear's ids `ids` in one request, so that they are all
/// made or none: the same write as `change`, sent once, whatever comes back.
pub fn change_all(
    transport: &dyn Transport,
    key: &Key,
    ids: &[String],
    changes: &Changes,
) -> Result<Vec<Issue>, Error> {
    send_batch(transport, key, ids, changes.input())
}

/// The one place `IssueBatchUpdate` is sent from.
fn send_batch(
    transport: &dyn Transport,
    key: &Key,
    ids: &[String],
    input: change::Input<'_>,
) -> Result<Vec<Issue>, Error> {
    if ids.is_empty() || ids.len() > BATCH_MOST {
        return Err(Error::Failed(format!(
            "from one to {BATCH_MOST} issues are changed at once"
        )));
    }
    let data: BatchData = run(
        transport,
        key,
        &query::ISSUE_BATCH_UPDATE,
        Some(query::Batch { ids, input }),
    )?;
    data.batch
        .filter(|payload| payload.success)
        .map(|payload| payload.issues.unwrap_or_default())
        .ok_or_else(|| Error::Failed(NOT_CHANGED.to_owned()))
}

/// What a `Failed` of Linear's own errors begins with: Linear answered and said no.
pub const SAID: &str = "Linear said: ";

/// What `create` says where Linear answered with data and not with `success`: no.
const NOT_MADE: &str = "Linear did not make the issue";

/// What `change` says where Linear answered with data and not with `success`: no.
const NOT_CHANGED: &str = "Linear did not change the issues";

/// Whether a `Failed` of a write leaves it open that Linear made it. Linear's own errors, and a
/// payload that says it did not, are its no; an HTTP status that is not 200 with no error in it, and
/// a body that cannot be read (a gateway's 502, a page cut short), come after the request was
/// sent and say nothing of whether Linear acted on it.
#[must_use]
pub fn left_open(why: &str) -> bool {
    !(why.starts_with(SAID) || why == NOT_CHANGED || why == NOT_POSTED || why == NOT_MADE)
}

#[cfg(test)]
mod tests;
