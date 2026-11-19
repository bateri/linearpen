//! The commands against a transport of memory: which names are found, what is sent and what is not,
//! and how each way of failing ends.

use std::sync::Mutex;

use super::*;
use crate::command_line::{self, Asked};
use crate::linear::transport::Reply;

const TEAMS: &str = include_str!("../../../test/stand-in/teams.json");
const OPTIONS: &str = include_str!("../../../test/stand-in/new-issue-options.json");
const ISSUE: &str = include_str!("../../../test/stand-in/issue.json");
const FILTER_OPTIONS: &str = include_str!("../../../test/stand-in/filter-options.json");

/// What a stand-in for Linear says to an operation (its name, the body).
type Answer = Box<dyn Fn(&str, &str) -> Result<Reply, Error> + Send + Sync>;

/// A picture of Linear the stand-in has: its bytes, or why they cannot be had.
type Held = Result<Vec<u8>, String>;

/// Answers by the name of the operation, and keeps the bodies it was sent.
struct Memory {
    answer: Answer,
    sent: Mutex<Vec<String>>,
    /// The pictures of Linear it has, by address, and the addresses it was asked for.
    pictures: Mutex<Vec<(String, Held)>>,
    fetched: Mutex<Vec<String>>,
}

impl Memory {
    fn new(answer: impl Fn(&str, &str) -> Result<Reply, Error> + Send + Sync + 'static) -> Self {
        Self {
            answer: Box::new(answer),
            sent: Mutex::new(Vec::new()),
            pictures: Mutex::new(Vec::new()),
            fetched: Mutex::new(Vec::new()),
        }
    }

    /// Has the picture at `url`: its bytes, or why they cannot be had.
    fn with_picture(self, url: &str, bytes: Result<Vec<u8>, &str>) -> Self {
        self.pictures
            .lock()
            .unwrap()
            .push((url.to_owned(), bytes.map_err(str::to_owned)));
        self
    }

    fn fetched(&self) -> Vec<String> {
        self.fetched.lock().unwrap().clone()
    }

    fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }

    /// The bodies of the requests of the operation `name`.
    fn of(&self, name: &str) -> Vec<String> {
        self.sent()
            .into_iter()
            .filter(|body| body.contains(&format!("\"operationName\":\"{name}\"")))
            .collect()
    }

    fn wrote(&self) -> usize {
        self.sent()
            .iter()
            .filter(|body| body.contains("\"query\":\"mutation "))
            .count()
    }
}

impl Transport for Memory {
    fn post(&self, _: &Key, body: &str) -> Result<Reply, Error> {
        self.sent.lock().unwrap().push(body.to_owned());
        let name = body
            .split("\"operationName\":\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_default()
            .to_owned();
        (self.answer)(&name, body)
    }

    fn fetch(&self, _: &Key, url: &str) -> Result<Vec<u8>, Error> {
        self.fetched.lock().unwrap().push(url.to_owned());
        let held = self.pictures.lock().unwrap();
        match held.iter().find(|(held, _)| held == url) {
            Some((_, Ok(bytes))) => Ok(bytes.clone()),
            Some((_, Err(why))) => Err(Error::Failed(why.clone())),
            None => Err(Error::Unreachable("no such picture".to_owned())),
        }
    }

    fn fetch_to(
        &self,
        key: &Key,
        url: &str,
        to: &std::path::Path,
        most: u64,
    ) -> Result<u64, Error> {
        let bytes = self.fetch(key, url)?;
        if bytes.len() as u64 > most {
            return Err(Error::Failed(
                crate::linear::transport::TOO_LARGE.to_owned(),
            ));
        }
        std::fs::write(to, &bytes).map_err(|_| Error::Unreachable("write".to_owned()))?;
        Ok(bytes.len() as u64)
    }
}

fn ok(body: &str) -> Result<Reply, Error> {
    Ok(Reply {
        status: 200,
        body: body.to_owned(),
    })
}

fn lost() -> Result<Reply, Error> {
    Err(Error::Unreachable("the request ran out of time".to_owned()))
}

/// An issue of the team `team`, with the id `id`.
fn issue_of(id: &str, identifier: &str, team: &str) -> String {
    format!(
        r#"{{"id":"{id}","identifier":"{identifier}","title":"Add rate limiting","priority":2,"updatedAt":"2026-11-09T14:20:00.000Z","url":"https://linear.app/x/{identifier}","state":{{"id":"s-prog","name":"In Progress","type":"started","position":2}},"labels":{{"nodes":[]}},"team":{{"id":"team-{team}","key":"{}"}},"cycle":null,"project":null,"assignee":null}}"#,
        team.to_uppercase()
    )
}

fn issue_reply(id: &str, identifier: &str, team: &str) -> Result<Reply, Error> {
    ok(&format!(
        r#"{{"data":{{"issue":{}}}}}"#,
        issue_of(id, identifier, team)
    ))
}

/// A world with the team ACM and its options, the issues `ACM-1` and `ACM-2` of it and `WEB-3` of
/// another, and a batch update that answers with the issues as changed.
fn world() -> Memory {
    Memory::new(|name, body| match name {
        "NewIssueTeams" => ok(TEAMS),
        "NewIssueOptions" => ok(OPTIONS),
        "FilterOptions" => ok(FILTER_OPTIONS),
        "Issue" => {
            if body.contains("\"id\":\"ACM-1\"") {
                issue_reply("i1", "ACM-1", "acm")
            } else if body.contains("\"id\":\"ACM-2\"") {
                issue_reply("i2", "ACM-2", "acm")
            } else if body.contains("\"id\":\"WEB-3\"") {
                issue_reply("i3", "WEB-3", "web")
            } else if body.contains("\"id\":\"ACM-412\"") {
                ok(ISSUE)
            } else {
                ok(r#"{"data":{"issue":null},"errors":[{"message":"Entity not found: Issue"}]}"#)
            }
        }
        "IssueBatchUpdate" => ok(&format!(
            r#"{{"data":{{"issueBatchUpdate":{{"success":true,"issues":[{}]}}}}}}"#,
            ["i1", "i2"]
                .iter()
                .filter(|id| body.contains(&format!("\"{id}\"")))
                .map(
                    |id| issue_of(id, &format!("ACM-{}", &id[1..]), "acm").replace(
                        "\"In Progress\",\"type\":\"started\"",
                        "\"Done\",\"type\":\"completed\"",
                    )
                )
                .collect::<Vec<_>>()
                .join(",")
        )),
        "CommentCreate" => ok(
            r#"{"data":{"commentCreate":{"success":true,"comment":{"id":"c","body":"x","createdAt":"2026-11-10T10:00:00.000Z","user":{"displayName":"Ada"}}}}}"#,
        ),
        _ => lost(),
    })
}

fn command(args: &[&str]) -> Command {
    match command_line::read(args.iter().map(|arg| (*arg).to_owned())) {
        Asked::Do(command) => command,
        other => panic!("{args:?}: {other:?}"),
    }
}

fn run_with(memory: &Memory, args: &[&str], stdin: &str) -> Done {
    let key = Key::new("lin_api_test").unwrap();
    let mut input = stdin.as_bytes();
    run(
        &Access {
            transport: memory,
            key: &key,
            key_source: "LINEAR_API_KEY",
        },
        &command(args),
        &mut input,
    )
}

#[test]
fn an_update_of_two_fields_is_one_batch_update_and_prints_each_issue_as_it_is_now() {
    let memory = world();
    let done = run_with(
        &memory,
        &[
            "update",
            "acm-1",
            "ACM-2",
            "--status",
            "done",
            "--label",
            "API",
            "--assignee",
            "me",
        ],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(memory.wrote(), 1);
    let sent = &memory.of("IssueBatchUpdate")[0];
    for part in ["\\\"i1\\\"", "s-done", "l-api", "u-ada"] {
        let plain = sent.replace('\\', "");
        assert!(plain.contains(&part.replace('\\', "")), "{part}: {sent}");
    }
    let lower = sent.to_lowercase();
    assert!(
        !lower.contains("description") && !lower.contains("teamid"),
        "{sent}"
    );
    assert_eq!(done.out.lines().count(), 2, "{}", done.out);
    assert!(done.out.starts_with("ACM-1  Done"), "{}", done.out);
    assert!(!sent.contains("lin_api_test"));
}

#[test]
fn a_name_that_is_not_there_stops_the_update_with_the_names_there_are_and_sends_nothing() {
    let memory = world();
    let done = run_with(&memory, &["update", "ACM-1", "--status", "Doing"], "");
    assert_eq!(done.status, 1);
    assert_eq!(
        done.err,
        "linearpen: ACM has no status \"Doing\"; it has Backlog, Todo, In Progress, In Review, Done, Canceled\n"
    );
    assert_eq!(memory.wrote(), 0);
    for (flag, value) in [
        ("--label", "nope"),
        ("--unlabel", "nope"),
        ("--assignee", "Nobody"),
        ("--project", "x"),
        ("--cycle", "99"),
        ("--estimate", "XXL"),
    ] {
        let memory = world();
        let done = run_with(&memory, &["update", "ACM-1", flag, value], "");
        assert_eq!(done.status, 1, "{flag}: {}", done.err);
        assert!(
            done.err.starts_with("linearpen: ACM has no "),
            "{flag}: {}",
            done.err
        );
        assert_eq!(memory.wrote(), 0, "{flag}");
    }
    // The names there are come from Linear: an escape in one does not reach the terminal.
    let memory = Memory::new(|name, body| match name {
        "NewIssueOptions" => ok(&OPTIONS.replace(
            r#""name": "Customer""#,
            r#""name": "Cust\u001b[31momer\nX""#,
        )),
        other => (world().answer)(other, body),
    });
    let done = run_with(&memory, &["update", "ACM-1", "--label", "nope"], "");
    assert_eq!(done.status, 1, "{}", done.err);
    assert!(!done.err.contains('\u{1b}'), "{:?}", done.err);
    assert!(done.err.contains("Cust [31momer X"), "{:?}", done.err);
    assert_eq!(done.err.lines().count(), 1, "{:?}", done.err);
    assert_eq!(memory.wrote(), 0);
}

#[test]
fn issues_of_two_teams_are_not_changed_together() {
    let memory = world();
    let done = run_with(
        &memory,
        &["update", "ACM-1", "WEB-3", "--status", "Done"],
        "",
    );
    assert_eq!(done.status, 1);
    assert!(
        done.err
            .contains("issues of one team are changed together: ACM-1 is of ACM, WEB-3 is of WEB"),
        "{}",
        done.err
    );
    assert_eq!(memory.wrote(), 0);
    assert!(memory.of("NewIssueOptions").is_empty());
}

#[test]
fn an_issue_linear_has_not_is_said_so() {
    let memory = world();
    let done = run_with(&memory, &["update", "ACM-77", "--status", "Done"], "");
    assert_eq!(
        (done.status, done.err.as_str()),
        (1, "linearpen: Linear has no issue ACM-77\n")
    );
    assert_eq!(memory.wrote(), 0);
}

#[test]
fn an_update_whose_answer_is_lost_is_not_sent_again_and_ends_with_three() {
    let memory = Memory::new(|name, body| {
        if name == "IssueBatchUpdate" {
            lost()
        } else {
            world().post(&Key::new("k").unwrap(), body)
        }
    });
    let done = run_with(&memory, &["update", "ACM-1", "--priority", "low"], "");
    assert_eq!(done.status, 3, "{}", done.err);
    assert!(
        done.err.contains(
            "it is not known whether Linear made it; look in Linear before running it again"
        ),
        "{}",
        done.err
    );
    assert_eq!(memory.of("IssueBatchUpdate").len(), 1);
}

#[test]
fn linear_saying_no_to_a_write_is_one_and_a_key_it_refuses_is_said_with_where_it_came_from() {
    let memory = Memory::new(|name, body| match name {
        "IssueBatchUpdate" => ok(r#"{"errors":[{"message":"nope"}]}"#),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(&memory, &["update", "ACM-1", "--priority", "low"], "");
    assert_eq!(
        (done.status, done.err.as_str()),
        (1, "linearpen: Linear said: nope\n")
    );
    let refused = Memory::new(|_, _| {
        Ok(Reply {
            status: 401,
            body: String::new(),
        })
    });
    let done = run_with(&refused, &["teams"], "");
    assert_eq!(
        (done.status, done.err.as_str()),
        (
            1,
            "linearpen: Linear did not accept the key from LINEAR_API_KEY\n"
        )
    );
}

#[test]
fn a_comment_is_posted_once_in_the_words_on_stdin_and_a_blank_one_is_not() {
    let memory = world();
    let done = run_with(
        &memory,
        &["comment", "ACM-412", "--body", "-"],
        "Looks right\nto me\n\n",
    );
    assert_eq!(
        (done.status, done.out.as_str()),
        (0, "ACM-412: comment added\n")
    );
    let sent = memory.of("CommentCreate");
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].contains("\"body\":\"Looks right\\nto me\"}"),
        "{}",
        sent[0]
    );
    let memory = world();
    let done = run_with(&memory, &["comment", "ACM-412", "--body", "-"], " \n");
    assert_eq!(done.status, 2, "{}", done.err);
    assert!(memory.sent().is_empty());
    // The comment goes to the issue's own id.
    let input: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(input["variables"]["input"]["issueId"], "issue-412");
    let lost_memory = Memory::new(|name, _| match name {
        "Issue" => ok(ISSUE),
        _ => lost(),
    });
    let done = run_with(&lost_memory, &["comment", "ACM-412", "--body", "hi"], "");
    assert_eq!(done.status, 3);
    assert_eq!(lost_memory.of("CommentCreate").len(), 1);
    // A write that never left the machine was not made: 1, not 3.
    let offline = Memory::new(|name, _| match name {
        "Issue" => ok(ISSUE),
        _ => Err(Error::Unreachable("the connection was refused".to_owned())),
    });
    let done = run_with(&offline, &["comment", "ACM-412", "--body", "hi"], "");
    assert_eq!(done.status, 1, "{}", done.err);
    assert!(!done.err.contains("not known"), "{}", done.err);
    // An issue Linear does not have is told before anything is written.
    let memory = world();
    let done = run_with(&memory, &["comment", "ACM-9", "--body", "hi"], "");
    assert_eq!(done.status, 1, "{}", done.err);
    assert!(memory.of("CommentCreate").is_empty());
}

#[test]
fn a_create_whose_answer_is_lost_is_asked_for_by_the_id_of_its_draft() {
    let found = Memory::new(|name, body| match name {
        "IssueCreate" => lost(),
        "Issue" => {
            let id = body
                .split("\"id\":\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or_default()
                .to_owned();
            issue_reply(&id, "ACM-900", "acm")
        }
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &found,
        &[
            "create", "--team", "acme", "--title", "Fix it", "--status", "todo",
        ],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(done.out, "ACM-900  https://linear.app/x/ACM-900\n");
    assert_eq!(found.of("IssueCreate").len(), 1);

    let missing = Memory::new(|name, body| match name {
        "IssueCreate" => lost(),
        "Issue" => {
            ok(r#"{"data":{"issue":null},"errors":[{"message":"Entity not found: Issue"}]}"#)
        }
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &missing,
        &["create", "--team", "ACM", "--title", "Fix it"],
        "",
    );
    // Linear has no issue under the draft's id: it was not made, as the page settles it.
    assert_eq!(done.status, 1, "{}", done.err);
    assert!(done.err.contains("so it was not made"), "{}", done.err);
    assert_eq!(missing.of("IssueCreate").len(), 1);

    let unknown = Memory::new(|name, body| match name {
        "IssueCreate" | "Issue" => lost(),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &unknown,
        &["create", "--team", "ACM", "--title", "Fix it"],
        "",
    );
    assert_eq!(done.status, 3);
    assert!(
        done.err.contains("the id of this draft is "),
        "{}",
        done.err
    );
    assert_eq!(unknown.of("IssueCreate").len(), 1);
}

#[test]
fn words_on_stdin_longer_than_a_body_is_are_too_long_even_cut_in_a_character() {
    let memory = world();
    let long = "\u{1f600}".repeat(crate::linear::limits::BODY_MOST + 1);
    let done = run_with(&memory, &["comment", "ACM-412", "--body", "-"], &long);
    assert_eq!(done.status, 2, "{}", done.err);
    assert!(done.err.contains("characters"), "{}", done.err);
    assert!(memory.sent().is_empty());
}

#[test]
fn an_issue_linear_does_not_tell_after_a_change_is_said_on_stderr_also_with_json() {
    let memory = Memory::new(|name, body| match name {
        "IssueBatchUpdate" => ok(&format!(
            r#"{{"data":{{"issueBatchUpdate":{{"success":true,"issues":[{}]}}}}}}"#,
            issue_of("i1", "ACM-1", "acm")
        )),
        "Issue" if body.contains("\"id\":\"i2\"") => lost(),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &memory,
        &["update", "ACM-1", "ACM-2", "--priority", "low", "--json"],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(done.out.contains("\"ACM-1\""), "{}", done.out);
    assert_eq!(
        done.err,
        "linearpen: ACM-2 was changed; Linear did not say how it is now\n"
    );
}

#[test]
fn a_create_linear_refuses_is_one_and_is_not_asked_about() {
    let memory = Memory::new(|name, body| match name {
        "IssueCreate" => ok(r#"{"errors":[{"message":"bad"}]}"#),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(&memory, &["create", "--team", "ACM", "--title", "x"], "");
    assert_eq!(done.status, 1);
    assert!(memory.of("Issue").is_empty());
}

#[test]
fn a_create_sends_the_values_by_the_ids_the_names_stand_for_and_the_description_from_stdin() {
    let memory = Memory::new(|name, body| match name {
        "IssueCreate" => ok(
            r#"{"data":{"issueCreate":{"success":true,"issue":{"id":"x","identifier":"ACM-5","url":"https://linear.app/x/ACM-5"}}}}"#,
        ),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &memory,
        &[
            "create",
            "--team",
            "ACM",
            "--title",
            "T",
            "--description",
            "-",
            "--priority",
            "high",
            "--assignee",
            "deniz",
            "--label",
            "bug",
            "--project",
            "billing",
            "--cycle",
            "current",
            "--estimate",
            "m",
            "--due",
            "2026-11-20",
            "--json",
        ],
        "The words\n",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    let made: serde_json::Value = serde_json::from_str(&done.out).unwrap();
    assert_eq!(made["identifier"], "ACM-5");
    let sent = memory.of("IssueCreate");
    let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    let input = &body["variables"]["input"];
    assert_eq!(input["teamId"], "team-acm");
    assert_eq!(input["description"], "The words");
    assert_eq!(input["priority"], 2);
    assert_eq!(input["assigneeId"], "u-deniz");
    assert_eq!(input["labelIds"], serde_json::json!(["l-bug"]));
    assert_eq!(input["projectId"], "p-1");
    assert_eq!(input["cycleId"], "c-12");
    assert_eq!(input["estimate"], 3);
    assert_eq!(input["dueDate"], "2026-11-20");
}

#[test]
fn a_list_asks_for_the_team_and_the_cycle_by_name_and_refuses_what_a_list_cannot_hold() {
    let memory = Memory::new(|name, body| match name {
        "CycleIssues" => ok(
            r#"{"data":{"viewer":{"id":"u","name":"Ada","organization":{"name":"Acme","urlKey":"acme"}},"cycle":{"issues":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#,
        ),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &memory,
        &[
            "list",
            "--team",
            "acm",
            "--cycle",
            "current",
            "--status",
            "in review",
            "--priority",
            "high",
        ],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(
        done.out
            .starts_with("ACM Cycle 12 (0)\n  No issue matches."),
        "{}",
        done.out
    );
    let sent = &memory.of("CycleIssues")[0];
    assert!(
        sent.contains("\\\"id\\\":\\\"c-12\\\"") || sent.contains("\"id\":\"c-12\""),
        "{sent}"
    );
    assert!(sent.contains("In Review"), "{sent}");
    // A closed status is for a search.
    let done = run_with(&world(), &["list", "--status", "done"], "");
    assert_eq!(done.status, 1);
    assert!(done.err.contains("closed status"), "{}", done.err);
    // A name Linear has not, among the ones it has.
    let done = run_with(&world(), &["list", "--label", "nope"], "");
    assert_eq!(done.status, 1);
    assert!(
        done.err
            .starts_with("linearpen: the workspace has no label \"nope\"; it has "),
        "{}",
        done.err
    );
    let done = run_with(&world(), &["list", "--team", "zzz"], "");
    assert!(
        done.err
            .contains("the workspace has no team \"zzz\"; it has ACM (Acme), WEB (web)"),
        "{}",
        done.err
    );
}

#[test]
fn show_prints_the_issue_with_its_description_and_what_a_terminal_would_obey_is_gone() {
    let dirty = ISSUE.replace("The public API", "The \\u001b]52;c;AAAA\\u0007 public API");
    let memory = Memory::new(move |_, _| ok(&dirty));
    let done = run_with(&memory, &["show", "ACM-412"], "");
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(
        !done.out.contains('\u{1b}') && !done.out.contains('\u{7}'),
        "{:?}",
        done.out
    );
    for part in [
        "ACM-412  Add rate limiting to the public API",
        "Status    In Progress",
        "Assignee  Ada",
        "Description\n  The ",
        "  - A limit per API key",
        "Part of\n  ACM-400  Todo  Billing limits per plan",
        "Blocked by\n  ACM-389  In Review",
        "Comments (2)\n  Deniz, Nov 6\n    Redis is already there",
    ] {
        assert!(done.out.contains(part), "{part}: {}", done.out);
    }
    let json = run_with(&world(), &["show", "ACM-412", "--json"], "");
    let shown: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    assert_eq!(shown["identifier"], "ACM-412");
    assert_eq!(shown["comments"].as_array().unwrap().len(), 2);
    assert_eq!(shown["ties"][0]["tie"], "blocked_by");
}

#[test]
fn teams_tells_the_teams_and_the_names_of_one() {
    let done = run_with(&world(), &["teams"], "");
    assert_eq!(done.out, "ACM  Acme\nWEB  web\n");
    let done = run_with(&world(), &["teams", "acm"], "");
    assert_eq!(done.status, 0, "{}", done.err);
    for part in [
        "ACM  Acme",
        "Statuses\n  Backlog\n  Todo",
        "  Ada (you: me)",
        "  12  Cycle 12  (running)",
        "  M (3)",
        "  Billing",
    ] {
        assert!(done.out.contains(part), "{part}: {}", done.out);
    }
    let json = run_with(&world(), &["teams", "ACM", "--json"], "");
    let names: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    assert_eq!(names["key"], "ACM");
    assert_eq!(names["estimates"][2]["label"], "M");
}

#[test]
fn none_takes_a_value_away_in_an_update_and_is_a_filter_in_a_list() {
    let memory = world();
    let done = run_with(
        &memory,
        &[
            "update",
            "ACM-1",
            "--assignee",
            "none",
            "--project",
            "none",
            "--cycle",
            "none",
            "--estimate",
            "none",
            "--due",
            "none",
        ],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    let body: serde_json::Value = serde_json::from_str(&memory.of("IssueBatchUpdate")[0]).unwrap();
    assert_eq!(
        body["variables"]["input"],
        serde_json::json!({
            "assigneeId": null, "projectId": null, "cycleId": null, "estimate": null, "dueDate": null
        })
    );
    let memory = Memory::new(|name, body| match name {
        "AllIssues" => ok(
            r#"{"data":{"viewer":{"id":"u","name":"Ada","organization":{"name":"Acme","urlKey":"acme"}},"issues":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#,
        ),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(
        &memory,
        &[
            "list",
            "--all",
            "--assignee",
            "me",
            "--assignee",
            "NONE",
            "--project",
            "none",
        ],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    let body: serde_json::Value = serde_json::from_str(&memory.of("AllIssues")[0]).unwrap();
    let filter = body["variables"]["filter"].to_string();
    for part in [
        "\"isMe\"",
        "\"assignee\":{\"null\":true}",
        "\"project\":{\"null\":true}",
    ] {
        assert!(filter.contains(part), "{part}: {filter}");
    }
    // The people and projects are not asked for where only me and none are said.
    assert!(memory.of("FilterOptions").is_empty() && memory.of("ListProjects").is_empty());
}

#[test]
fn a_create_that_linear_says_it_did_not_make_is_a_no_and_not_a_not_known() {
    let memory = Memory::new(|name, body| match name {
        "IssueCreate" => ok(r#"{"data":{"issueCreate":{"success":false,"issue":null}}}"#),
        _ => world().post(&Key::new("k").unwrap(), body),
    });
    let done = run_with(&memory, &["create", "--team", "ACM", "--title", "x"], "");
    assert_eq!(
        (done.status, memory.of("Issue").len()),
        (1, 0),
        "{}",
        done.err
    );
}

const MINE: &str = "11111111-2222-4333-8444-555555555555";

/// The words of `line` as a shell would give them: white space between them, single and double
/// quotes and a backslash outside quotes to hold what would end a word.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut open = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                open = true;
            }
            (None, '\\') => {
                word.extend(chars.next());
                open = true;
            }
            (None, c) if c.is_whitespace() => {
                if open {
                    words.push(std::mem::take(&mut word));
                    open = false;
                }
            }
            (None, c) => {
                word.push(c);
                open = true;
            }
        }
    }
    if open {
        words.push(word);
    }
    words
}

// The commands the agent is told to run are commands `linearpen` takes, as the program the agent is told
// to run is written whatever its path is.
#[test]
fn the_commands_the_prompt_gives_the_agent_are_ones_the_command_line_takes() {
    for exe in [
        None,
        Some("/usr/local/bin/linearpen"),
        Some("/Applications/My Tools/lp's bin/linearpen"),
    ] {
        let said = crate::work::handoff::prompt(
            std::path::Path::new("/state/work/x/issues.md"),
            false,
            Some(crate::work::handoff::Writes {
                exe: exe.map(std::path::Path::new),
                issue: MINE,
            }),
            None,
        );
        // The commands are what is between the backticks.
        let spans: Vec<&str> = said.split('`').skip(1).step_by(2).collect();
        assert_eq!(spans.len(), 5, "{said}");
        let read = |span: &str| {
            let words = shell_words(
                &span
                    .replace("<status>", "In Review")
                    .replace("<title>", "A title")
                    .replace("PATH", "/tmp/shot.png")
                    .replace("KEY", "ACM"),
            );
            assert_eq!(words[0], exe.unwrap_or("linearpen"), "{span}");
            match command_line::read(words[1..].iter().cloned()) {
                Asked::Do(command) => command,
                other => panic!("{span}: {other:?}"),
            }
        };
        let Command::Update(update) = read(spans[0]) else {
            panic!("not an update: {}", spans[0]);
        };
        assert_eq!(update.issues, [MINE]);
        assert_eq!(update.fields.status.as_deref(), Some("In Review"));
        let Command::Comment(comment) = read(spans[1]) else {
            panic!("not a comment: {}", spans[1]);
        };
        assert_eq!(comment.issue, MINE);
        assert_eq!(comment.body, Some(crate::command_line::Words::Stdin));
        assert!(comment.attach.is_empty());
        let Command::Comment(with_picture) = read(spans[2]) else {
            panic!("not a comment: {}", spans[2]);
        };
        assert_eq!(with_picture.attach, ["/tmp/shot.png"]);
        let Command::Update(describe) = read(spans[3]) else {
            panic!("not an update: {}", spans[3]);
        };
        assert_eq!(describe.issues, [MINE]);
        assert_eq!(describe.attach, ["/tmp/shot.png"]);
        let Command::Create(create) = read(spans[4]) else {
            panic!("not a create: {}", spans[4]);
        };
        assert_eq!(create.title, "A title");
    }
}

// --- Pictures ---------------------------------------------------------------------------------------

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nthe-first";
const JPEG: &[u8] = &[0xff, 0xd8, 0xff, 0xe0, 1, 2, 3];
const LOG: &[u8] = b"line one\nline two\n";

/// The issue of the fixture with pictures in its description and its first comment: three in the
/// description (one of them not Linear's), and in the comment a picture, a file of Linear's (a link)
/// and a link to another site, which is words.
fn picture_issue() -> String {
    let mut value: serde_json::Value = serde_json::from_str(ISSUE).unwrap();
    let issue = &mut value["data"]["issue"];
    issue["description"] = "Broken: ![Checkout error](https://uploads.linear.app/w/1/checkout.png) and\n![](https://uploads.linear.app/w/2/trace.jpg) and ![elsewhere](https://example.com/y.png)\n\nThen done.".into();
    issue["comments"]["nodes"][0]["body"] =
        "Looks like ![](https://uploads.linear.app/w/3/other.png) and the log [build log.txt](https://uploads.linear.app/w/4/build.log) or [a site](https://example.com/z)".into();
    value.to_string()
}

fn picture_world() -> Memory {
    let issue = picture_issue();
    Memory::new(move |name, body| match name {
        "Issue" => ok(&issue),
        _ => world().post(&Key::new("k").unwrap(), body),
    })
    .with_picture(
        "https://uploads.linear.app/w/1/checkout.png",
        Ok(PNG.to_vec()),
    )
    .with_picture(
        "https://uploads.linear.app/w/2/trace.jpg",
        Ok(JPEG.to_vec()),
    )
    .with_picture("https://uploads.linear.app/w/3/other.png", Ok(PNG.to_vec()))
    .with_picture("https://uploads.linear.app/w/4/build.log", Ok(LOG.to_vec()))
}

#[test]
fn show_puts_each_file_as_a_numbered_placeholder_lists_them_and_says_how_to_save_them() {
    let memory = picture_world();
    let done = run_with(&memory, &["show", "ACM-412"], "");
    assert_eq!(done.status, 0, "{}", done.err);
    let out = done.out;
    // In the order of the issue, every picture and file counted, the name of the address for one with no
    // name; a link to another site is words and no file.
    for placeholder in [
        "[image 1: Checkout error]",
        "[image 2: trace.jpg]",
        "[image 3: elsewhere]",
        "[image 4: other.png]",
        "[file 5: build log.txt]",
        "[a site](https://example.com/z)",
    ] {
        assert!(out.contains(placeholder), "{placeholder}: {out}");
    }
    assert!(!out.contains("!["), "{out}");
    assert!(out.contains("\nFiles\n"), "{out}");
    assert!(
        out.contains("  1  description  https://uploads.linear.app/w/1/checkout.png\n"),
        "{out}"
    );
    assert!(
        out.contains("  3  description  https://example.com/y.png\n"),
        "{out}"
    );
    assert!(
        out.contains("  4  comment by Deniz  https://uploads.linear.app/w/3/other.png\n"),
        "{out}"
    );
    assert!(
        out.contains("  5  comment by Deniz  https://uploads.linear.app/w/4/build.log\n"),
        "{out}"
    );
    assert!(
        out.ends_with("\n5 files: linearpen show ACM-412 --files DIR saves them\n"),
        "{out}"
    );
    // Nothing was fetched, and nothing was written.
    assert!(memory.fetched().is_empty());
    assert_eq!(memory.wrote(), 0);
    // An issue with none has no section and no hint.
    let plain = run_with(&world(), &["show", "ACM-412"], "");
    assert!(
        !plain.out.contains("Files") && !plain.out.contains("--files"),
        "{}",
        plain.out
    );
}

#[test]
fn show_json_keeps_the_texts_as_they_are_and_lists_the_files() {
    let memory = picture_world();
    let done = run_with(&memory, &["show", "ACM-412", "--json"], "");
    assert_eq!(done.status, 0, "{}", done.err);
    let value: serde_json::Value = serde_json::from_str(&done.out).unwrap();
    assert!(
        value["description"]
            .as_str()
            .unwrap()
            .contains("![Checkout error](https://uploads.linear.app/w/1/checkout.png)")
    );
    assert!(
        value["comments"][0]["body"]
            .as_str()
            .unwrap()
            .contains("![](")
    );
    let files = value["files"].as_array().unwrap();
    assert_eq!(files.len(), 5);
    assert_eq!(
        files[0],
        serde_json::json!({"n": 1, "kind": "image", "name": "Checkout error", "url": "https://uploads.linear.app/w/1/checkout.png", "in": "description", "comment": null, "path": null, "refused": null})
    );
    assert_eq!(files[3]["in"], "comment");
    assert_eq!(files[3]["comment"], 0);
    assert_eq!(files[3]["name"], "");
    assert_eq!(files[4]["kind"], "file");
    assert_eq!(files[4]["name"], "build log.txt");
    assert!(value.get("images").is_none(), "the old name is gone");
    assert!(memory.fetched().is_empty());
}

#[test]
fn files_saves_the_files_of_linear_in_a_folder_of_its_own_and_tells_each_by_its_file() {
    let memory = picture_world();
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("made/for/it");
    let dir_arg = dir.to_str().unwrap();
    let done = run_with(&memory, &["show", "ACM-412", "--files", dir_arg], "");
    assert_eq!(done.status, 0, "{}", done.err);
    let at = |name: &str| {
        std::path::absolute(dir.join(name))
            .unwrap()
            .display()
            .to_string()
    };
    assert!(
        done.out.contains(&format!(
            "  1  description  {}\n",
            at("1-Checkout-error.png")
        )),
        "{}",
        done.out
    );
    assert!(
        done.out
            .contains(&format!("  2  description  {}\n", at("2-trace.jpg"))),
        "{}",
        done.out
    );
    assert!(
        done.out
            .contains("  3  description  not on Linear  https://example.com/y.png\n"),
        "{}",
        done.out
    );
    assert!(
        done.out
            .contains(&format!("  4  comment by Deniz  {}\n", at("4-other.png"))),
        "{}",
        done.out
    );
    assert!(
        done.out.contains(&format!(
            "  5  comment by Deniz  {}\n",
            at("5-build-log.txt")
        )),
        "{}",
        done.out
    );
    assert!(!done.out.contains("--files DIR"), "{}", done.out);
    // The extension is the bytes' own, the folder is the person's and the files are too.
    assert_eq!(
        std::fs::read(dir.join("1-Checkout-error.png")).unwrap(),
        PNG
    );
    assert_eq!(std::fs::read(dir.join("2-trace.jpg")).unwrap(), JPEG);
    // A file that is not a picture is saved as it is, with the extension its name has.
    assert_eq!(std::fs::read(dir.join("5-build-log.txt")).unwrap(), LOG);
    use std::os::unix::fs::PermissionsExt;
    let mode =
        |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&dir.join("1-Checkout-error.png")), 0o600);
    // Only the files of Linear were asked for.
    assert_eq!(memory.fetched().len(), 4);
    assert!(
        !memory
            .fetched()
            .iter()
            .any(|url| url.contains("example.com"))
    );
    // JSON tells the path and the reason.
    let json = run_with(
        &memory,
        &["show", "ACM-412", "--files", dir_arg, "--json"],
        "",
    );
    let value: serde_json::Value = serde_json::from_str(&json.out).unwrap();
    assert_eq!(value["files"][0]["path"], at("1-Checkout-error.png"));
    assert_eq!(value["files"][2]["path"], serde_json::Value::Null);
    assert_eq!(value["files"][2]["refused"], "not on Linear");
    // The same bytes are the file that is there: nothing is made twice.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
}

#[test]
fn another_file_under_a_name_that_is_taken_gets_a_number_and_nothing_is_written_over() {
    let memory = picture_world();
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("pics");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("1-Checkout-error.png"), b"mine").unwrap();
    let elsewhere = temp.path().join("elsewhere");
    std::fs::write(&elsewhere, b"keep").unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir.join("2-trace.jpg")).unwrap();
    let done = run_with(
        &memory,
        &["show", "ACM-412", "--files", dir.to_str().unwrap()],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(
        std::fs::read(dir.join("1-Checkout-error.png")).unwrap(),
        b"mine"
    );
    assert_eq!(
        std::fs::read(dir.join("1-Checkout-error-2.png")).unwrap(),
        PNG
    );
    // A link is never followed: what it leads to is as it was, and the picture is beside it.
    assert_eq!(std::fs::read(&elsewhere).unwrap(), b"keep");
    assert_eq!(std::fs::read(dir.join("2-trace-2.jpg")).unwrap(), JPEG);
}

#[test]
fn a_file_that_cannot_be_had_gets_its_reason_and_the_command_still_succeeds() {
    let issue = picture_issue()
        .replace("w/1/checkout.png", "w/1/big.png")
        .replace("w/2/trace.jpg", "w/2/text.png")
        .replace("w/3/other.png", "w/3/gone.png");
    let memory = Memory::new(move |name, body| match name {
        "Issue" => ok(&issue),
        _ => world().post(&Key::new("k").unwrap(), body),
    })
    .with_picture(
        "https://uploads.linear.app/w/1/big.png",
        Err(crate::linear::transport::TOO_LARGE),
    )
    .with_picture(
        "https://uploads.linear.app/w/2/text.png",
        Ok(b"<html>".to_vec()),
    )
    .with_picture("https://uploads.linear.app/w/3/gone.png", Err("HTTP 404"));
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("pics");
    let done = run_with(
        &memory,
        &["show", "ACM-412", "--files", dir.to_str().unwrap()],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    // What is not a picture is a file all the same: it is saved as it is.
    assert_eq!(std::fs::read(dir.join("2-text.png")).unwrap(), b"<html>");
    assert!(dir.exists());
    for line in [
        "  1  description  too large  https://uploads.linear.app/w/1/big.png\n",
        "  3  description  not on Linear  https://example.com/y.png\n",
        "  4  comment by Deniz  could not be fetched  https://uploads.linear.app/w/3/gone.png\n",
    ] {
        assert!(done.out.contains(line), "{line}: {}", done.out);
    }
}

#[test]
fn no_folder_is_made_where_nothing_could_be_had() {
    let memory = picture_world_without_files();
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("pics");
    let done = run_with(
        &memory,
        &["show", "ACM-412", "--files", dir.to_str().unwrap()],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert!(done.out.contains("could not be fetched"), "{}", done.out);
    assert!(!dir.exists());
}

/// The issue with pictures and a file that none of Linear's answers for.
fn picture_world_without_files() -> Memory {
    let issue = picture_issue();
    Memory::new(move |name, body| match name {
        "Issue" => ok(&issue),
        _ => world().post(&Key::new("k").unwrap(), body),
    })
    .with_picture(
        "https://uploads.linear.app/w/1/checkout.png",
        Err("HTTP 404"),
    )
    .with_picture("https://uploads.linear.app/w/2/trace.jpg", Err("HTTP 404"))
    .with_picture("https://uploads.linear.app/w/3/other.png", Err("HTTP 404"))
    .with_picture("https://uploads.linear.app/w/4/build.log", Err("HTTP 404"))
}
