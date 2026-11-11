//! What a new issue is made of: the teams it can be made in, what each team offers to give it (its
//! states, labels, projects and members), the input of the mutation, and what Linear says it made.
//!
//! What Linear sends is shown to the person and never trusted: names go through `plain`. What is
//! offered is only what can be chosen: groups of labels, finished states and projects, and people who
//! have left are not.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize, Serializer};

use super::error::plain;
use super::types::{Priority, StateKind};

/// A team an issue can be made in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    pub id: String,
    /// `ACM`, which an issue's identifier begins with.
    pub key: String,
    pub name: String,
}

/// A state a new issue can begin in.
#[derive(Debug, Clone, PartialEq)]
pub struct StateChoice {
    pub id: String,
    pub name: String,
    pub kind: StateKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelChoice {
    pub id: String,
    pub name: String,
    /// `#rrggbb`, Linear's own colour of the label.
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectChoice {
    pub id: String,
    pub name: String,
}

/// Someone an issue can be given to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: String,
    pub name: String,
    /// The person whose key it is.
    pub me: bool,
}

/// What a new issue of one team can be given.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub team: String,
    /// The state it begins in unless the person chooses another: the team's own, else the first
    /// unstarted, else the first backlog. Always one of `states`.
    pub default_state: Option<String>,
    /// The open states: triage, backlog, unstarted, then started; within a kind by position.
    pub states: Vec<StateChoice>,
    /// The team's labels and the workspace's, by name; no group.
    pub labels: Vec<LabelChoice>,
    /// The projects that are not finished or canceled, by name.
    pub projects: Vec<ProjectChoice>,
    /// The people who are active, by name.
    pub members: Vec<Member>,
}

/// The issue to make. The `id` is the id of the draft: Linear takes it as the id of the issue, so
/// that an issue whose answer was lost can be asked for.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewIssue {
    pub id: String,
    pub team_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_id: Option<String>,
    #[serde(
        skip_serializing_if = "Priority::is_none",
        serialize_with = "priority_number"
    )]
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub label_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

fn priority_number<S: Serializer>(priority: &Priority, out: S) -> Result<S::Ok, S::Error> {
    out.serialize_u8(priority.number())
}

/// What Linear says it made.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Created {
    pub id: String,
    pub identifier: String,
}

// What the wire holds. Nothing here is offered as it is: `teams` and `Options::of` make the values
// above of it.

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
pub(super) struct TeamsData {
    teams: Nodes<WireTeam>,
}

#[derive(Deserialize)]
struct WireTeam {
    id: String,
    key: String,
    name: String,
}

#[derive(Deserialize)]
struct IdOnly {
    id: String,
}

#[derive(Deserialize)]
struct WireState {
    id: String,
    name: String,
    #[serde(rename = "type")]
    kind: StateKind,
    #[serde(default)]
    position: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireLabel {
    id: String,
    name: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    is_group: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireProject {
    id: String,
    name: String,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    canceled_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireMember {
    id: String,
    display_name: String,
    #[serde(default)]
    is_me: bool,
    #[serde(default = "yes")]
    active: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireOptionsTeam {
    id: String,
    #[serde(default)]
    default_issue_state: Option<IdOnly>,
    states: Nodes<WireState>,
    labels: Nodes<WireLabel>,
    projects: Nodes<WireProject>,
    members: Nodes<WireMember>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptionsData {
    team: Option<WireOptionsTeam>,
    issue_labels: Nodes<WireLabel>,
}

/// Case-blind order of names, then the id so that the order is the same however Linear sent them.
fn by_name(a: (&str, &str), b: (&str, &str)) -> Ordering {
    a.0.to_lowercase()
        .cmp(&b.0.to_lowercase())
        .then_with(|| a.0.cmp(b.0))
        .then_with(|| a.1.cmp(b.1))
}

impl TeamsData {
    /// The teams, by name.
    pub(super) fn teams(self) -> Vec<Team> {
        let mut teams: Vec<Team> = self
            .teams
            .nodes
            .into_iter()
            .map(|team| Team {
                id: team.id,
                key: plain(&team.key),
                name: plain(&team.name),
            })
            .collect();
        teams.sort_by(|a, b| by_name((&a.name, &a.id), (&b.name, &b.id)));
        teams
    }
}

impl OptionsData {
    /// What can be chosen, or nothing where Linear has no such team.
    pub(super) fn options(self) -> Option<Options> {
        let team = self.team?;
        // Only the open kinds are offered, in the order a person reads them: what has not been
        // triaged, what waits, what is next, what is being done.
        let open = |kind: StateKind| match kind {
            StateKind::Triage => Some(0),
            StateKind::Backlog => Some(1),
            StateKind::Unstarted => Some(2),
            StateKind::Started => Some(3),
            _ => None,
        };
        let mut states: Vec<(u8, f64, StateChoice)> = team
            .states
            .nodes
            .into_iter()
            .filter_map(|state| {
                Some((
                    open(state.kind)?,
                    state.position,
                    StateChoice {
                        id: state.id,
                        name: plain(&state.name),
                        kind: state.kind,
                    },
                ))
            })
            .collect();
        states.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then_with(|| a.2.id.cmp(&b.2.id))
        });
        let states: Vec<StateChoice> = states.into_iter().map(|(_, _, state)| state).collect();
        let first_of = |kind: StateKind| states.iter().find(|state| state.kind == kind);
        let default_state = team
            .default_issue_state
            .map(|state| state.id)
            .filter(|id| states.iter().any(|state| &state.id == id))
            .or_else(|| first_of(StateKind::Unstarted).map(|state| state.id.clone()))
            .or_else(|| first_of(StateKind::Backlog).map(|state| state.id.clone()));

        let mut labels: Vec<LabelChoice> = Vec::new();
        for label in team.labels.nodes.into_iter().chain(self.issue_labels.nodes) {
            if label.is_group || labels.iter().any(|kept| kept.id == label.id) {
                continue;
            }
            labels.push(LabelChoice {
                id: label.id,
                name: plain(&label.name),
                color: plain(&label.color),
            });
        }
        labels.sort_by(|a, b| by_name((&a.name, &a.id), (&b.name, &b.id)));

        let mut projects: Vec<ProjectChoice> = team
            .projects
            .nodes
            .into_iter()
            .filter(|project| project.completed_at.is_none() && project.canceled_at.is_none())
            .map(|project| ProjectChoice {
                id: project.id,
                name: plain(&project.name),
            })
            .collect();
        projects.sort_by(|a, b| by_name((&a.name, &a.id), (&b.name, &b.id)));

        let mut members: Vec<Member> = team
            .members
            .nodes
            .into_iter()
            .filter(|member| member.active)
            .map(|member| Member {
                id: member.id,
                name: plain(&member.display_name),
                me: member.is_me,
            })
            .collect();
        members.sort_by(|a, b| by_name((&a.name, &a.id), (&b.name, &b.id)));

        Some(Options {
            team: team.id,
            default_state,
            states,
            labels,
            projects,
            members,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct CreateData {
    #[serde(rename = "issueCreate")]
    issue_create: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    issue: Option<Created>,
}

impl CreateData {
    /// The issue, where Linear says it made one.
    pub(super) fn created(self) -> Option<Created> {
        self.issue_create
            .filter(|payload| payload.success)
            .and_then(|payload| payload.issue)
    }
}
