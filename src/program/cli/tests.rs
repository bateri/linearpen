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

/// Answers by the name of the operation, and keeps the bodies it was sent.
struct Memory {
    answer: Answer,
    sent: Mutex<Vec<String>>,
}

impl Memory {
    fn new(answer: impl Fn(&str, &str) -> Result<Reply, Error> + Send + Sync + 'static) -> Self {
        Self {
            answer: Box::new(answer),
            sent: Mutex::new(Vec::new()),
        }
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
    run_guarded(memory, Guard::Off, args, stdin)
}

fn run_guarded(memory: &Memory, guard: Guard, args: &[&str], stdin: &str) -> Done {
    let key = Key::new("lin_api_test").unwrap();
    let mut input = stdin.as_bytes();
    run(
        &Access {
            transport: memory,
            key: &key,
            key_source: "LINEAR_API_KEY",
            guard,
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

// What a `linearpen` run in an agent's pane may write (`guard`): the issue it was started on, by
// Linear's id (the stand-in curl answers a UUID with "not found", so what is allowed is tried here).

const MINE: &str = "11111111-2222-4333-8444-555555555555";
const OTHER: &str = "99999999-2222-4333-8444-555555555555";

/// A world of two issues of ACM: `ACM-5` with the id `MINE` and `ACM-6` with the id `OTHER`; an
/// update answers with `MINE`, as changed.
fn agent_world() -> Memory {
    Memory::new(|name, body| {
        let asked = |key: &str| body.contains(&format!("\"id\":\"{key}\""));
        match name {
            "Issue" if asked(MINE) || asked("ACM-5") => issue_reply(MINE, "ACM-5", "acm"),
            "Issue" if asked(OTHER) || asked("ACM-6") => issue_reply(OTHER, "ACM-6", "acm"),
            "Issue" => {
                ok(r#"{"data":{"issue":null},"errors":[{"message":"Entity not found: Issue"}]}"#)
            }
            "IssueBatchUpdate" => ok(&format!(
                r#"{{"data":{{"issueBatchUpdate":{{"success":true,"issues":[{}]}}}}}}"#,
                issue_of(MINE, "ACM-5", "acm")
            )),
            _ => world().post(&Key::new("k").unwrap(), body),
        }
    })
}

fn for_agent() -> Guard {
    Guard::of(Some(std::ffi::OsStr::new(MINE)))
}

#[test]
fn the_variable_makes_the_guard_and_only_an_id_that_is_a_uuid_gives_it_an_issue() {
    use std::os::unix::ffi::OsStrExt;
    assert_eq!(Guard::of(None), Guard::Off);
    assert_eq!(
        Guard::of(Some(std::ffi::OsStr::new(MINE))),
        Guard::Issue(MINE.to_owned())
    );
    // The case of the letters of an id is not what it is.
    let loud = "ABCDEF01-2222-4333-8444-555555555555";
    assert_eq!(
        Guard::of(Some(std::ffi::OsStr::new(loud))),
        Guard::Issue(loud.to_lowercase())
    );
    // A variable that is set is an agent's run, whatever it holds: it is not "no agent".
    for value in [
        std::ffi::OsStr::new(""),
        std::ffi::OsStr::new("ACM-5"),
        std::ffi::OsStr::new("issue-412"),
        std::ffi::OsStr::new(" 11111111-2222-4333-8444-555555555555"),
        std::ffi::OsStr::from_bytes(b"11111111-2222-4333-8444-55555555555\xff"),
    ] {
        assert_eq!(Guard::of(Some(value)), Guard::Nothing, "{value:?}");
    }
}

#[test]
fn an_agent_may_change_the_status_of_its_issue_and_comment_on_it_and_each_is_one_write() {
    let memory = agent_world();
    let done = run_guarded(
        &memory,
        for_agent(),
        &["update", MINE, "--status", "Done"],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(memory.wrote(), 1);
    let sent = memory.of("IssueBatchUpdate");
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].contains(MINE) && sent[0].contains("s-done"),
        "{}",
        sent[0]
    );
    assert!(
        !sent[0].to_lowercase().contains("description"),
        "{}",
        sent[0]
    );

    // The same issue by its identifier, and an id written in capitals, are the same issue.
    let memory = agent_world();
    let done = run_guarded(
        &memory,
        for_agent(),
        &["update", "acm-5", "--status", "Done"],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(memory.wrote(), 1);
    let memory = agent_world();
    let loud = MINE.to_uppercase();
    let done = run_guarded(
        &memory,
        for_agent(),
        &["update", &loud, "--status", "Done"],
        "",
    );
    assert_eq!(done.status, 0, "{}", done.err);
    assert_eq!(memory.wrote(), 1);

    let memory = agent_world();
    let done = run_guarded(
        &memory,
        for_agent(),
        &["comment", MINE, "--body", "-"],
        "Done; the limiter is in.\n",
    );
    assert_eq!(
        (done.status, done.out.as_str()),
        (0, "ACM-5: comment added\n"),
        "{}",
        done.err
    );
    assert_eq!(memory.wrote(), 1);
    let sent = memory.of("CommentCreate");
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(MINE), "{}", sent[0]);
    assert!(sent[0].contains("Done; the limiter is in."), "{}", sent[0]);
}

#[test]
fn an_agent_is_refused_any_other_issue_before_anything_is_written() {
    for args in [
        &["update", OTHER, "--status", "Done"][..],
        &["update", "ACM-6", "--status", "Done"],
        &["update", MINE, OTHER, "--status", "Done"],
        &["update", "ACM-5", "ACM-6", "--status", "Done"],
        &["comment", OTHER, "--body", "x"],
        &["comment", "ACM-6", "--body", "x"],
    ] {
        let memory = agent_world();
        let done = run_guarded(&memory, for_agent(), args, "");
        assert_eq!(done.status, 1, "{args:?}: {}", done.err);
        assert_eq!(
            done.err,
            "linearpen: linearpen was started for an agent on another issue; it may change only that issue's status and comment on it\n",
            "{args:?}"
        );
        assert_eq!(memory.wrote(), 0, "{args:?}");
        assert!(done.out.is_empty(), "{args:?}");
    }
}

#[test]
fn an_agent_makes_no_issue_and_changes_nothing_but_the_status_and_asks_linear_nothing_first() {
    for (args, said) in [
        (
            &["create", "--team", "ACM", "--title", "x"][..],
            "it makes no issues",
        ),
        (
            &["update", MINE, "--priority", "low"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--title", "New"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--label", "bug"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--unlabel", "bug"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--assignee", "me"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--project", "none"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--cycle", "none"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--estimate", "none"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--due", "none"],
            "it changes only the status",
        ),
        (
            &["update", MINE, "--status", "Done", "--priority", "low"],
            "it changes only the status",
        ),
    ] {
        let memory = agent_world();
        let done = run_guarded(&memory, for_agent(), args, "");
        assert_eq!(done.status, 1, "{args:?}: {}", done.err);
        assert!(done.err.contains(said), "{args:?}: {}", done.err);
        assert!(
            done.err
                .starts_with("linearpen: linearpen was started for an agent"),
            "{args:?}: {}",
            done.err
        );
        // Not a word went to Linear: not a write, and not a read.
        assert!(memory.sent().is_empty(), "{args:?}: {:?}", memory.sent());
    }
}

#[test]
fn an_agent_whose_variable_names_no_issue_writes_nothing_at_all() {
    for args in [
        &["create", "--team", "ACM", "--title", "x"][..],
        &["update", "ACM-5", "--status", "Done"],
        &["update", MINE, "--status", "Done"],
        &["comment", MINE, "--body", "x"],
        &["comment", "ACM-5", "--body", "-"],
    ] {
        for value in ["", "ACM-5", "not-an-id"] {
            let memory = agent_world();
            let done = run_guarded(
                &memory,
                Guard::of(Some(std::ffi::OsStr::new(value))),
                args,
                "words",
            );
            assert_eq!(done.status, 1, "{args:?} {value:?}: {}", done.err);
            assert_eq!(
                done.err,
                "linearpen: linearpen was started for an agent with no issue of its own; it writes nothing\n",
                "{args:?} {value:?}"
            );
            assert!(memory.sent().is_empty(), "{args:?} {value:?}");
        }
    }
}

#[test]
fn an_agent_reads_what_it_likes() {
    for guard in [for_agent(), Guard::Nothing] {
        let memory = world();
        let done = run_guarded(&memory, guard.clone(), &["show", "ACM-412"], "");
        assert_eq!(done.status, 0, "{guard:?}: {}", done.err);
        assert!(done.out.contains("ACM-412"), "{}", done.out);
        let done = run_guarded(&memory, guard.clone(), &["teams"], "");
        assert_eq!(done.status, 0, "{guard:?}: {}", done.err);
        let done = run_guarded(&memory, guard.clone(), &["teams", "ACM"], "");
        assert_eq!(done.status, 0, "{guard:?}: {}", done.err);
        assert_eq!(memory.wrote(), 0);
    }
}

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

// The two commands the agent is told to run are commands `linearpen` takes, as the program the agent
// is told to run is written whatever its path is, and what they come to is what the guard lets by.
#[test]
fn the_two_commands_the_prompt_gives_the_agent_are_the_two_the_command_line_takes() {
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
        );
        // The commands are what is between the backticks.
        let spans: Vec<&str> = said.split('`').skip(1).step_by(2).collect();
        assert_eq!(spans.len(), 2, "{said}");
        for span in &spans {
            let words = shell_words(&span.replace("<status>", "In Review"));
            assert_eq!(words[0], exe.unwrap_or("linearpen"), "{span}");
        }
        let update = shell_words(&spans[0].replace("<status>", "In Review"));
        let Asked::Do(Command::Update(update)) = command_line::read(update[1..].iter().cloned())
        else {
            panic!("not an update: {}", spans[0]);
        };
        assert_eq!(update.issues, [MINE]);
        assert_eq!(update.fields.status.as_deref(), Some("In Review"));
        // It is what the guard of that very run lets by.
        for_agent().updates(&update).unwrap();
        let comment = shell_words(spans[1]);
        let Asked::Do(Command::Comment(comment)) = command_line::read(comment[1..].iter().cloned())
        else {
            panic!("not a comment: {}", spans[1]);
        };
        assert_eq!(comment.issue, MINE);
        assert_eq!(comment.body, crate::command_line::Words::Stdin);
        for_agent().comments().unwrap();
    }
}
