//! Linear: the questions the program asks it and how the answers are read. It reads, and writes one
//! thing, the issue the person makes: the operations are in `query.rs`, all of them plain `query`
//! but `IssueCreate`, and `run` refuses anything that is not listed there. `create` is the only
//! function that sends the mutation, and sends it once. The way to Linear is `curl`
//! (`transport.rs`). It knows nothing of the page; it takes and gives plain Rust values.

mod error;
mod key;
mod new_issue;
pub mod query;
pub mod transport;
pub mod types;

pub use error::{Error, plain, tidy};
pub use key::{Key, KeyError};
pub use new_issue::{Created, NewIssue, Options, Team};
pub use transport::Transport;
pub use types::{Issue, Priority, Viewer};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use query::{Filter, Lookup, Operation, Page};
use types::Connection;

/// Which issues: the ones assigned to the person, or all of the workspace's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    Mine,
    All,
}

impl Scope {
    /// The name the page is told: `mine` or `all`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Mine => "mine",
            Self::All => "all",
        }
    }
}

/// The most issues of one list that are taken, and how many are asked for at a time.
pub const LIMIT: usize = 500;
pub const PAGE: u32 = 100;

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
            "{} is not a query, and the one thing this program writes is a new issue",
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
        return Err(Error::Failed(format!(
            "Linear said: {}",
            tidy(&first.message)
        )));
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

/// The open issues of `scope`, a hundred a page, up to [`LIMIT`].
pub fn fetch(transport: &dyn Transport, key: &Key, scope: Scope) -> Result<Fetched, Error> {
    let mut issues: Vec<Issue> = Vec::new();
    let mut after: Option<String> = None;
    let mut truncated = false;
    loop {
        let page = Page {
            first: PAGE,
            after: after.as_deref(),
            filter: Filter::open(),
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
        };
        issues.extend(connection.nodes);
        let more = connection.page_info.has_next_page;
        if issues.len() >= LIMIT {
            issues.truncate(LIMIT);
            truncated = more;
            return Ok(Fetched {
                viewer,
                listing: Listing { issues, truncated },
            });
        }
        match (more, connection.page_info.end_cursor) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => {
                return Ok(Fetched {
                    viewer,
                    listing: Listing { issues, truncated },
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
        .ok_or_else(|| Error::Failed("Linear did not make the issue".to_owned()))
}

#[cfg(test)]
mod tests;
