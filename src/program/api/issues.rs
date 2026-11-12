//! The list the page draws: what the person is looking at (the scope, the filter, the groups folded)
//! made from the two lists the session holds, as the topic `issues` and the answer of `view`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use pencere::{Options, ViewError};
use serde::Serialize;

use crate::linear::{Issue, Scope, plain};
use crate::model::filter::Filter;
use crate::model::groups::{Group, group};
use crate::model::time::short_date;
use crate::session::{Session, State};

#[derive(Debug, Serialize)]
pub struct LabelMessage {
    /// Linear's id of the label: what a change adds or removes it by.
    pub id: String,
    pub name: String,
    pub color: String,
}

/// Linear's ids of what an issue is of and in, which the page names in a change and by which a menu
/// marks what the issue has: the team, the state, the person and the project.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ids {
    pub team_id: Option<String>,
    pub state_id: String,
    pub assignee_id: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowMessage {
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// Where the issue is in Linear, and the name of the branch Linear suggests for it (none where
    /// it has none): what the page copies. Neither holds a control character.
    pub url: String,
    pub branch_name: Option<String>,
    pub priority: &'static str,
    pub updated: String,
    pub labels: Vec<LabelMessage>,
    /// Whose it is, for the list of all issues: "you", a name, or nothing.
    pub assignee: Option<String>,
    #[serde(flatten)]
    pub ids: Ids,
}

pub fn ids(issue: &Issue) -> Ids {
    Ids {
        team_id: issue.team.as_ref().map(|team| team.id.clone()),
        state_id: issue.state.id.clone(),
        assignee_id: issue.assignee.as_ref().map(|who| who.id.clone()),
        project_id: issue.project.as_ref().map(|project| project.id.clone()),
    }
}

pub fn labels(issue: &Issue) -> Vec<LabelMessage> {
    issue
        .labels
        .iter()
        .map(|label| LabelMessage {
            id: label.id.clone(),
            name: label.name.clone(),
            color: label.color.clone(),
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct GroupMessage {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
    /// How many issues the group holds (with the filter on).
    pub count: usize,
    pub closed: bool,
    /// The rows of the group; none where it is folded.
    pub rows: Vec<RowMessage>,
}

#[derive(Debug, Serialize)]
pub struct ErrorMessage {
    pub kind: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub mine: usize,
    pub all: usize,
    /// There are more than the program takes of that list.
    pub mine_cut: bool,
    pub all_cut: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkingMessage {
    /// The name of the agent the issue was handed to.
    pub agent: &'static str,
    /// Its pane has written nothing for a while: the agent is done, or waits for the person.
    pub idle: bool,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    /// There are lists to show (the first answer of Linear has come).
    pub loaded: bool,
    /// Linear is being asked now.
    pub refreshing: bool,
    pub error: Option<ErrorMessage>,
    pub scope: &'static str,
    pub filter: String,
    pub counts: Counts,
    /// How many issues of the scope the filter lets through.
    pub matched: usize,
    pub groups: Vec<GroupMessage>,
    /// When the lists were told, seconds since the epoch.
    pub updated: Option<u64>,
    pub organization: Option<String>,
    pub person: Option<String>,
    /// The issues an agent works on, by Linear's id.
    pub working: BTreeMap<String, WorkingMessage>,
}

/// The branch Linear suggests for an issue, as one line with no control character in it, or none.
pub fn branch_name(issue: &Issue) -> Option<String> {
    issue
        .branch_name
        .as_deref()
        .map(plain)
        .filter(|name| !name.is_empty())
}

fn row(issue: &Issue, scope: Scope) -> RowMessage {
    RowMessage {
        id: issue.id.clone(),
        identifier: issue.identifier.clone(),
        title: issue.title.clone(),
        url: plain(&issue.url),
        branch_name: branch_name(issue),
        priority: issue.priority.name(),
        updated: short_date(&issue.updated_at),
        labels: labels(issue),
        assignee: match (scope, &issue.assignee) {
            (Scope::All, Some(who)) if who.is_me => Some("you".to_owned()),
            (Scope::All, Some(who)) => Some(who.display_name.clone()),
            _ => None,
        },
        ids: ids(issue),
    }
}

/// The key of a group, which the page names it by when it folds it.
fn key_of(group: &Group<'_>) -> String {
    format!("{}:{}", group.kind.name(), group.name)
}

/// What the page is to draw, from `state`.
pub fn snapshot(state: &State) -> Snapshot {
    let scope = state.view.scope;
    let filter = Filter::new(&state.view.filter);
    let mut groups = Vec::new();
    let mut matched = 0;
    if let Some(lists) = &state.lists {
        let listing = match scope {
            Scope::Mine => &lists.mine,
            Scope::All => &lists.all,
        };
        for found in group(&listing.issues, &filter) {
            let key = key_of(&found);
            // A filter shows what it finds: no group is folded over a match.
            let closed = filter.is_empty()
                && state
                    .view
                    .closed
                    .get(&key)
                    .copied()
                    .unwrap_or_else(|| found.folded_at_first());
            matched += found.issues.len();
            groups.push(GroupMessage {
                key,
                name: found.name.to_owned(),
                kind: found.kind.name(),
                count: found.issues.len(),
                closed,
                rows: if closed {
                    Vec::new()
                } else {
                    found.issues.iter().map(|issue| row(issue, scope)).collect()
                },
            });
        }
    }
    let counts = state.lists.as_ref().map_or(
        Counts {
            mine: 0,
            all: 0,
            mine_cut: false,
            all_cut: false,
        },
        |lists| Counts {
            mine: lists.mine.issues.len(),
            all: lists.all.issues.len(),
            mine_cut: lists.mine.truncated,
            all_cut: lists.all.truncated,
        },
    );
    Snapshot {
        loaded: state.lists.is_some(),
        refreshing: state.refreshing,
        error: state.error.as_ref().map(|error| ErrorMessage {
            kind: error.kind(),
            message: error.to_string(),
        }),
        scope: scope.name(),
        filter: state.view.filter.clone(),
        counts,
        matched,
        groups,
        updated: state.updated,
        organization: state
            .lists
            .as_ref()
            .map(|lists| lists.viewer.organization.name.clone()),
        person: state.lists.as_ref().map(|lists| lists.viewer.name.clone()),
        working: state
            .working
            .iter()
            .map(|(id, working)| {
                (
                    id.clone(),
                    WorkingMessage {
                        agent: working.agent,
                        idle: working.idle,
                    },
                )
            })
            .collect(),
    }
}

/// How long a thread of the topic waits before it looks whether it is cancelled.
const LOOK: Duration = Duration::from_millis(500);

pub fn topic(options: Options, session: &Arc<Session>) -> Options {
    let session = Arc::clone(session);
    options.topic("issues", move |_, subscription| {
        let session = Arc::clone(&session);
        let feed = subscription.clone();
        let waker = Arc::clone(&session);
        // Whoever listens wants the lists: they are asked for if they are not yet.
        session.begin();
        // The thread waits on the session, not on the feed: the end of the subscription wakes it.
        subscription.on_end(move || waker.wake());
        thread::Builder::new()
            .name("linearpen-issues".to_owned())
            .spawn(move || {
                // What the session holds as it is when the page subscribes, and then each change.
                let mut seen = None;
                while !feed.is_cancelled() {
                    let generation = session.generation();
                    if seen != Some(generation) {
                        seen = Some(generation);
                        let message = snapshot(&session.state());
                        if let Ok(value) = serde_json::to_value(message) {
                            feed.push(value);
                        }
                    }
                    session.wait_change(generation, LOOK);
                }
            })
            .map(drop)
            .map_err(|why| {
                ViewError::with_message("internal", format!("the issues cannot be watched: {why}"))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::fixture_issue;

    #[test]
    fn a_row_tells_the_ids_of_what_the_issue_is_of_and_in() {
        let value = serde_json::to_value(row(&fixture_issue(), Scope::Mine)).unwrap();
        assert_eq!(value["teamId"], "team-acm");
        assert_eq!(value["stateId"], "s1");
        assert_eq!(value["assigneeId"], "u-ada");
        assert!(value["projectId"].is_null());
        assert_eq!(value["labels"][0]["id"], "l-api");
        // Whose it is is said in the list of all issues alone; the id always.
        assert!(value["assignee"].is_null());
    }
}
