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

/// A cycle an issue of the team can be put in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleChoice {
    pub id: String,
    pub number: u32,
    pub name: Option<String>,
    /// The cycle that is running now.
    pub active: bool,
}

/// A number of points an issue of the team can be given, and what the team calls it (`5`, or `M`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateChoice {
    /// What is sent: a t-shirt size is the Fibonacci number it stands for.
    pub value: u32,
    pub label: String,
}

/// The most cycles offered besides the active one: the ones that come next.
const CYCLES_NEXT: usize = 6;

/// What a new issue of one team can be given.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub team: String,
    /// The state it begins in unless the person chooses another: the team's own, else the first
    /// unstarted, else the first backlog. Always one of `states`.
    pub default_state: Option<String>,
    /// The open states: triage, backlog, unstarted, then started; within a kind by position.
    pub states: Vec<StateChoice>,
    /// Every state an issue can be put in, as the same order goes on: the open ones, then completed
    /// and canceled. A duplicate is not one (it wants the issue it duplicates).
    pub all_states: Vec<StateChoice>,
    /// The team's labels and the workspace's, by name; no group.
    pub labels: Vec<LabelChoice>,
    /// The projects that are not finished or canceled, by name.
    pub projects: Vec<ProjectChoice>,
    /// The people who are active, by name.
    pub members: Vec<Member>,
    /// Whether the team has cycles; where it has not, an issue of it has no cycle to choose.
    pub cycles_enabled: bool,
    /// The active cycle, then the ones that come next by number. None where the team has no cycles.
    pub cycles: Vec<CycleChoice>,
    /// The team's scale of estimates, smallest first; none where it does not estimate.
    pub estimates: Vec<EstimateChoice>,
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
struct WireCycle {
    id: String,
    number: f64,
    #[serde(default)]
    name: Option<String>,
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
    #[serde(default)]
    cycles_enabled: bool,
    #[serde(default)]
    active_cycle: Option<WireCycle>,
    #[serde(default)]
    cycles: Option<Nodes<WireCycle>>,
    /// `notUsed`, `exponential`, `fibonacci`, `linear` or `tShirt`.
    #[serde(default)]
    issue_estimation_type: Option<String>,
    #[serde(default)]
    issue_estimation_allow_zero: bool,
    #[serde(default)]
    issue_estimation_extended: bool,
}

/// The scale of a way of estimating, smallest first: what is sent and what it is called. A t-shirt
/// size is sent as the Fibonacci number it stands for. Zero comes first where the team allows it.
fn scale(kind: &str, allow_zero: bool, extended: bool) -> Vec<EstimateChoice> {
    let numbers = |base: &[u32], more: &[u32]| -> Vec<(u32, String)> {
        base.iter()
            .chain(if extended { more } else { &[] })
            .map(|n| (*n, n.to_string()))
            .collect()
    };
    let mut sizes: Vec<(u32, String)> = match kind {
        "exponential" => numbers(&[1, 2, 4, 8, 16], &[32, 64]),
        "fibonacci" => numbers(&[1, 2, 3, 5, 8], &[13, 21]),
        "linear" => numbers(&[1, 2, 3, 4, 5], &[6, 7]),
        "tShirt" => [(1, "XS"), (2, "S"), (3, "M"), (5, "L"), (8, "XL")]
            .into_iter()
            .chain(if extended {
                vec![(13, "XXL"), (21, "XXXL")]
            } else {
                Vec::new()
            })
            .map(|(value, label)| (value, label.to_owned()))
            .collect(),
        _ => return Vec::new(),
    };
    if allow_zero {
        sizes.insert(0, (0, "0".to_owned()));
    }
    sizes
        .into_iter()
        .map(|(value, label)| EstimateChoice { value, label })
        .collect()
}

/// A cycle read from the wire; a number that is not a count of cycles is no cycle.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn cycle_of(wire: WireCycle, active: bool) -> Option<CycleChoice> {
    (wire.number.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&wire.number)).then(|| {
        CycleChoice {
            id: wire.id,
            number: wire.number as u32,
            name: wire
                .name
                .map(|name| plain(&name))
                .filter(|name| !name.is_empty()),
            active,
        }
    })
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
        // The kinds are offered in the order a person reads them: what has not been triaged, what
        // waits, what is next, what is being done, what is done, what is dropped. A new issue begins
        // in one of the first four; a duplicate is of no kind that is offered, and a state of the
        // canceled kind that is named so is not either, for it wants the issue it duplicates.
        let order = |kind: StateKind| match kind {
            StateKind::Triage => Some(0),
            StateKind::Backlog => Some(1),
            StateKind::Unstarted => Some(2),
            StateKind::Started => Some(3),
            StateKind::Completed => Some(4),
            StateKind::Canceled => Some(5),
            StateKind::Other => None,
        };
        let mut all: Vec<(u8, f64, StateChoice)> = team
            .states
            .nodes
            .into_iter()
            .filter(|state| {
                !(state.kind == StateKind::Canceled
                    && state.name.trim().eq_ignore_ascii_case("duplicate"))
            })
            .filter_map(|state| {
                Some((
                    order(state.kind)?,
                    state.position,
                    StateChoice {
                        id: state.id,
                        name: plain(&state.name),
                        kind: state.kind,
                    },
                ))
            })
            .collect();
        all.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.total_cmp(&b.1))
                .then_with(|| a.2.id.cmp(&b.2.id))
        });
        let all_states: Vec<StateChoice> = all.into_iter().map(|(_, _, state)| state).collect();
        let states: Vec<StateChoice> = all_states
            .iter()
            .filter(|state| !matches!(state.kind, StateKind::Completed | StateKind::Canceled))
            .cloned()
            .collect();
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

        // The cycle that runs, then the next ones by number: where the team has cycles at all.
        let mut cycles: Vec<CycleChoice> = Vec::new();
        if team.cycles_enabled {
            let running = team.active_cycle.and_then(|cycle| cycle_of(cycle, true));
            let mut next: Vec<CycleChoice> = team
                .cycles
                .into_iter()
                .flat_map(|nodes| nodes.nodes)
                .filter_map(|cycle| cycle_of(cycle, false))
                .filter(|cycle| running.as_ref().is_none_or(|run| run.id != cycle.id))
                .collect();
            next.sort_by(|a, b| a.number.cmp(&b.number).then_with(|| a.id.cmp(&b.id)));
            next.dedup_by(|a, b| a.id == b.id);
            next.truncate(CYCLES_NEXT);
            cycles.extend(running);
            cycles.extend(next);
        }
        let estimates = team
            .issue_estimation_type
            .as_deref()
            .map_or_else(Vec::new, |kind| {
                scale(
                    kind,
                    team.issue_estimation_allow_zero,
                    team.issue_estimation_extended,
                )
            });

        Some(Options {
            team: team.id,
            default_state,
            states,
            all_states,
            labels,
            projects,
            members,
            cycles_enabled: team.cycles_enabled,
            cycles,
            estimates,
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
