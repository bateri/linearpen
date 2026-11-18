//! What the commands print with `--json`: plain structures that serde writes, one for each thing a
//! command tells. The fields are the ones `docs/cli.md` lists. JSON escapes every control character,
//! so Linear's text goes in as it came.

use serde::Serialize;

use crate::linear::types::{Comment, Neighbor, Tie};
use crate::linear::{Issue, Listing};

#[derive(Debug, Serialize)]
pub struct State {
    pub name: String,
    /// `triage`, `backlog`, `unstarted`, `started`, `completed`, `canceled` or `other`.
    pub kind: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Person {
    pub name: String,
    pub me: bool,
}

#[derive(Debug, Serialize)]
pub struct Cycle {
    pub number: u32,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IssueRow {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub state: State,
    /// `none`, `urgent`, `high`, `medium` or `low`.
    pub priority: &'static str,
    pub assignee: Option<Person>,
    pub labels: Vec<String>,
    /// The key of the team: `ACM`.
    pub team: Option<String>,
    pub project: Option<String>,
    pub cycle: Option<Cycle>,
    pub estimate: Option<u32>,
    /// `2026-11-20`.
    pub due_date: Option<String>,
    pub updated_at: String,
    pub branch_name: Option<String>,
}

impl IssueRow {
    pub fn of(issue: &Issue) -> Self {
        Self {
            id: issue.id.clone(),
            identifier: issue.identifier.clone(),
            title: issue.title.clone(),
            url: issue.url.clone(),
            state: State {
                name: issue.state.name.clone(),
                kind: issue.state.kind.name(),
            },
            priority: issue.priority.name(),
            assignee: issue.assignee.as_ref().map(|who| Person {
                name: who.display_name.clone(),
                me: who.is_me,
            }),
            labels: issue
                .labels
                .iter()
                .map(|label| label.name.clone())
                .collect(),
            team: issue.team.as_ref().map(|team| team.key.clone()),
            project: issue.project.as_ref().map(|project| project.name.clone()),
            cycle: issue.cycle.as_ref().map(|cycle| Cycle {
                number: cycle.number,
                name: cycle.name.clone(),
            }),
            estimate: issue.estimate,
            due_date: issue.due_date.clone(),
            updated_at: issue.updated_at.clone(),
            branch_name: issue.branch_name.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Issues {
    /// What the list is called (`My issues`).
    pub list: String,
    /// There are more issues in Linear than the ones listed.
    pub truncated: bool,
    pub issues: Vec<IssueRow>,
}

impl Issues {
    pub fn of(list: &str, listing: &Listing) -> Self {
        Self {
            list: list.to_owned(),
            truncated: listing.truncated,
            issues: listing.issues.iter().map(IssueRow::of).collect(),
        }
    }
}

/// The issues a change left, as Linear says they are now.
#[derive(Debug, Serialize)]
pub struct Changed {
    pub issues: Vec<IssueRow>,
}

#[derive(Debug, Serialize)]
pub struct Made {
    pub id: String,
    pub identifier: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct Neighbour {
    pub identifier: String,
    pub title: String,
    pub state: State,
}

impl Neighbour {
    fn of(other: &Neighbor) -> Self {
        Self {
            identifier: other.identifier.clone(),
            title: other.title.clone(),
            state: State {
                name: other.state.name.clone(),
                kind: other.state.kind.name(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Tied {
    /// `blocks`, `blocked_by`, `related`, `duplicate_of` or `duplicated_by`.
    pub tie: &'static str,
    pub issue: Neighbour,
}

#[derive(Debug, Serialize)]
pub struct Said {
    pub author: Option<String>,
    pub created_at: String,
    pub body: String,
}

impl Said {
    pub fn of(comment: &Comment) -> Self {
        Self {
            author: comment.user.as_ref().map(|who| who.display_name.clone()),
            created_at: comment.created_at.clone(),
            body: comment.body.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub issue: IssueRow,
    pub description: Option<String>,
    pub parent: Option<Neighbour>,
    pub children: Vec<Neighbour>,
    pub ties: Vec<Tied>,
    pub comments: Vec<Said>,
    /// There are more comments in Linear than the ones given.
    pub comments_truncated: bool,
}

fn tie_name(tie: Tie) -> &'static str {
    match tie {
        Tie::Blocks => "blocks",
        Tie::BlockedBy => "blocked_by",
        Tie::Related => "related",
        Tie::DuplicateOf => "duplicate_of",
        Tie::DuplicatedBy => "duplicated_by",
    }
}

impl Shown {
    pub fn of(issue: &Issue) -> Self {
        Self {
            issue: IssueRow::of(issue),
            description: issue.description.clone(),
            parent: issue.parent.as_ref().map(Neighbour::of),
            children: issue.children.iter().map(Neighbour::of).collect(),
            ties: issue
                .ties()
                .into_iter()
                .map(|(tie, other)| Tied {
                    tie: tie_name(tie),
                    issue: Neighbour::of(other),
                })
                .collect(),
            comments: issue
                .comments
                .as_ref()
                .map(|comments| comments.nodes.iter().map(Said::of).collect())
                .unwrap_or_default(),
            comments_truncated: issue
                .comments
                .as_ref()
                .is_some_and(|comments| comments.page_info.has_next_page),
        }
    }
}

/// The comment that was added.
#[derive(Debug, Serialize)]
pub struct Added {
    pub issue: String,
    pub comment: Said,
}

#[derive(Debug, Serialize)]
pub struct TeamRow {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CycleRow {
    pub number: u32,
    pub label: String,
    pub running: bool,
}

#[derive(Debug, Serialize)]
pub struct Size {
    pub label: String,
    pub points: u32,
}

/// What a team's issues can be given: the names to write in the options.
#[derive(Debug, Serialize)]
pub struct TeamNames {
    pub key: String,
    pub name: String,
    pub statuses: Vec<State>,
    pub labels: Vec<String>,
    pub people: Vec<Person>,
    pub projects: Vec<String>,
    pub cycles: Vec<CycleRow>,
    pub estimates: Vec<Size>,
}
