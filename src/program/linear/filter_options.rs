//! What the filters of a list offer to choose from: the names of the workflow states, the people and
//! the labels of the workspace, in one question. Only names and ids are read; what a name is called
//! is Linear's text and goes through `plain`. A workspace with more than 250 of one holds the first
//! 250.

use std::collections::HashSet;

use serde::Deserialize;

use super::error::plain;
use super::query;
use super::types::StateKind;
use super::{Error, Key, Transport, run};

/// A state, by the name the teams give it (each team has its own states, by the same names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateOption {
    pub name: String,
    pub kind: StateKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelOption {
    pub name: String,
    pub color: String,
}

/// The choices of the filters of status, assignee and labels, once each by its name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterOptions {
    pub states: Vec<StateOption>,
    pub people: Vec<Person>,
    pub labels: Vec<LabelOption>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
}

#[derive(Deserialize)]
struct WireState {
    name: String,
    #[serde(rename = "type")]
    kind: StateKind,
    #[serde(default)]
    position: f64,
}

#[derive(Deserialize)]
struct WirePerson {
    id: String,
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(default, rename = "isMe")]
    is_me: bool,
}

#[derive(Deserialize)]
struct WireLabel {
    name: String,
    #[serde(default)]
    color: String,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "workflowStates")]
    states: Nodes<WireState>,
    users: Nodes<WirePerson>,
    #[serde(rename = "issueLabels")]
    labels: Nodes<WireLabel>,
}

/// Where a kind of state stands in the menu: what Linear's own list of statuses has first.
fn order(kind: StateKind) -> u8 {
    match kind {
        StateKind::Triage => 0,
        StateKind::Backlog => 1,
        StateKind::Unstarted => 2,
        StateKind::Started => 3,
        StateKind::Completed => 4,
        StateKind::Canceled => 5,
        StateKind::Other => 6,
    }
}

/// The states, the people and the labels the filters offer.
///
/// # Errors
/// Where Linear cannot be asked, or does not answer as it should.
pub fn filter_options(transport: &dyn Transport, key: &Key) -> Result<FilterOptions, Error> {
    let data: Data = run::<(), Data>(transport, key, &query::FILTER_OPTIONS, None)?;
    let mut states = data.states.nodes;
    states.sort_by(|a, b| {
        order(a.kind)
            .cmp(&order(b.kind))
            .then(a.position.total_cmp(&b.position))
    });
    let mut seen = HashSet::new();
    let states = states
        .into_iter()
        .filter_map(|state| {
            let name = plain(&state.name);
            (!name.is_empty() && seen.insert(name.clone())).then_some(StateOption {
                name,
                kind: state.kind,
            })
        })
        .collect();
    let mut people: Vec<Person> = data
        .users
        .nodes
        .into_iter()
        .filter_map(|person| {
            let name = plain(&person.display_name);
            (!name.is_empty()).then_some(Person {
                id: person.id,
                name,
                me: person.is_me,
            })
        })
        .collect();
    // The person first, the others by name.
    people.sort_by(|a, b| {
        b.me.cmp(&a.me)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let mut seen = HashSet::new();
    let mut labels: Vec<LabelOption> = data
        .labels
        .nodes
        .into_iter()
        .filter_map(|label| {
            let name = plain(&label.name);
            (!name.is_empty() && seen.insert(name.clone())).then_some(LabelOption {
                name,
                color: plain(&label.color),
            })
        })
        .collect();
    labels.sort_by_key(|label| label.name.to_lowercase());
    Ok(FilterOptions {
        states,
        people,
        labels,
    })
}
