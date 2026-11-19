//! The calls `work_options` and `start`: handing an issue to an agent in a pane of bateri. What
//! happens is `work/`'s; this reads what the page asks, finds the issue, and tells what came.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::change::said;
use super::issue::check_id;
use super::{Failure, call};
use crate::linear::{Change, Error, Issue};
use crate::session::{Session, now_seconds};
use crate::state::prefs;
use crate::work::agents::{self, Agent};
use crate::work::bateri::Place;
use crate::work::{self, Env, Folder, Refused, Request, handoff, progress};

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
struct OptionsMessage {
    agents: Vec<AgentMessage>,
    unavailable: Option<String>,
    folder: FolderMessage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hand {
    /// The one issue handed over: Work on is an issue's own (a member `ids` is refused with any
    /// other that is not named).
    id: String,
    agent: String,
    folder: String,
    open: String,
    /// What the form's Status row said, where it had one: move the issue, if it has not begun, to
    /// its team's first state of work in progress, once the pane is made (`true`), or leave it
    /// (`false`). Not given where the form had no such row. Only `true` writes to Linear.
    progress: Option<bool>,
    /// What the person wrote for the agent, which ends its prompt: the person's own words, not
    /// Linear's. Not given, or only blank, where they wrote none.
    prompt: Option<String>,
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
    })
}

fn start(session: &Session, params: Hand) -> Result<StartedMessage, Failure> {
    check_id(&params.id)?;
    let agent = Agent::parse(&params.agent)
        .ok_or_else(|| Failure::BadParams("agent is claude, codex or cursor".to_owned()))?;
    let folder = Folder::parse(&params.folder)
        .ok_or_else(|| Failure::BadParams("folder is here or worktree".to_owned()))?;
    let place = Place::parse(&params.open)
        .ok_or_else(|| Failure::BadParams("open is split or tab".to_owned()))?;
    let words =
        handoff::words(params.prompt.as_deref().unwrap_or_default()).map_err(Failure::BadParams)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    // What cannot be done from here is said before Linear is asked for the text of the issue.
    if let Some(why) = session.work.setup.unavailable() {
        return Err(Failure::Unavailable(why));
    }
    let issue: Issue = session.detail_fresh(&params.id).map_err(Failure::Linear)?;
    let started = work::start(
        &env(session),
        &Request {
            issue: &issue,
            agent,
            folder,
            place,
            words: words.as_deref(),
        },
    )
    .map_err(|refused| match refused {
        Refused::Unavailable(why) => Failure::Unavailable(why),
        Refused::Failed(why) => Failure::StartFailed(why),
    })?;
    // The mark is on the id the page sent: it is the one it knows the issue by.
    session.watch(
        std::slice::from_ref(&params.id),
        agent.name(),
        &started.pane,
        &started.context,
    );
    // A run of a while ago is put away now and then: here, when a new one has been made.
    session.tidy();
    // Only now, with the pane made: a start that came to nothing leaves Linear as it was.
    let moved = if params.progress == Some(true) {
        move_forward(session, std::slice::from_ref(&issue))
    } else {
        Moved::default()
    };
    remember(session, agent, folder, place, params.progress);
    Ok(StartedMessage {
        pane: started.pane,
        folder: started.folder.display().to_string(),
        context: started.context.display().to_string(),
        moved: moved.count,
        not_moved: moved.not,
    })
}

/// What the form is opened with: the choices of the last start, each only where it is one the form
/// has (a word the file holds that it has not is as if the file did not say). Whether the agent is
/// installed is for the form to tell, with what `work_options` says.
#[derive(Debug, Default, Serialize)]
pub(super) struct Remembered {
    agent: Option<&'static str>,
    folder: Option<&'static str>,
    place: Option<&'static str>,
    status: Option<&'static str>,
}

/// The words of the choices, as the page and the file say them.
fn word_of_folder(folder: Folder) -> &'static str {
    match folder {
        Folder::Here => "here",
        Folder::Worktree => "worktree",
    }
}

fn word_of_place(place: Place) -> &'static str {
    match place {
        Place::Split => "split",
        Place::Tab => "tab",
    }
}

fn word_of_status(move_them: bool) -> &'static str {
    if move_them { "progress" } else { "keep" }
}

/// The choices of the last start, as the state folder keeps them.
pub(super) fn remembered(session: &Session) -> Remembered {
    let Some(state) = session.paths.state.as_deref() else {
        return Remembered::default();
    };
    let kept = prefs::load(state);
    Remembered {
        agent: kept.agent.as_deref().and_then(Agent::parse).map(Agent::id),
        folder: kept
            .folder
            .as_deref()
            .and_then(Folder::parse)
            .map(word_of_folder),
        place: kept
            .place
            .as_deref()
            .and_then(Place::parse)
            .map(word_of_place),
        status: match kept.status.as_deref() {
            Some("progress") => Some(word_of_status(true)),
            Some("keep") => Some(word_of_status(false)),
            _ => None,
        },
    }
}

/// Keeps the choices of a start that was made, for the next form. Best effort: the agent is started
/// whether or not they can be kept. Status is kept only where the form had the row (`progress` given).
fn remember(session: &Session, agent: Agent, folder: Folder, place: Place, progress: Option<bool>) {
    let Some(state) = session.paths.state.as_deref() else {
        return;
    };
    let mut kept = prefs::load(state);
    kept.agent = Some(agent.id().to_owned());
    kept.folder = Some(word_of_folder(folder).to_owned());
    kept.place = Some(word_of_place(place).to_owned());
    if let Some(move_them) = progress {
        kept.status = Some(word_of_status(move_them).to_owned());
    }
    let _ = prefs::save(state, &kept);
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
    fn the_id_is_an_id_and_the_prompt_is_words_or_nothing() {
        use crate::api::params::parse;
        let base =
            json!({ "id": "issue-412", "agent": "claude", "folder": "here", "open": "split" });
        assert!(parse::<Hand>(base.clone()).is_ok());
        // One issue: `ids` is a member no more, and nor is any other that is not named.
        for (name, value) in [("ids", json!(["issue-412"])), ("words", json!("Go."))] {
            let mut more = base.clone();
            more[name] = value;
            assert!(parse::<Hand>(more).is_err(), "{name} is refused");
        }
        // The prompt is words, or null, or not there.
        for (value, fine) in [
            (json!("Start with the limiter."), true),
            (json!(null), true),
            (json!(7), false),
            (json!(["Go."]), false),
        ] {
            let mut more = base.clone();
            more["prompt"] = value.clone();
            assert_eq!(parse::<Hand>(more).is_ok(), fine, "{value}");
        }
        let mut none = base.clone();
        none.as_object_mut().unwrap().remove("id");
        assert!(parse::<Hand>(none).is_err());
        for bad in ["", "a b", "../x"] {
            assert!(check_id(bad).is_err(), "{bad:?}");
        }
    }

    // --- `progress`: what a start moves in Linear ---------------------------------------------------

    use crate::page_tests::{Canned, Change as Answer, Ledger, Mode, rig, rig_polling};
    use serde_json::json;

    fn hand(id: &str, progress: bool) -> Hand {
        Hand {
            id: id.to_owned(),
            agent: "claude".to_owned(),
            folder: "here".to_owned(),
            open: "split".to_owned(),
            progress: Some(progress),
            prompt: None,
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
        let started = start(&rig.session, hand("issue-415", true)).unwrap();
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
        let started = start(&rig.session, hand("issue-415", false)).unwrap();
        assert_eq!((started.moved, started.not_moved), (0, None));
        assert!(linear.changed().is_empty());
        assert_eq!(linear.times("IssueBatchUpdate"), 0);
        rig.hands.end();
    }

    #[test]
    fn with_progress_and_nothing_to_move_nothing_is_sent() {
        let (linear, rig) = machine();
        let started = start(&rig.session, hand("issue-412", true)).unwrap();
        assert_eq!((started.moved, started.not_moved), (0, None));
        assert_eq!(linear.times("IssueBatchUpdate"), 0, "not even an empty one");
        rig.hands.end();
    }

    #[test]
    fn an_issue_of_another_team_is_moved_to_the_state_of_its_own_team() {
        let (linear, rig) = machine();
        linear.set_team_of("issue-420", "team-web", "WEB");
        let started = start(&rig.session, hand("issue-420", true)).unwrap();
        assert_eq!(started.moved, 1);
        let sent = linear.changed();
        assert_eq!(sent.len(), 1, "{sent:?}");
        assert_eq!(sent[0]["ids"], json!(["issue-420"]));
        assert_eq!(sent[0]["input"].as_object().unwrap().len(), 1, "{sent:?}");
        assert!(sent[0]["input"]["stateId"].is_string(), "{sent:?}");
        rig.hands.end();
    }

    #[test]
    fn the_change_goes_after_the_pane_is_made() {
        let (linear, rig) = machine();
        let ledger: Ledger = Ledger::default();
        linear.keep_ledger(&ledger);
        rig.hands.keep_ledger(&ledger);
        start(&rig.session, hand("issue-415", true)).unwrap();
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
        let failed = start(&rig.session, hand("issue-415", true));
        assert!(matches!(failed, Err(Failure::StartFailed(_))), "{failed:?}");
        assert_eq!(linear.times("IssueBatchUpdate"), 0);
    }

    #[test]
    fn a_change_linear_refuses_leaves_the_start_whole_and_says_why_and_is_not_sent_again() {
        let (linear, rig) = machine();
        linear.set_change(Answer::Refused);
        let started = start(&rig.session, hand("issue-415", true)).unwrap();
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
        let started = start(&rig.session, hand("issue-415", true)).unwrap();
        assert_eq!(started.moved, 0);
        assert!(started.not_moved.is_some());
        assert_eq!(linear.times("IssueBatchUpdate"), 1);
        rig.hands.end();
    }

    #[test]
    fn progress_is_a_boolean_and_nothing_else_is_taken() {
        use crate::api::params::parse;
        let base = json!({ "id": "i", "agent": "claude", "folder": "here", "open": "split" });
        assert_eq!(parse::<Hand>(base.clone()).unwrap().progress, None);
        let mut with = base.clone();
        with["progress"] = json!(true);
        assert_eq!(parse::<Hand>(with).unwrap().progress, Some(true));
        let mut without = base.clone();
        without["progress"] = json!(false);
        assert_eq!(parse::<Hand>(without).unwrap().progress, Some(false));
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

    // --- Old runs are put away ------------------------------------------------------------------

    /// A run of `days` days ago as `handoff::write` leaves it, in the state folder of the rig.
    fn old_run(rig: &crate::page_tests::Rig, days: u64, first: &str) -> std::path::PathBuf {
        let at = now_seconds() - days * 86_400;
        let dir = rig
            .state_dir()
            .join("work")
            .join(format!("{}-{first}", work::handoff::stamp(at)));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("issues.md"), "issues").unwrap();
        dir
    }

    #[test]
    fn a_start_puts_away_the_runs_of_a_while_ago_and_keeps_the_one_it_made_and_the_recent_ones() {
        let (_linear, rig) = machine();
        let old = old_run(&rig, 30, "ACM-1");
        let recent = old_run(&rig, 3, "ACM-2");
        let started = start(&rig.session, hand("issue-412", false)).unwrap();
        assert!(!old.exists(), "a run of thirty days ago is put away");
        assert!(recent.join("issues.md").exists());
        assert!(
            std::path::Path::new(&started.context).exists(),
            "the file the agent reads from is there"
        );
        rig.hands.end();
    }

    #[test]
    fn the_run_of_a_pane_that_is_watched_is_kept_while_it_is_and_put_away_after() {
        let (_linear, rig) = machine();
        let old = old_run(&rig, 30, "ACM-1");
        let stale = old_run(&rig, 40, "ACM-2");
        // An agent of that run is at work in p9: the pane does not end until it is told to.
        rig.session.watch(
            &["issue-412".to_owned()],
            "Claude",
            "p9",
            &old.join("issues.md"),
        );
        assert_eq!(rig.session.tidy(), 1);
        assert!(old.join("issues.md").exists(), "its agent reads from it");
        assert!(!stale.exists());
        rig.hands.end();
        until("the pane ended", || rig.session.state().working.is_empty());
        assert_eq!(rig.session.tidy(), 1);
        assert!(!old.exists());
    }

    // --- What the form chose is kept ------------------------------------------------------------

    fn kept(rig: &crate::page_tests::Rig) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(rig.state_dir().join("prefs.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn said_prefs(rig: &crate::page_tests::Rig) -> serde_json::Value {
        serde_json::to_value(remembered(&rig.session)).unwrap()
    }

    #[test]
    fn a_start_keeps_the_agent_the_folder_the_place_and_the_status_where_the_row_was_there() {
        let (_linear, rig) = machine();
        assert_eq!(kept(&rig), None, "nothing before the first start");
        assert_eq!(
            said_prefs(&rig),
            json!({ "agent": null, "folder": null, "place": null, "status": null })
        );
        let mut tab = hand("issue-415", false);
        tab.folder = "worktree".to_owned();
        tab.open = "tab".to_owned();
        start(&rig.session, tab).unwrap();
        let first =
            json!({ "agent": "claude", "folder": "worktree", "place": "tab", "status": "keep" });
        assert_eq!(kept(&rig), Some(first.clone()));
        assert_eq!(said_prefs(&rig), first);
        // A form that had no Status row says nothing of it: what was kept of it stays.
        let mut none = hand("issue-412", false);
        none.progress = None;
        start(&rig.session, none).unwrap();
        assert_eq!(
            kept(&rig),
            Some(
                json!({ "agent": "claude", "folder": "here", "place": "split", "status": "keep" })
            )
        );
        // And one that did, and moved the issues, says so.
        start(&rig.session, hand("issue-415", true)).unwrap();
        assert_eq!(kept(&rig).unwrap()["status"], "progress");
        rig.hands.end();
    }

    #[test]
    fn a_start_that_came_to_nothing_keeps_nothing() {
        let (_linear, rig) = machine();
        rig.hands.refuse_the_pane("the bateri app is not connected");
        assert!(start(&rig.session, hand("issue-415", true)).is_err());
        assert_eq!(kept(&rig), None);
        // Nor does a call that was not understood.
        let mut odd = hand("issue-415", false);
        odd.agent = "emacs".to_owned();
        assert!(start(&rig.session, odd).is_err());
        assert_eq!(kept(&rig), None);
    }

    #[test]
    fn what_the_file_says_that_the_form_has_not_is_not_said() {
        let (_linear, rig) = machine();
        let file = rig.state_dir().join("prefs.json");
        std::fs::write(
            &file,
            r#"{ "agent": "emacs", "folder": "moon", "place": "tab", "status": "maybe" }"#,
        )
        .unwrap();
        assert_eq!(
            said_prefs(&rig),
            json!({ "agent": null, "folder": null, "place": "tab", "status": null })
        );
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(
            said_prefs(&rig),
            json!({ "agent": null, "folder": null, "place": null, "status": null })
        );
        // Words that are the form's own are said as they are, an agent that is not here among them
        // (whether it is installed is for the form to say, with what `work_options` tells).
        std::fs::write(
            &file,
            r#"{ "agent": "cursor", "folder": "worktree", "place": "split", "status": "progress" }"#,
        )
        .unwrap();
        assert_eq!(
            said_prefs(&rig),
            json!({ "agent": "cursor", "folder": "worktree", "place": "split", "status": "progress" })
        );
    }

    #[test]
    fn with_no_folder_for_the_state_nothing_is_kept_and_nothing_is_said() {
        let linear = Canned::new(Mode::Fine);
        let session = crate::page_tests::session_over(&linear, Some("lin_api_test"), None);
        assert_eq!(
            serde_json::to_value(remembered(&session)).unwrap(),
            json!({ "agent": null, "folder": null, "place": null, "status": null })
        );
        remember(
            &session,
            Agent::Claude,
            Folder::Here,
            Place::Split,
            Some(true),
        );
    }

    #[test]
    fn an_issue_given_to_a_second_agent_keeps_the_file_of_the_first_while_its_pane_is_on() {
        let (_linear, rig) = machine();
        let first = old_run(&rig, 30, "ACM-1");
        let second = old_run(&rig, 30, "ACM-2");
        let ids = ["issue-412".to_owned()];
        // The same issue, on two panes: it has one mark, and both files are read from.
        rig.session
            .watch(&ids, "Claude", "p9", &first.join("issues.md"));
        rig.session
            .watch(&ids, "Codex", "p10", &second.join("issues.md"));
        assert_eq!(rig.session.tidy(), 0);
        assert!(first.exists() && second.exists());
        rig.hands.end();
        until("both panes ended", || {
            rig.session.state().working.is_empty()
        });
        assert_eq!(rig.session.tidy(), 2);
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
        start(&rig.session, hand("issue-412", false)).unwrap();
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
        start(&rig.session, hand("issue-412", false)).unwrap();
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
        start(&rig.session, hand("issue-412", false)).unwrap();
        until("the new pane is looked at", || {
            rig.hands.ls_calls() >= calls + 3
        });
        assert_eq!(idle(&rig), Some(false));
        rig.hands.end();
        until("the mark goes", || idle(&rig).is_none());
    }
}
