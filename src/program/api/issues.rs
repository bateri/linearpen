//! The list the page draws: what the person is looking at (the list, the filter, the groups folded)
//! made from the lists the session holds, as the topic `issues` and the answer of `view`, and the
//! names of the lists a person can go to.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use pencere::{Options, ViewError};
use serde::Serialize;

use crate::linear::{Entry, Filters, Issue, Listing, Scope, plain};
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
/// marks what the issue has: the team, the state, the person, the project and the cycle, with its
/// estimate and its due date.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ids {
    pub team_id: Option<String>,
    pub state_id: String,
    pub assignee_id: Option<String>,
    pub project_id: Option<String>,
    pub cycle_id: Option<String>,
    /// Points, where the team estimates and the issue has some.
    pub estimate: Option<u32>,
    /// `2026-11-15`.
    pub due_date: Option<String>,
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
        cycle_id: issue.cycle.as_ref().map(|cycle| cycle.id.clone()),
        estimate: issue.estimate,
        due_date: issue.due_date.clone(),
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
    /// How many issues are the person's.
    pub mine: usize,
    /// There are more than the program takes of that list.
    pub mine_cut: bool,
    /// How many the list the page looks at holds (the same as `mine` where it is My issues), and
    /// whether there are more than the program takes of it.
    pub shown: usize,
    pub shown_cut: bool,
    /// How many of those are open: what the list button says.
    pub shown_open: usize,
}

/// A cycle of a list of the card.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleMessage {
    pub number: u32,
    /// When it ends, as Linear says it.
    pub ends_at: String,
    /// The day it ends, as the dates of the lists are written (`Nov 15`): the same way, once.
    pub ends: String,
}

/// A list a person can go to: what to ask for it by (`kind` and `id`), what it is called, and what
/// the card says of it.
#[derive(Debug, Serialize)]
pub struct ItemMessage {
    /// `view:<id>`: the name the page keeps it by.
    pub key: String,
    pub kind: &'static str,
    pub id: Option<String>,
    pub title: String,
    /// Where in the card it stands: `favorite`, `team`, `view` or `project`.
    pub section: &'static str,
    /// The key of the team it is of (`ACM`).
    pub team: Option<String>,
    pub cycle: Option<CycleMessage>,
}

/// The lists a person can go to, besides My issues and All issues, as Linear last said them.
#[derive(Debug, Serialize)]
pub struct CatalogMessage {
    /// Linear has said them (the first time it was asked).
    pub loaded: bool,
    /// Why the last asking could not, where it could not.
    pub error: Option<ErrorMessage>,
    pub items: Vec<ItemMessage>,
}

/// The name a scope is kept by: `mine`, `all`, or `view:<id>`.
#[must_use]
pub fn scope_key(scope: &Scope) -> String {
    match scope.id() {
        Some(id) => format!("{}:{id}", scope.kind()),
        None => scope.kind().to_owned(),
    }
}

fn item(entry: &Entry) -> ItemMessage {
    ItemMessage {
        key: scope_key(&entry.scope),
        kind: entry.scope.kind(),
        id: entry.scope.id().map(str::to_owned),
        title: entry.title.clone(),
        section: entry.section.name(),
        team: entry.team.clone(),
        cycle: entry.cycle.as_ref().map(|cycle| CycleMessage {
            number: cycle.number,
            ends_at: cycle.ends_at.clone(),
            ends: short_date(&cycle.ends_at),
        }),
    }
}

/// What the lists are narrowed by in Linear, as the page set it: the status names, the priorities
/// (`urgent` to `none`), the assignees (`me`, `none`, or Linear's id of a person), the label names
/// and the projects (`none`, or Linear's id).
#[derive(Debug, Serialize)]
pub struct FiltersMessage {
    pub status: Vec<String>,
    pub priority: Vec<String>,
    pub assignee: Vec<String>,
    pub labels: Vec<String>,
    pub project: Vec<String>,
}

impl From<&Filters> for FiltersMessage {
    fn from(filters: &Filters) -> Self {
        Self {
            status: filters.status.clone(),
            priority: filters.priority.clone(),
            assignee: filters.assignee.clone(),
            labels: filters.labels.clone(),
            project: filters.project.clone(),
        }
    }
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
    /// Which list the page looks at: `mine`, `all`, `team`, `cycle`, `view`, `project`, `label` or
    /// `search`; and Linear's id of it, for the ones that have one (the words, for a search).
    pub scope: &'static str,
    #[serde(rename = "listId")]
    pub list_id: Option<String>,
    /// Whether that list has come: `ready`, `loading` (it is being asked for) or `failed`.
    pub listing: &'static str,
    /// What Linear narrows the list by.
    pub filters: FiltersMessage,
    /// The words the person typed in the field. Of a search they are the words of the search, and
    /// do not narrow what it found.
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
    pub catalog: CatalogMessage,
}

/// The branch Linear suggests for an issue, as one line with no control character in it, or none.
pub fn branch_name(issue: &Issue) -> Option<String> {
    issue
        .branch_name
        .as_deref()
        .map(plain)
        .filter(|name| !name.is_empty())
}

fn row(issue: &Issue, scope: &Scope) -> RowMessage {
    RowMessage {
        id: issue.id.clone(),
        identifier: issue.identifier.clone(),
        title: issue.title.clone(),
        url: plain(&issue.url),
        branch_name: branch_name(issue),
        priority: issue.priority.name(),
        updated: short_date(&issue.updated_at),
        labels: labels(issue),
        // Whose it is is said where the list is not the person's own.
        assignee: match (scope, &issue.assignee) {
            (Scope::Mine, _) | (_, None) => None,
            (_, Some(who)) if who.is_me => Some("you".to_owned()),
            (_, Some(who)) => Some(who.display_name.clone()),
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
    let scope = &state.view.scope;
    // The words of a search were sent to Linear, whose ranking is not a text match: nothing found is
    // let go of for not having them.
    let filter = match scope {
        Scope::Search(_) => Filter::new(""),
        _ => Filter::new(&state.view.filter),
    };
    let mut groups = Vec::new();
    let mut matched = 0;
    let held = state
        .lists
        .as_ref()
        .and_then(|lists| lists.of(scope, &state.view.filters));
    if let Some(listing) = held {
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
            mine_cut: false,
            shown: 0,
            shown_cut: false,
            shown_open: 0,
        },
        |lists| Counts {
            mine: lists.mine.issues.len(),
            mine_cut: lists.mine.truncated,
            shown: held.map_or(0, |listing| listing.issues.len()),
            shown_cut: held.is_some_and(|listing| listing.truncated),
            shown_open: held.map_or(0, Listing::open),
        },
    );
    // The list the page looks at: here, or on its way, or one Linear could not tell.
    let listing = match (&state.lists, held) {
        (None, _) => "loading",
        (Some(_), Some(_)) => "ready",
        (Some(_), None) if state.refreshing || state.refresh_wanted_now() => "loading",
        (Some(_), None) => "failed",
    };
    let catalog = CatalogMessage {
        loaded: state.names.is_some(),
        error: state.names_error.as_ref().map(|error| ErrorMessage {
            kind: error.kind(),
            message: error.to_string(),
        }),
        items: state
            .names
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(item)
            .collect(),
    };
    Snapshot {
        loaded: state.lists.is_some(),
        refreshing: state.refreshing,
        error: state.error.as_ref().map(|error| ErrorMessage {
            kind: error.kind(),
            message: error.to_string(),
        }),
        scope: scope.kind(),
        list_id: scope.id().map(str::to_owned),
        listing,
        filters: FiltersMessage::from(&state.view.filters),
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
        catalog,
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
        let value = serde_json::to_value(row(&fixture_issue(), &Scope::Mine)).unwrap();
        assert_eq!(value["teamId"], "team-acm");
        assert_eq!(value["stateId"], "s1");
        assert_eq!(value["assigneeId"], "u-ada");
        assert!(value["projectId"].is_null());
        assert_eq!(value["labels"][0]["id"], "l-api");
        // Whose it is is said in the list of all issues alone; the id always.
        assert!(value["assignee"].is_null());
    }
}
