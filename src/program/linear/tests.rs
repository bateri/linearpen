//! The client against a transport of memory: pages, the limit, and what each kind of answer is.

use std::sync::Mutex;

use super::transport::Reply;
use super::*;

/// Answers from a list, one reply to each request, and keeps the bodies it was sent.
struct Scripted {
    replies: Mutex<Vec<Result<Reply, Error>>>,
    sent: Mutex<Vec<String>>,
}

impl Scripted {
    fn new(replies: Vec<Result<Reply, Error>>) -> Self {
        let mut replies = replies;
        replies.reverse();
        Self {
            replies: Mutex::new(replies),
            sent: Mutex::new(Vec::new()),
        }
    }

    /// The same reply, however many times it is asked for.
    fn always(reply: Reply, times: usize) -> Self {
        Self::new((0..times).map(|_| Ok(reply.clone())).collect())
    }

    fn sent(&self) -> Vec<String> {
        self.sent.lock().unwrap().clone()
    }
}

impl Transport for Scripted {
    fn post(&self, _: &Key, body: &str) -> Result<Reply, Error> {
        self.sent.lock().unwrap().push(body.to_owned());
        self.replies
            .lock()
            .unwrap()
            .pop()
            .unwrap_or_else(|| Err(Error::Unreachable("no more replies".to_owned())))
    }
}

fn key() -> Key {
    Key::new("lin_api_test").unwrap()
}

fn issue(n: usize) -> String {
    format!(
        r#"{{"id":"i{n}","identifier":"ACM-{n}","title":"t{n}","priority":3,
        "updatedAt":"2026-11-09T14:20:00.000Z","url":"https://linear.app/x/{n}",
        "state":{{"id":"s","name":"Todo","type":"unstarted","position":0}},
        "labels":{{"nodes":[]}},"team":{{"id":"team-acm","key":"ACM"}},"cycle":null,"project":null,"assignee":null}}"#
    )
}

const VIEWER: &str = r#""id":"u1","name":"Ada","organization":{"name":"Acme","urlKey":"acme"}"#;

/// A page of the workspace's issues: `count` of them, numbered from `from`.
fn all_page(from: usize, count: usize, more: bool) -> Reply {
    let nodes: Vec<String> = (from..from + count).map(issue).collect();
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"viewer":{{{VIEWER}}},"issues":{{"nodes":[{}],"pageInfo":{{"hasNextPage":{more},"endCursor":"c{}"}}}}}}}}"#,
            nodes.join(","),
            from + count
        ),
    }
}

fn mine_page(count: usize) -> Reply {
    let nodes: Vec<String> = (0..count).map(issue).collect();
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"viewer":{{{VIEWER},"assignedIssues":{{"nodes":[{}],"pageInfo":{{"hasNextPage":false,"endCursor":null}}}}}}}}}}"#,
            nodes.join(",")
        ),
    }
}

#[test]
fn my_issues_are_one_page_when_that_is_all_there_is() {
    let transport = Scripted::new(vec![Ok(mine_page(3))]);
    let fetched = fetch(&transport, &key(), Scope::Mine).unwrap();
    assert_eq!(fetched.listing.issues.len(), 3);
    assert!(!fetched.listing.truncated);
    assert_eq!(fetched.viewer.organization.url_key, "acme");
    assert_eq!(transport.sent().len(), 1);
}

#[test]
fn pages_follow_the_cursor_until_there_is_no_next() {
    let transport = Scripted::new(vec![
        Ok(all_page(0, 100, true)),
        Ok(all_page(100, 40, false)),
    ]);
    let fetched = fetch(&transport, &key(), Scope::All).unwrap();
    assert_eq!(fetched.listing.issues.len(), 140);
    assert!(!fetched.listing.truncated);
    let sent = transport.sent();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].contains(r#""after":null"#), "{}", sent[0]);
    assert!(sent[1].contains(r#""after":"c100""#), "{}", sent[1]);
    assert!(sent[0].contains(r#""first":100"#));
}

#[test]
fn a_list_stops_at_five_hundred_and_says_there_was_more() {
    // A workspace that never runs out: the sixth page is not asked for.
    let transport = Scripted::new((0..8).map(|n| Ok(all_page(n * 100, 100, true))).collect());
    let fetched = fetch(&transport, &key(), Scope::All).unwrap();
    assert_eq!(fetched.listing.issues.len(), LIMIT);
    assert!(fetched.listing.truncated);
    assert_eq!(transport.sent().len(), 5);
}

#[test]
fn exactly_five_hundred_is_not_a_truncation() {
    let transport = Scripted::new((0..5).map(|n| Ok(all_page(n * 100, 100, n < 4))).collect());
    let fetched = fetch(&transport, &key(), Scope::All).unwrap();
    assert_eq!(fetched.listing.issues.len(), 500);
    assert!(!fetched.listing.truncated);
}

#[test]
fn no_cursor_ends_the_pages_even_if_linear_says_there_is_more() {
    let reply = Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"viewer":{{{VIEWER}}},"issues":{{"nodes":[],"pageInfo":{{"hasNextPage":true,"endCursor":null}}}}}}}}"#
        ),
    };
    let transport = Scripted::always(reply, 3);
    let fetched = fetch(&transport, &key(), Scope::All).unwrap();
    assert!(fetched.listing.issues.is_empty());
    assert_eq!(transport.sent().len(), 1);
}

#[test]
fn the_request_is_a_plain_query_with_its_variables() {
    let transport = Scripted::new(vec![Ok(mine_page(0))]);
    fetch(&transport, &key(), Scope::Mine).unwrap();
    let sent = transport.sent();
    assert!(
        sent[0].contains(r#""operationName":"MyIssues""#),
        "{}",
        sent[0]
    );
    assert!(sent[0].contains(r#""nin":["completed","canceled"]"#));
    assert!(!sent[0].to_lowercase().contains("mutation"));
}

#[test]
fn a_401_is_refused_whatever_the_body() {
    let reply = Reply {
        status: 401,
        body: "<html>no</html>".to_owned(),
    };
    let transport = Scripted::always(reply, 1);
    assert_eq!(verify(&transport, &key()), Err(Error::Refused));
}

#[test]
fn an_authentication_error_in_a_200_is_refused_too() {
    let reply = Reply {
        status: 200,
        body: r#"{"errors":[{"message":"Authentication required","extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#.to_owned(),
    };
    let transport = Scripted::always(reply, 1);
    assert_eq!(verify(&transport, &key()), Err(Error::Refused));
}

#[test]
fn graphql_errors_are_failed_with_the_words_cleaned() {
    let reply = Reply {
        status: 400,
        body: "{\"errors\":[{\"message\":\"Bad field\\n\\u001b[31mx\"}]}".to_owned(),
    };
    let transport = Scripted::always(reply, 1);
    let Err(Error::Failed(words)) = verify(&transport, &key()) else {
        panic!("failed");
    };
    assert_eq!(words, "Linear said: Bad field [31mx");
}

#[test]
fn another_status_or_a_body_that_is_not_json_is_failed() {
    for (status, body) in [(500, "oops"), (200, "oops"), (502, "")] {
        let transport = Scripted::always(
            Reply {
                status,
                body: body.to_owned(),
            },
            1,
        );
        let got = verify(&transport, &key());
        assert!(
            matches!(got, Err(Error::Failed(_))),
            "{status} {body}: {got:?}"
        );
    }
    let no_data = Reply {
        status: 200,
        body: r#"{"data":null}"#.to_owned(),
    };
    assert!(matches!(
        verify(&Scripted::always(no_data, 1), &key()),
        Err(Error::Failed(_))
    ));
}

#[test]
fn an_unreachable_transport_is_unreachable() {
    let transport = Scripted::new(vec![Err(Error::Unreachable("down".to_owned()))]);
    assert_eq!(
        fetch(&transport, &key(), Scope::Mine),
        Err(Error::Unreachable("down".to_owned()))
    );
}

#[test]
fn a_mutation_is_not_sent_at_all() {
    let bad = Operation {
        name: "Bad",
        text: "mutation Bad { issueDelete(id: \"x\") { success } }",
    };
    let transport = Scripted::new(Vec::new());
    let got: Result<VerifyData, Error> = run::<(), _>(&transport, &key(), &bad, None);
    assert!(matches!(got, Err(Error::Failed(_))));
    assert!(transport.sent().is_empty());
}

#[test]
fn one_issue_comes_with_its_description_and_comments_and_the_id_is_a_variable() {
    let body = format!(
        r#"{{"data":{{"issue":{}}}}}"#,
        issue(7).replacen(
            r#""team":"#,
            r#""description":"The words","comments":{"nodes":[{"body":"b","createdAt":"2026-11-06T09:12:00.000Z","user":null}],"pageInfo":{"hasNextPage":true,"endCursor":"c"}},"team":"#,
            1
        )
    );
    let transport = Scripted::new(vec![Ok(Reply { status: 200, body })]);
    let found = fetch_issue(&transport, &key(), "abc-123").unwrap();
    assert_eq!(found.description.as_deref(), Some("The words"));
    let comments = found.comments.unwrap();
    assert_eq!(comments.nodes.len(), 1);
    assert!(comments.nodes[0].user.is_none());
    assert!(comments.page_info.has_next_page);
    let sent = transport.sent();
    assert!(
        sent[0].contains(r#""operationName":"Issue""#),
        "{}",
        sent[0]
    );
    // What the person's page names is a variable, not a part of the text.
    assert!(
        sent[0].contains(r#""variables":{"id":"abc-123"}"#),
        "{}",
        sent[0]
    );
    assert!(!sent[0].to_lowercase().contains("mutation"));
}

#[test]
fn an_issue_that_is_not_there_is_said() {
    let reply = Reply {
        status: 200,
        body: r#"{"data":{"issue":null}}"#.to_owned(),
    };
    let transport = Scripted::always(reply, 1);
    assert!(matches!(
        fetch_issue(&transport, &key(), "nope"),
        Err(Error::Failed(_))
    ));
}

#[test]
fn a_mutation_that_is_not_listed_is_not_sent_at_all() {
    // The name says the one that is listed, the text is another: the text is what is read.
    let disguised = Operation {
        name: "IssueCreate",
        text: "mutation IssueDelete($id: String!) { issueDelete(id: $id) { success } }",
    };
    // A query that carries a mutation behind it.
    let hidden = Operation {
        name: "Hidden",
        text: "query Hidden { viewer { id } } mutation IssueCreate { issueCreate { success } }",
    };
    let transport = Scripted::new(Vec::new());
    for bad in [disguised, hidden] {
        let got: Result<VerifyData, Error> = run::<(), _>(&transport, &key(), &bad, None);
        assert!(matches!(got, Err(Error::Failed(_))), "{}", bad.name);
    }
    assert!(transport.sent().is_empty());
}

/// Each mutation leaves from its own function and from nowhere else: the places that name it are the
/// file that lists it and the one function that sends it.
#[test]
fn only_create_and_change_name_the_mutations() {
    fn sources(dir: &std::path::Path, found: &mut Vec<(std::path::PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                found.push((path, text));
            }
        }
    }
    let mut found = Vec::new();
    sources(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program"),
        &mut found,
    );
    for (path, text) in &found {
        let name = path.file_name().unwrap().to_str().unwrap();
        let uses = text.matches("ISSUE_CREATE").count();
        let batches = text.matches("ISSUE_BATCH_UPDATE").count();
        match (
            path.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            name,
        ) {
            // Where they are defined and listed (and their guards).
            ("linear", "query.rs") => assert!(uses >= 1 && batches >= 1),
            // Where each is sent: once.
            ("linear", "mod.rs") => {
                assert_eq!(uses, 1, "{}", path.display());
                assert_eq!(batches, 1, "{}", path.display());
            }
            // The tests of the client may name them.
            ("linear", "tests.rs") => {}
            _ => assert_eq!(uses + batches, 0, "{} names a mutation", path.display()),
        }
    }
    // Nothing else posts a body to Linear but `run`, which reads the operation before it sends it,
    // and the function that makes the issue is called from the session alone.
    for (path, text) in &found {
        let name = path.file_name().unwrap().to_str().unwrap();
        if name == "tests.rs" || name == "page_tests.rs" {
            continue;
        }
        let posts = text.matches(".post(").count();
        let expected = usize::from(path.ends_with("linear/mod.rs"));
        assert_eq!(posts, expected, "{} posts to Linear", path.display());
        let makes = text.matches("linear::create(").count();
        let expected = usize::from(path.ends_with("program/session.rs"));
        assert_eq!(makes, expected, "{} makes an issue", path.display());
        let changes = text.matches("linear::change(").count();
        assert_eq!(changes, expected, "{} changes issues", path.display());
    }
}

const TEAM_ID: &str = "team-acm";

/// What `NewIssueOptions` answers: states in no order, a group among the labels, a finished project,
/// a member who has left, and a label the workspace has that the team has not.
const OPTIONS: &str = r##"{"data":{"team":{"id":"team-acm","defaultIssueState":{"id":"s-todo"},
"states":{"nodes":[
 {"id":"s-prog","name":"In Progress","type":"started","position":2},
 {"id":"s-done","name":"Done","type":"completed","position":3},
 {"id":"s-todo","name":"Todo","type":"unstarted","position":1},
 {"id":"s-back","name":"Backlog","type":"backlog","position":0},
 {"id":"s-tri","name":"Triage","type":"triage","position":0},
 {"id":"s-rev","name":"In Review","type":"started","position":1},
 {"id":"s-cancel","name":"Canceled","type":"canceled","position":4}]},
"labels":{"nodes":[
 {"id":"l-bug","name":"bug","color":"#eb5757","isGroup":false},
 {"id":"l-area","name":"Area","color":"#999999","isGroup":true},
 {"id":"l-api","name":"api","color":"#3e63dd","isGroup":false}]},
"projects":{"nodes":[
 {"id":"p-1","name":"Billing","completedAt":null,"canceledAt":null},
 {"id":"p-2","name":"Old","completedAt":"2026-01-01T00:00:00.000Z","canceledAt":null},
 {"id":"p-3","name":"Dropped","completedAt":null,"canceledAt":"2026-02-01T00:00:00.000Z"}]},
"members":{"nodes":[
 {"id":"u-deniz","displayName":"deniz","isMe":false,"active":true},
 {"id":"u-gone","displayName":"Gone","isMe":false,"active":false},
 {"id":"u-ada","displayName":"Ada","isMe":true,"active":true}]}},
"issueLabels":{"nodes":[
 {"id":"l-bug","name":"bug","color":"#eb5757","isGroup":false},
 {"id":"l-ws","name":"Customer","color":"#f2994a","isGroup":false}]}}}"##;

fn asked_options(body: &str) -> Options {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: body.to_owned(),
    })]);
    let got = options(&transport, &key(), TEAM_ID).unwrap();
    let sent = transport.sent();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].contains(r#""operationName":"NewIssueOptions""#),
        "{}",
        sent[0]
    );
    // Which team is a variable, not a part of the text.
    assert!(
        sent[0].contains(r#""variables":{"team":"team-acm"}"#),
        "{}",
        sent[0]
    );
    got
}

#[test]
fn the_choices_are_the_open_states_in_order_the_labels_without_groups_the_projects_not_finished_and_the_members_who_are_there()
 {
    let got = asked_options(OPTIONS);
    assert_eq!(got.team, "team-acm");
    let states: Vec<&str> = got.states.iter().map(|s| s.name.as_str()).collect();
    // Triage, backlog, unstarted, started; within a kind by position. Done and Canceled are not offered.
    assert_eq!(
        states,
        ["Triage", "Backlog", "Todo", "In Review", "In Progress"]
    );
    assert_eq!(got.states[2].kind, types::StateKind::Unstarted);
    // Every state an issue can be put in: then Done and Canceled, in the order of their kinds.
    let all: Vec<&str> = got.all_states.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        all,
        [
            "Triage",
            "Backlog",
            "Todo",
            "In Review",
            "In Progress",
            "Done",
            "Canceled"
        ]
    );
    // The team's labels and the workspace's, once each, by name whatever the case, with no group.
    let labels: Vec<&str> = got.labels.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(labels, ["api", "bug", "Customer"]);
    assert_eq!(got.labels[0].id, "l-api");
    assert_eq!(got.labels[0].color, "#3e63dd");
    let projects: Vec<&str> = got.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(projects, ["Billing"]);
    let members: Vec<(&str, bool)> = got
        .members
        .iter()
        .map(|m| (m.name.as_str(), m.me))
        .collect();
    assert_eq!(members, [("Ada", true), ("deniz", false)]);
    assert_eq!(got.default_state.as_deref(), Some("s-todo"));
}

#[test]
fn the_state_a_new_issue_begins_in_is_the_teams_default_else_the_first_unstarted_else_the_first_backlog()
 {
    let with = |default: &str, states: &str| {
        let body = format!(
            r#"{{"data":{{"team":{{"id":"team-acm","defaultIssueState":{default},"states":{{"nodes":[{states}]}},
            "labels":{{"nodes":[]}},"projects":{{"nodes":[]}},"members":{{"nodes":[]}}}},"issueLabels":{{"nodes":[]}}}}}}"#
        );
        asked_options(&body).default_state
    };
    let backlog = r#"{"id":"b","name":"Backlog","type":"backlog","position":0}"#;
    let todo2 = r#"{"id":"t2","name":"Next","type":"unstarted","position":2}"#;
    let todo1 = r#"{"id":"t1","name":"Todo","type":"unstarted","position":1}"#;
    let done = r#"{"id":"d","name":"Done","type":"completed","position":3}"#;
    let states = format!("{backlog},{todo2},{todo1},{done}");
    assert_eq!(
        with(r#"{"id":"t2"}"#, &states).as_deref(),
        Some("t2"),
        "the team's own"
    );
    assert_eq!(
        with("null", &states).as_deref(),
        Some("t1"),
        "the first unstarted"
    );
    assert_eq!(
        with("null", backlog).as_deref(),
        Some("b"),
        "the first backlog"
    );
    // A default that is not among the states offered (it is a finished one) is not used.
    assert_eq!(
        with(r#"{"id":"d"}"#, &states).as_deref(),
        Some("t1"),
        "not a state that is finished"
    );
    assert_eq!(with("null", done), None, "nothing to begin in");
}

#[test]
fn what_linear_names_is_plain_in_the_choices() {
    let body = OPTIONS.replace("Billing", "Bill\\u001b[31ming");
    let got = asked_options(&body);
    assert_eq!(got.projects[0].name, "Bill [31ming");
}

#[test]
fn a_team_that_is_not_there_is_said() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: r#"{"data":{"team":null,"issueLabels":{"nodes":[]}}}"#.to_owned(),
    })]);
    assert!(matches!(
        options(&transport, &key(), "nope"),
        Err(Error::Failed(_))
    ));
}

#[test]
fn the_teams_are_asked_for_by_name_and_listed_by_name() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: r#"{"data":{"teams":{"nodes":[
            {"id":"t2","key":"WEB","name":"web"},{"id":"t1","key":"ACM","name":"Acme"}]}}}"#
            .to_owned(),
    })]);
    let got = teams(&transport, &key()).unwrap();
    let keys: Vec<&str> = got.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["ACM", "WEB"]);
    assert_eq!(got[0].id, "t1");
    assert!(
        transport.sent()[0].contains(r#""operationName":"NewIssueTeams""#),
        "{}",
        transport.sent()[0]
    );
}

fn new_issue() -> NewIssue {
    NewIssue {
        id: "11111111-2222-4333-8444-555555555555".to_owned(),
        team_id: "team-acm".to_owned(),
        title: "Fix the login".to_owned(),
        description: None,
        state_id: None,
        priority: Priority::None,
        assignee_id: None,
        label_ids: Vec::new(),
        project_id: None,
    }
}

#[test]
fn the_input_leaves_out_what_is_empty_and_says_the_priority_as_a_number() {
    let bare = serde_json::to_value(new_issue()).unwrap();
    assert_eq!(
        bare,
        serde_json::json!({
            "id": "11111111-2222-4333-8444-555555555555",
            "teamId": "team-acm",
            "title": "Fix the login"
        })
    );
    let full = serde_json::to_value(NewIssue {
        description: Some("The words".to_owned()),
        state_id: Some("s-todo".to_owned()),
        priority: Priority::High,
        assignee_id: Some("u-ada".to_owned()),
        label_ids: vec!["l-api".to_owned(), "l-bug".to_owned()],
        project_id: Some("p-1".to_owned()),
        ..new_issue()
    })
    .unwrap();
    assert_eq!(
        full,
        serde_json::json!({
            "id": "11111111-2222-4333-8444-555555555555",
            "teamId": "team-acm",
            "title": "Fix the login",
            "description": "The words",
            "stateId": "s-todo",
            "priority": 2,
            "assigneeId": "u-ada",
            "labelIds": ["l-api", "l-bug"],
            "projectId": "p-1"
        })
    );
    for (priority, number) in [
        (Priority::None, 0),
        (Priority::Urgent, 1),
        (Priority::High, 2),
        (Priority::Medium, 3),
        (Priority::Low, 4),
    ] {
        assert_eq!(priority.number(), number);
        assert_eq!(Priority::parse(priority.name()), Some(priority));
    }
    assert_eq!(Priority::parse("loud"), None);
}

const MADE: &str = r#"{"data":{"issueCreate":{"success":true,"issue":{"id":"issue-900","identifier":"ACM-900"}}}}"#;

#[test]
fn create_sends_the_one_mutation_once_with_the_draft_id_as_a_variable() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: MADE.to_owned(),
    })]);
    let made = create(&transport, &key(), &new_issue()).unwrap();
    assert_eq!(made.id, "issue-900");
    assert_eq!(made.identifier, "ACM-900");
    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "one request, however it ends");
    let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(body["operationName"], "IssueCreate");
    assert!(
        body["query"]
            .as_str()
            .unwrap()
            .starts_with("mutation IssueCreate(")
    );
    assert_eq!(
        body["variables"]["input"]["id"],
        "11111111-2222-4333-8444-555555555555"
    );
    // What the person typed is a variable: it is not in the text of the mutation.
    assert!(!body["query"].as_str().unwrap().contains("Fix the login"));
}

#[test]
fn a_create_that_says_it_did_not_succeed_is_a_failure() {
    for body in [
        r#"{"data":{"issueCreate":{"success":false,"issue":null}}}"#,
        r#"{"data":{"issueCreate":{"success":true,"issue":null}}}"#,
        r#"{"data":{"issueCreate":null}}"#,
    ] {
        let transport = Scripted::new(vec![Ok(Reply {
            status: 200,
            body: body.to_owned(),
        })]);
        assert!(
            matches!(
                create(&transport, &key(), &new_issue()),
                Err(Error::Failed(_))
            ),
            "{body}"
        );
        assert_eq!(transport.sent().len(), 1);
    }
}

#[test]
fn a_key_that_may_not_write_is_forbidden_not_refused() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 403,
        body: r#"{"errors":[{"message":"Forbidden"}]}"#.to_owned(),
    })]);
    assert_eq!(
        create(&transport, &key(), &new_issue()),
        Err(Error::Forbidden)
    );
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: r#"{"errors":[{"message":"x","extensions":{"type":"forbidden"}}]}"#.to_owned(),
    })]);
    assert_eq!(
        create(&transport, &key(), &new_issue()),
        Err(Error::Forbidden)
    );
}

#[test]
fn a_state_that_is_a_duplicate_is_not_offered_to_change_to() {
    let body = r#"{"data":{"team":{"id":"team-acm","defaultIssueState":null,"states":{"nodes":[
        {"id":"s-todo","name":"Todo","type":"unstarted","position":1},
        {"id":"s-done","name":"Done","type":"completed","position":2},
        {"id":"s-cancel","name":"Canceled","type":"canceled","position":3},
        {"id":"s-dup","name":"Duplicate","type":"canceled","position":4},
        {"id":"s-odd","name":"Odd","type":"duplicate","position":5}]},
        "labels":{"nodes":[]},"projects":{"nodes":[]},"members":{"nodes":[]}},"issueLabels":{"nodes":[]}}}"#;
    let got = asked_options(body);
    let all: Vec<&str> = got.all_states.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(all, ["Todo", "Done", "Canceled"]);
    let open: Vec<&str> = got.states.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(open, ["Todo"]);
}

/// What `IssueBatchUpdate` answers: the issues as they are now.
fn changed_reply(success: bool, issues: &[usize]) -> Reply {
    let nodes: Vec<String> = issues.iter().copied().map(issue).collect();
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"issueBatchUpdate":{{"success":{success},"issues":[{}]}}}}}}"#,
            nodes.join(",")
        ),
    }
}

fn ids(of: &[&str]) -> Vec<String> {
    of.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn a_change_is_sent_once_with_the_ids_and_its_one_field_as_variables() {
    let transport = Scripted::new(vec![Ok(changed_reply(true, &[1, 2]))]);
    let got = change(
        &transport,
        &key(),
        &ids(&["i1", "i2"]),
        &Change::State("s-done".to_owned()),
    )
    .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[0].team.as_ref().map(|t| t.id.as_str()),
        Some("team-acm")
    );
    let sent = transport.sent();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].contains(r#""operationName":"IssueBatchUpdate""#),
        "{}",
        sent[0]
    );
    assert!(
        sent[0].contains(r#""variables":{"ids":["i1","i2"],"input":{"stateId":"s-done"}}"#),
        "{}",
        sent[0]
    );
    // What the person wrote or chose is a variable, never a part of the text; and the key is not in it.
    assert!(!sent[0].contains("lin_api"), "{}", sent[0]);
}

#[test]
fn a_change_that_linear_does_not_make_is_an_error_and_is_not_sent_again() {
    for reply in [
        changed_reply(false, &[]),
        Reply {
            status: 200,
            body: r#"{"data":{"issueBatchUpdate":null}}"#.to_owned(),
        },
        Reply {
            status: 200,
            body: r#"{"errors":[{"message":"state does not belong to the team"}],"data":null}"#
                .to_owned(),
        },
    ] {
        let transport = Scripted::new(vec![Ok(reply)]);
        let got = change(
            &transport,
            &key(),
            &ids(&["i1"]),
            &Change::Title("x".to_owned()),
        );
        assert!(matches!(got, Err(Error::Failed(_))), "{got:?}");
        assert_eq!(transport.sent().len(), 1, "never sent twice");
    }
    // No answer is the transport's error, and it is not sent again either.
    let transport = Scripted::new(vec![Err(Error::Unreachable("timed out".to_owned()))]);
    let got = change(
        &transport,
        &key(),
        &ids(&["i1"]),
        &Change::Title("x".to_owned()),
    );
    assert!(matches!(got, Err(Error::Unreachable(_))));
    assert_eq!(transport.sent().len(), 1);
}

#[test]
fn from_one_to_fifty_issues_are_changed_at_once_and_no_more() {
    let transport = Scripted::new(vec![Ok(changed_reply(true, &[]))]);
    let many: Vec<String> = (0..=BATCH_MOST).map(|n| format!("i{n}")).collect();
    for bad in [Vec::new(), many.clone()] {
        let got = change(&transport, &key(), &bad, &Change::Title("x".to_owned()));
        assert!(matches!(got, Err(Error::Failed(_))), "{}", bad.len());
    }
    assert!(transport.sent().is_empty(), "nothing is sent");
    let fifty = &many[..BATCH_MOST];
    assert!(change(&transport, &key(), fifty, &Change::Title("x".to_owned())).is_ok());
}

#[test]
fn no_change_asks_for_the_description_or_the_team_to_be_written() {
    for change_made in [
        Change::State("s".to_owned()),
        Change::Priority(Priority::Low),
        Change::Assignee(None),
        Change::AddLabel("l".to_owned()),
        Change::RemoveLabel("l".to_owned()),
        Change::Project(None),
        Change::Title("t".to_owned()),
    ] {
        let transport = Scripted::new(vec![Ok(changed_reply(true, &[1]))]);
        change(&transport, &key(), &ids(&["i1"]), &change_made).unwrap();
        let sent = transport.sent();
        let variables = sent[0].split(r#""variables":"#).nth(1).unwrap();
        assert!(!variables.contains("description"), "{variables}");
        assert!(
            !variables.to_ascii_lowercase().contains("teamid"),
            "{variables}"
        );
    }
}
