//! The calls `work_options` and `start`: handing issues to an agent in a pane of bateri. What
//! happens is `work/`'s; this reads what the page asks, finds the issues, and tells what came.

use std::collections::HashSet;
use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issue::check_id;
use super::{Failure, call};
use crate::linear::Issue;
use crate::session::{Session, now_seconds};
use crate::work::agents::{self, Agent};
use crate::work::bateri::Place;
use crate::work::{self, Env, Folder, Refused, Request};

/// The most issues handed over at once.
const IDS_MOST: usize = 25;

/// The most characters of the person's own words that go to the prompt.
const PROMPT_MOST: usize = work::handoff::PROMPT_MOST;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nothing {}

#[derive(Debug, Serialize)]
struct AgentMessage {
    id: &'static str,
    name: &'static str,
}

#[derive(Debug, Serialize)]
struct FolderMessage {
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct WorktreeMessage {
    available: bool,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct OptionsMessage {
    agents: Vec<AgentMessage>,
    unavailable: Option<String>,
    folder: FolderMessage,
    worktree: WorktreeMessage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hand {
    ids: Vec<String>,
    #[serde(default)]
    prompt: String,
    agent: String,
    folder: String,
    open: String,
}

#[derive(Debug, Serialize)]
struct StartedMessage {
    pane: String,
    folder: String,
    context: String,
}

/// The ids, checked: one to `IDS_MOST`, each one an id, none twice.
fn check_ids(ids: &[String]) -> Result<(), Failure> {
    if ids.is_empty() || ids.len() > IDS_MOST {
        return Err(Failure::BadParams(format!(
            "ids are one to {IDS_MOST} ids of issues"
        )));
    }
    let mut seen = HashSet::new();
    for id in ids {
        check_id(id)?;
        if !seen.insert(id) {
            return Err(Failure::BadParams(format!("{id} is named twice")));
        }
    }
    Ok(())
}

fn env(session: &Session) -> Env<'_> {
    Env {
        setup: &session.work.setup,
        runner: session.work.runner.as_ref(),
        state: session.paths.state.as_deref(),
        now: now_seconds(),
    }
}

fn options(session: &Session, _: Nothing) -> Result<OptionsMessage, Failure> {
    let setup = &session.work.setup;
    let worktree = match work::worktree_available(&env(session)) {
        Ok(()) => WorktreeMessage {
            available: true,
            reason: None,
        },
        Err(reason) => WorktreeMessage {
            available: false,
            reason: Some(reason),
        },
    };
    Ok(OptionsMessage {
        agents: agents::installed(&setup.path)
            .into_iter()
            .map(|agent| AgentMessage {
                id: agent.id(),
                name: agent.name(),
            })
            .collect(),
        unavailable: setup.unavailable(),
        folder: FolderMessage {
            name: setup
                .folder
                .as_deref()
                .and_then(|folder| folder.file_name())
                .and_then(|name| name.to_str())
                .map(str::to_owned),
        },
        worktree,
    })
}

fn start(session: &Session, params: Hand) -> Result<StartedMessage, Failure> {
    check_ids(&params.ids)?;
    if params.prompt.chars().count() > PROMPT_MOST {
        return Err(Failure::BadParams(format!(
            "prompt is at most {PROMPT_MOST} characters"
        )));
    }
    let agent = Agent::parse(&params.agent)
        .ok_or_else(|| Failure::BadParams("agent is claude, codex or cursor".to_owned()))?;
    let folder = Folder::parse(&params.folder)
        .ok_or_else(|| Failure::BadParams("folder is here or worktree".to_owned()))?;
    let place = Place::parse(&params.open)
        .ok_or_else(|| Failure::BadParams("open is split or tab".to_owned()))?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    // What cannot be done from here is said before Linear is asked for the text of each issue.
    if let Some(why) = session.work.setup.unavailable() {
        return Err(Failure::Unavailable(why));
    }
    let issues: Vec<Issue> = params
        .ids
        .iter()
        .map(|id| session.detail_fresh(id))
        .collect::<Result<_, _>>()
        .map_err(Failure::Linear)?;
    let started = work::start(
        &env(session),
        &Request {
            issues: &issues,
            prompt: &params.prompt,
            agent,
            folder,
            place,
        },
    )
    .map_err(|refused| match refused {
        Refused::Unavailable(why) => Failure::Unavailable(why),
        Refused::Failed(why) => Failure::StartFailed(why),
    })?;
    // The marks are on the ids the page sent: they are the ones it knows the issues by.
    session.watch(&params.ids, agent.name(), &started.pane);
    Ok(StartedMessage {
        pane: started.pane,
        folder: started.folder.display().to_string(),
        context: started.context.display().to_string(),
    })
}

pub fn calls(options_: Options, session: &Arc<Session>) -> Options {
    let options_ = call(options_, session, "work_options", options);
    call(options_, session, "start", start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ids_are_one_to_twenty_five_and_none_twice() {
        let ids = |n: usize| (0..n).map(|i| format!("issue-{i}")).collect::<Vec<_>>();
        assert!(check_ids(&ids(1)).is_ok());
        assert!(check_ids(&ids(25)).is_ok());
        assert!(check_ids(&[]).is_err());
        assert!(check_ids(&ids(26)).is_err());
        assert!(check_ids(&["a".to_owned(), "a".to_owned()]).is_err());
        assert!(check_ids(&["a b".to_owned()]).is_err());
        assert!(check_ids(&["../x".to_owned()]).is_err());
    }
}
