//! The calls `work_options` and `start`: handing issues to an agent in a pane of bateri. What
//! happens is `work/`'s; this reads what the page asks, finds the issues, and tells what came.

use std::collections::HashSet;
use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::change::said;
use super::issue::check_id;
use super::{Failure, call};
use crate::linear::{Change, Error, Issue};
use crate::session::{Session, now_seconds};
use crate::work::agents::{self, Agent};
use crate::work::bateri::Place;
use crate::work::{self, Env, Folder, Refused, Request, progress};

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
    /// Move the issues that have not begun to their team's first state of work in progress, once
    /// the pane is made.
    #[serde(default)]
    progress: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartedMessage {
    pane: String,
    folder: String,
    context: String,
    /// How many issues were moved.
    moved: usize,
    /// Why some that were to move were not.
    not_moved: Option<String>,
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
    // Only now, with the pane made: a start that came to nothing leaves Linear as it was.
    let moved = if params.progress {
        move_forward(session, &issues)
    } else {
        Moved::default()
    };
    Ok(StartedMessage {
        pane: started.pane,
        folder: started.folder.display().to_string(),
        context: started.context.display().to_string(),
        moved: moved.count,
        not_moved: moved.not,
    })
}

/// What moving the issues came to.
#[derive(Debug, Default)]
struct Moved {
    count: usize,
    /// Why some that were to move were not.
    not: Option<String>,
}

/// Why the issues were not moved, in the words of why a change was not made.
fn not_moved(error: &Error) -> String {
    let words = said(error);
    match words.strip_prefix("Linear did not change it") {
        Some(rest) => format!("Linear did not move the issues{rest}"),
        None => words,
    }
}

/// Moves the issues that have not begun to the first state of work in progress of their team, one
/// change for each team: the one status write of `change`, sent once, never again. Where it cannot
/// be done the agent is started all the same, and the words say why.
fn move_forward(session: &Session, issues: &[Issue]) -> Moved {
    let mut moved = Moved::default();
    let mut reasons: Vec<String> = Vec::new();
    for step in progress::steps(issues) {
        let state = match session.options(&step.team) {
            Ok(options) => match progress::first_started(&options) {
                Some(state) => state.id.clone(),
                None => {
                    reasons.push(format!(
                        "The {} team has no state of work in progress to move its issues to",
                        step.key
                    ));
                    continue;
                }
            },
            Err(error) => {
                reasons.push(not_moved(&error));
                continue;
            }
        };
        match session.change(&step.ids, &Change::State(state)) {
            Ok(changed) => moved.count += changed.len(),
            Err(error) => reasons.push(not_moved(&error)),
        }
    }
    reasons.dedup();
    if !reasons.is_empty() {
        moved.not = Some(reasons.join("; "));
    }
    moved
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

    // --- `progress`: what a start moves in Linear ---------------------------------------------------

    use crate::page_tests::{Canned, Change as Answer, Ledger, Mode, rig, rig_polling};
    use serde_json::json;

    fn hand(ids: &[&str], progress: bool) -> Hand {
        Hand {
            ids: ids.iter().map(|id| (*id).to_owned()).collect(),
            prompt: String::new(),
            agent: "claude".to_owned(),
            folder: "here".to_owned(),
            open: "split".to_owned(),
            progress,
        }
    }

    /// Linear, a machine with `claude` and bateri, and what a start needs of them.
    fn machine() -> (Arc<Canned>, crate::page_tests::Rig) {
        let linear = Canned::new(Mode::Fine);
        let rig = rig(&linear, true, &["claude"]);
        (linear, rig)
    }

    // The fixtures: issue-415 is in Todo, issue-412 and issue-398 are in In Progress.

    #[test]
    fn progress_moves_an_issue_that_has_not_begun_to_the_first_state_of_work_and_only_that() {
        let (linear, rig) = machine();
        let started = start(&rig.session, hand(&["issue-415", "issue-412"], true)).unwrap();
        assert_eq!(started.moved, 1);
        assert_eq!(started.not_moved, None);
        // One status write, of the one issue that had not begun, to In Progress and to nothing
        // else: no other field of an issue goes with it.
        assert_eq!(
            linear.changed(),
            [json!({ "ids": ["issue-415"], "input": { "stateId": "s-prog" } })]
        );
        rig.hands.end();
    }

    #[test]
    fn without_progress_no_mutation_is_sent() {
        let (linear, rig) = machine();
        let started = start(&rig.session, hand(&["issue-415"], false)).unwrap();
        assert_eq!((started.moved, started.not_moved), (0, None));
        assert!(linear.changed().is_empty());
        assert_eq!(linear.times("IssueBatchUpdate"), 0);
        rig.hands.end();
    }

    #[test]
    fn with_progress_and_nothing_to_move_nothing_is_sent() {
        let (linear, rig) = machine();
        let started = start(&rig.session, hand(&["issue-412", "issue-398"], true)).unwrap();
        assert_eq!((started.moved, started.not_moved), (0, None));
        assert_eq!(linear.times("IssueBatchUpdate"), 0, "not even an empty one");
        rig.hands.end();
    }

    #[test]
    fn issues_of_two_teams_are_two_changes_each_to_its_own_team() {
        let (linear, rig) = machine();
        linear.set_team_of("issue-420", "team-web", "WEB");
        let started = start(
            &rig.session,
            hand(&["issue-415", "issue-420", "issue-418"], true),
        )
        .unwrap();
        assert_eq!(started.moved, 3);
        let sent = linear.changed();
        assert_eq!(sent.len(), 2, "{sent:?}");
        assert_eq!(sent[0]["ids"], json!(["issue-415", "issue-418"]));
        assert_eq!(sent[1]["ids"], json!(["issue-420"]));
        for each in &sent {
            assert_eq!(each["input"].as_object().unwrap().len(), 1, "{each}");
            assert!(each["input"]["stateId"].is_string(), "{each}");
        }
        rig.hands.end();
    }

    #[test]
    fn the_change_goes_after_the_pane_is_made() {
        let (linear, rig) = machine();
        let ledger: Ledger = Ledger::default();
        linear.keep_ledger(&ledger);
        rig.hands.keep_ledger(&ledger);
        start(&rig.session, hand(&["issue-415"], true)).unwrap();
        let seen = ledger.lock().unwrap().clone();
        let at = |what: &str| seen.iter().position(|line| line.starts_with(what));
        let pane = at("bateri split").unwrap_or_else(|| panic!("a pane was made: {seen:?}"));
        let change = at("linear IssueBatchUpdate")
            .unwrap_or_else(|| panic!("the issue was moved: {seen:?}"));
        assert!(pane < change, "{seen:?}");
        rig.hands.end();
    }

    #[test]
    fn where_no_pane_is_made_nothing_is_written_to_linear() {
        let (linear, rig) = machine();
        rig.hands.refuse_the_pane("the bateri app is not connected");
        let failed = start(&rig.session, hand(&["issue-415"], true));
        assert!(matches!(failed, Err(Failure::StartFailed(_))), "{failed:?}");
        assert_eq!(linear.times("IssueBatchUpdate"), 0);
    }

    #[test]
    fn a_change_linear_refuses_leaves_the_start_whole_and_says_why_and_is_not_sent_again() {
        let (linear, rig) = machine();
        linear.set_change(Answer::Refused);
        let started = start(&rig.session, hand(&["issue-415"], true)).unwrap();
        assert!(started.pane.starts_with('p'));
        assert_eq!(started.moved, 0);
        let why = started.not_moved.expect("the reason is told");
        assert!(why.starts_with("Linear did not move the issues"), "{why}");
        assert_eq!(linear.times("IssueBatchUpdate"), 1);
        rig.hands.end();
    }

    #[test]
    fn a_change_whose_answer_was_lost_is_told_and_not_sent_again() {
        let (linear, rig) = machine();
        linear.set_change(Answer::Lost);
        let started = start(&rig.session, hand(&["issue-415"], true)).unwrap();
        assert_eq!(started.moved, 0);
        assert!(started.not_moved.is_some());
        assert_eq!(linear.times("IssueBatchUpdate"), 1);
        rig.hands.end();
    }

    #[test]
    fn progress_is_a_boolean_and_nothing_else_is_taken() {
        use crate::api::params::parse;
        let base = json!({ "ids": ["i"], "agent": "claude", "folder": "here", "open": "split" });
        assert!(!parse::<Hand>(base.clone()).unwrap().progress);
        let mut with = base.clone();
        with["progress"] = json!(true);
        assert!(parse::<Hand>(with).unwrap().progress);
        let mut bad = base.clone();
        bad["progress"] = json!("yes");
        assert!(parse::<Hand>(bad).is_err());
        let mut more = base;
        more["state"] = json!("s-done");
        assert!(
            parse::<Hand>(more).is_err(),
            "no state is named by the page"
        );
    }

    // --- Looking at the panes: which agent has stopped ---------------------------------------------

    /// Now, as `output_at` is told: seconds since the epoch.
    fn now() -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }

    /// What `bateri ls --json` says of the pane p41 when it last wrote `ago` seconds ago.
    fn ls_of(ago: f64) -> String {
        json!({ "terminals": [
            { "id": "p40", "state": "running", "activity": "running", "output_at": now() },
            { "id": "p41", "state": "running", "activity": "running", "program": "claude",
              "exit": null, "output_at": now() - ago, "label": "ACM-412" },
        ] })
        .to_string()
    }

    /// The machine, with the panes looked at every 15 ms, and an agent started on ACM-412.
    fn watched() -> (crate::page_tests::Rig, Arc<Canned>) {
        let linear = Canned::new(Mode::Fine);
        let rig = rig_polling(
            &linear,
            true,
            &["claude"],
            std::time::Duration::from_millis(15),
        );
        rig.hands.set_ls(&ls_of(0.0));
        start(&rig.session, hand(&["issue-412"], false)).unwrap();
        (rig, linear)
    }

    /// Whether the issue is marked as one whose agent has stopped (`None` where it is not marked).
    fn idle(rig: &crate::page_tests::Rig) -> Option<bool> {
        rig.session
            .state()
            .working
            .get("issue-412")
            .map(|working| working.idle)
    }

    /// Waits until `what` holds, for a few seconds at most.
    fn until(what: &str, mut holds: impl FnMut() -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !holds() {
            assert!(std::time::Instant::now() < end, "it did not come: {what}");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn a_pane_that_wrote_lately_is_at_work_and_one_silent_for_ten_seconds_has_stopped_until_it_writes()
     {
        let (rig, _linear) = watched();
        // The pane is looked at again and again, and it is at work.
        until("the pane is looked at", || rig.hands.ls_calls() >= 3);
        assert_eq!(idle(&rig), Some(false));
        // Silent for a minute: the agent has stopped.
        rig.hands.set_ls(&ls_of(60.0));
        until("the agent has stopped", || idle(&rig) == Some(true));
        // Nine seconds is not ten; and writing again takes the word back.
        rig.hands.set_ls(&ls_of(9.0));
        until("the agent is at work again", || idle(&rig) == Some(false));
        rig.hands.set_ls(&ls_of(11.0));
        until("the agent has stopped again", || idle(&rig) == Some(true));
        rig.hands.set_ls(&ls_of(0.0));
        until("the agent writes", || idle(&rig) == Some(false));
        rig.hands.end();
    }

    #[test]
    fn a_list_that_is_broken_empty_or_without_the_pane_says_nothing_and_the_mark_stays() {
        let (rig, _linear) = watched();
        let says_nothing = [
            "not json at all".to_owned(),
            String::new(),
            r#"{"terminals":[]}"#.to_owned(),
            r#"{"terminals":3}"#.to_owned(),
            // The pane is not in the list, or has no time, or one that is no number.
            json!({ "terminals": [{ "id": "p9", "output_at": 1 }] }).to_string(),
            json!({ "terminals": [{ "id": "p41", "state": "running" }] }).to_string(),
            json!({ "terminals": [{ "id": "p41", "output_at": "a minute ago" }] }).to_string(),
            json!({ "terminals": [{ "id": "p41", "output_at": null }] }).to_string(),
        ];
        for text in says_nothing {
            // First it has stopped, and then the list says nothing of it: it is at work as before.
            rig.hands.set_ls(&ls_of(60.0));
            until("the agent has stopped", || idle(&rig) == Some(true));
            rig.hands.set_ls(&text);
            until("the list said nothing", || idle(&rig) == Some(false));
            // Never does the list take the mark away: it is there until the pane ends.
            let calls = rig.hands.ls_calls();
            until("it is looked at again", || {
                rig.hands.ls_calls() >= calls + 3
            });
            assert_eq!(idle(&rig), Some(false), "{text}");
        }
        rig.hands.end();
        until("the mark goes with the pane", || idle(&rig).is_none());
    }

    #[test]
    fn nothing_is_looked_at_where_no_pane_is_watched_and_no_more_when_the_last_ends() {
        let linear = Canned::new(Mode::Fine);
        let rig = rig_polling(
            &linear,
            true,
            &["claude"],
            std::time::Duration::from_millis(15),
        );
        rig.hands.set_ls(&ls_of(0.0));
        // A start that put no agent in a pane looks at nothing.
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(rig.hands.ls_calls(), 0);
        start(&rig.session, hand(&["issue-412"], false)).unwrap();
        until("the pane is looked at", || rig.hands.ls_calls() >= 2);
        rig.hands.end();
        until("the mark goes", || idle(&rig).is_none());
        // One more look may be on its way; after it no call is made.
        std::thread::sleep(std::time::Duration::from_millis(100));
        let calls = rig.hands.ls_calls();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(
            rig.hands.ls_calls(),
            calls,
            "the panes are looked at no more"
        );
        // A pane made after it is looked at again.
        rig.hands.begin_again();
        start(&rig.session, hand(&["issue-412"], false)).unwrap();
        until("the new pane is looked at", || {
            rig.hands.ls_calls() >= calls + 3
        });
        assert_eq!(idle(&rig), Some(false));
        rig.hands.end();
        until("the mark goes", || idle(&rig).is_none());
    }
}
