//! The agents at work on issues, as bateri's list says them: what `list` and `show` say of an issue
//! an agent works on, and `agent`, which takes the keyboard to its pane. An agent is found by the one
//! rule the page has (`work::at_work`): a pane that runs one, named with the issue's identifier.
//! Linear is asked nothing of it (but the identifier of an issue given by its id), and bateri nothing
//! where the program is not in one of its panes.

use std::collections::HashMap;

use super::{Access, Stop};
use crate::linear::plain;
use crate::work::agents::Agent;
use crate::work::bateri;
use crate::work::runner::Runner;

/// The way to bateri, given where the program runs in a pane of it, and the moment an agent's
/// silence is measured to, seconds since the epoch.
#[derive(Clone, Copy)]
pub struct Panes<'a> {
    pub runner: &'a dyn Runner,
    pub now: f64,
}

/// An agent at work on an issue.
#[derive(Debug, Clone, PartialEq)]
pub struct Working {
    pub agent: Agent,
    /// Its pane (`p41`).
    pub pane: String,
    /// Its pane has written nothing for a while: it is done, or it waits for the person.
    pub idle: bool,
}

impl Working {
    /// How a line of text says it: `Claude at work in p41`, `Claude idle in p41`.
    #[must_use]
    pub fn words(&self) -> String {
        let state = if self.idle { "idle" } else { "at work" };
        format!("{} {state} in {}", self.agent.name(), self.pane)
    }
}

/// The agents at work, by the identifier of the issue each is on (of two on one issue, the one
/// started last). Nothing where the program is not in bateri or bateri's list cannot be had: the
/// issues are then told as they are, with no agent.
#[must_use]
pub fn by_issue(panes: Option<&Panes>) -> HashMap<String, Working> {
    panes
        .and_then(|panes| Some(of(&bateri::terminals(panes.runner)?, panes.now)))
        .unwrap_or_default()
}

/// The agents at work in `terminals`, by the identifier of their issue, at `now`.
fn of(terminals: &[bateri::Terminal], now: f64) -> HashMap<String, Working> {
    let mut found = HashMap::new();
    for one in crate::work::at_work(terminals) {
        let working = Working {
            agent: one.agent,
            pane: one.terminal.id.clone(),
            idle: one
                .terminal
                .output_at
                .is_some_and(|at| bateri::idle(at, now)),
        };
        found.insert(one.label.to_owned(), working);
    }
    found
}

/// `agent KEY-N`: the keyboard goes to the pane of the agent at work on the issue `issue`, as a press
/// on its mark in the pane does. Linear's id of an issue is first asked of Linear as its identifier.
pub fn go(access: &Access, issue: &str) -> Result<String, Stop> {
    let Some(panes) = access.panes.as_ref() else {
        return Err(Stop::no(
            "linearpen is not in a bateri pane, which is where the agents are",
        ));
    };
    let identifier = if crate::state::draft::is_uuid(issue) {
        super::read::fetched(access, issue)?.identifier
    } else {
        issue.to_owned()
    };
    let terminals = bateri::terminals(panes.runner)
        .ok_or_else(|| Stop::no("bateri did not say which panes there are"))?;
    let working = of(&terminals, panes.now)
        .remove(&identifier)
        .ok_or_else(|| Stop::no(format!("No agent is at work on {identifier}")))?;
    bateri::focus(panes.runner, &working.pane).map_err(Stop::no)?;
    Ok(format!(
        "{}: went to {}, in pane {}\n",
        plain(&identifier),
        working.agent.name(),
        working.pane
    ))
}
