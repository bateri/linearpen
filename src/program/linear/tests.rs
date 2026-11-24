//! The client against a transport of memory: pages, the limit, and what each kind of answer is.

use std::sync::Mutex;

use super::transport::Reply;
use super::upload::{send_file, upload};
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
    issue_in(n, "Todo", "unstarted")
}

/// An issue in the state `name` of the kind `kind`.
fn issue_in(n: usize, name: &str, kind: &str) -> String {
    format!(
        r#"{{"id":"i{n}","identifier":"ACM-{n}","title":"t{n}","priority":3,
        "updatedAt":"2026-11-09T14:20:00.000Z","url":"https://linear.app/x/{n}",
        "state":{{"id":"s-{kind}","name":"{name}","type":"{kind}","position":0}},
        "labels":{{"nodes":[]}},"team":{{"id":"team-acm","key":"ACM"}},"cycle":null,"project":null,"assignee":null}}"#
    )
}

/// The closed part of the filter of a list, and the open part.
const CLOSED_PART: &str = r#""in":["completed","canceled"]"#;
const OPEN_PART: &str = r#""nin":["completed","canceled"]"#;

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
    mine_of(&(0..count).map(issue).collect::<Vec<_>>())
}

/// A page of the person's issues that is `nodes`, and the last.
fn mine_of(nodes: &[String]) -> Reply {
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
    let transport = Scripted::new(vec![Ok(mine_page(3)), Ok(mine_page(0))]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 3);
    assert!(!fetched.listing.truncated);
    assert_eq!(fetched.viewer.organization.url_key, "acme");
    // The open issues, then the closed ones.
    let sent = transport.sent();
    assert_eq!(sent.len(), 2);
    assert!(sent[0].contains(OPEN_PART), "{}", sent[0]);
    assert!(sent[1].contains(CLOSED_PART), "{}", sent[1]);
}

#[test]
fn a_list_holds_the_open_issues_and_then_the_closed_ones() {
    let transport = Scripted::new(vec![
        Ok(mine_of(&[issue(1), issue_in(2, "In Progress", "started")])),
        Ok(mine_of(&[
            issue_in(3, "Done", "completed"),
            issue_in(4, "Canceled", "canceled"),
        ])),
    ]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    let held: Vec<(&str, &str)> = fetched
        .listing
        .issues
        .iter()
        .map(|issue| (issue.identifier.as_str(), issue.state.name.as_str()))
        .collect();
    assert_eq!(
        held,
        [
            ("ACM-1", "Todo"),
            ("ACM-2", "In Progress"),
            ("ACM-3", "Done"),
            ("ACM-4", "Canceled")
        ]
    );
    assert!(!fetched.listing.truncated);
}

#[test]
fn an_issue_closed_between_the_two_questions_is_held_once_as_it_is_now() {
    let transport = Scripted::new(vec![
        Ok(mine_of(&[issue(1), issue(2)])),
        Ok(mine_of(&[issue_in(2, "Done", "completed")])),
    ]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    let held: Vec<(&str, &str)> = fetched
        .listing
        .issues
        .iter()
        .map(|issue| (issue.identifier.as_str(), issue.state.name.as_str()))
        .collect();
    assert_eq!(held, [("ACM-1", "Todo"), ("ACM-2", "Done")]);
}

/// The filters of a list that asks for the state `name`.
fn status(name: &str) -> Filters {
    Filters::new(
        vec![name.to_owned()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

/// 340 open issues, then closed ones that never run out.
fn open_340_then_closed() -> Scripted {
    let mut replies: Vec<Result<Reply, Error>> = vec![
        Ok(all_page(0, 100, true)),
        Ok(all_page(100, 100, true)),
        Ok(all_page(200, 100, true)),
        Ok(all_page(300, 40, false)),
    ];
    replies.extend((0..3).map(|n| Ok(all_page(1000 + n * 100, 100, true))));
    Scripted::new(replies)
}

#[test]
fn a_list_takes_the_hundred_closed_issues_that_changed_last_and_is_not_cut_by_the_others() {
    let transport = open_340_then_closed();
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 340 + CLOSED_MOST);
    // Linear has more closed ones: they are not asked for, and the open ones are whole.
    assert!(!fetched.listing.truncated);
    let sent = transport.sent();
    assert_eq!(sent.len(), 5);
    assert!(sent[4].contains(CLOSED_PART) && sent[4].contains(r#""first":100"#));
}

#[test]
fn closed_issues_asked_for_by_state_fill_what_the_open_ones_leave_of_five_hundred() {
    // They are asked for 100, 60, and the list is cut.
    let transport = open_340_then_closed();
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &status("Done"),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), LIMIT);
    assert!(fetched.listing.truncated);
    let sent = transport.sent();
    assert_eq!(sent.len(), 6);
    assert!(sent[4].contains(CLOSED_PART) && sent[4].contains(r#""first":100"#));
    assert!(sent[5].contains(r#""first":60"#), "{}", sent[5]);
    assert!(sent[5].contains(r#""after":"c1100""#), "{}", sent[5]);
    // The text and the command line take them so too.
    let transport = open_340_then_closed();
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &status("Done"),
        Closed::Asked,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), LIMIT);
    assert_eq!(transport.sent().len(), 6);
}

#[test]
fn the_text_and_the_command_line_take_the_open_issues_alone_where_no_state_is_asked_for() {
    let transport = Scripted::new(vec![Ok(mine_of(&[
        issue(1),
        issue_in(2, "In Progress", "started"),
    ]))]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Asked,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 2);
    assert!(!fetched.listing.truncated);
    let sent = transport.sent();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains(OPEN_PART), "{}", sent[0]);
}

#[test]
fn a_listing_counts_its_open_issues() {
    let transport = Scripted::new(vec![
        Ok(mine_of(&[issue(1), issue_in(2, "In Progress", "started")])),
        Ok(mine_of(&[
            issue_in(3, "Done", "completed"),
            issue_in(4, "Canceled", "canceled"),
        ])),
    ]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 4);
    assert_eq!(fetched.listing.open(), 2);
}

#[test]
fn five_hundred_open_issues_leave_no_room_for_the_closed_ones() {
    // The closed ones are not asked for. The open ones are whole: the list is not cut.
    let transport = Scripted::new((0..5).map(|n| Ok(all_page(n * 100, 100, n < 4))).collect());
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 500);
    assert!(!fetched.listing.truncated);
    assert_eq!(transport.sent().len(), 5);
    // Where closed states are asked for by name, some may be there: the list is said to be cut.
    let transport = Scripted::new((0..5).map(|n| Ok(all_page(n * 100, 100, n < 4))).collect());
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &status("Done"),
        Closed::Recent,
    )
    .unwrap();
    assert!(fetched.listing.truncated);
    assert_eq!(transport.sent().len(), 5);
}

#[test]
fn pages_follow_the_cursor_until_there_is_no_next() {
    let transport = Scripted::new(vec![
        Ok(all_page(0, 100, true)),
        Ok(all_page(100, 40, false)),
        Ok(all_page(1000, 0, false)),
    ]);
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 140);
    assert!(!fetched.listing.truncated);
    let sent = transport.sent();
    assert_eq!(sent.len(), 3);
    assert!(sent[0].contains(r#""after":null"#), "{}", sent[0]);
    assert!(sent[1].contains(r#""after":"c100""#), "{}", sent[1]);
    assert!(sent[0].contains(r#""first":100"#));
}

#[test]
fn a_list_stops_at_five_hundred_and_says_there_was_more() {
    // A workspace that never runs out: the sixth page is not asked for.
    let transport = Scripted::new((0..8).map(|n| Ok(all_page(n * 100, 100, true))).collect());
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), LIMIT);
    assert!(fetched.listing.truncated);
    assert_eq!(transport.sent().len(), 5);
}

#[test]
fn exactly_five_hundred_found_is_not_a_truncation() {
    let transport = Scripted::new(
        (0..5)
            .map(|n| Ok(search_page(n * 100, 100, n < 4)))
            .collect(),
    );
    let scope = Scope::Search("checkout".to_owned());
    let fetched = fetch(
        &transport,
        &key(),
        &scope,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert_eq!(fetched.listing.issues.len(), 500);
    assert!(!fetched.listing.truncated);
    // A search asks once for every state: no part of the states is sent.
    let sent = transport.sent();
    assert_eq!(sent.len(), 5);
    assert!(sent.iter().all(|body| !body.contains("completed")));
}

/// A page of what a search finds: `count` issues, numbered from `from`.
fn search_page(from: usize, count: usize, more: bool) -> Reply {
    let nodes: Vec<String> = (from..from + count).map(issue).collect();
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"viewer":{{{VIEWER}}},"searchIssues":{{"nodes":[{}],"pageInfo":{{"hasNextPage":{more},"endCursor":"c{}"}}}}}}}}"#,
            nodes.join(","),
            from + count
        ),
    }
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
    let fetched = fetch(
        &transport,
        &key(),
        &Scope::All,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    assert!(fetched.listing.issues.is_empty());
    // One question for the open issues and one for the closed ones.
    assert_eq!(transport.sent().len(), 2);
}

#[test]
fn the_request_is_a_plain_query_with_its_variables() {
    let transport = Scripted::new(vec![Ok(mine_page(0)), Ok(mine_page(0))]);
    fetch(
        &transport,
        &key(),
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )
    .unwrap();
    let sent = transport.sent();
    for body in &sent {
        assert!(body.contains(r#""operationName":"MyIssues""#), "{body}");
        assert!(!body.to_lowercase().contains("mutation"));
    }
    assert!(sent[0].contains(OPEN_PART));
    assert!(sent[1].contains(CLOSED_PART));
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
        fetch(
            &transport,
            &key(),
            &Scope::Mine,
            &Filters::default(),
            Closed::Recent
        ),
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
fn only_create_change_and_comment_name_the_mutations() {
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
        let comments = text.matches("COMMENT_CREATE").count();
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
            ("linear", "query.rs") => assert!(uses >= 1 && batches >= 1 && comments >= 1),
            // Where each is sent: once.
            ("linear", "mod.rs") => {
                assert_eq!(uses, 1, "{}", path.display());
                assert_eq!(batches, 1, "{}", path.display());
                assert_eq!(comments, 0, "{}", path.display());
            }
            ("linear", "comment.rs") => {
                assert_eq!(comments, 1, "{}", path.display());
                assert_eq!(uses + batches, 0, "{}", path.display());
            }
            // The tests of the client may name them.
            ("linear", "tests.rs") => {}
            _ => assert_eq!(
                uses + batches + comments,
                0,
                "{} names a mutation",
                path.display()
            ),
        }
    }
    // Nothing else posts a body to Linear but `run`, which reads the operation before it sends it,
    // and the functions that write are called from the session and from the commands that write alone.
    for (path, text) in &found {
        let name = path.file_name().unwrap().to_str().unwrap();
        if name == "tests.rs" || name == "page_tests.rs" {
            continue;
        }
        let posts = text.matches(".post(").count();
        let expected = usize::from(path.ends_with("linear/mod.rs"));
        assert_eq!(posts, expected, "{} posts to Linear", path.display());
        // The two that call them: the session (the page's) and the commands that write.
        let session = usize::from(path.ends_with("program/session.rs"));
        let commands = usize::from(path.ends_with("program/cli/write.rs"));
        let makes = text.matches("linear::create(").count();
        assert_eq!(
            makes,
            session + commands,
            "{} makes an issue",
            path.display()
        );
        let changes = text.matches("linear::change(").count();
        assert_eq!(changes, session, "{} changes issues", path.display());
        let changes = text.matches("linear::change_all(").count();
        assert_eq!(
            changes,
            commands,
            "{} changes issues together",
            path.display()
        );
        let posts = text.matches("linear::comment(").count();
        assert_eq!(
            posts,
            session + commands,
            "{} posts a comment",
            path.display()
        );
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

/// The options of a team that has the given extra members of the wire (`"cyclesEnabled":true`, ...).
fn options_with(extra: &str) -> Options {
    let body = format!(
        r#"{{"data":{{"team":{{"id":"team-acm","defaultIssueState":null,"states":{{"nodes":[]}},
        "labels":{{"nodes":[]}},"projects":{{"nodes":[]}},"members":{{"nodes":[]}}{extra}}},
        "issueLabels":{{"nodes":[]}}}}}}"#
    );
    asked_options(&body)
}

#[test]
fn a_team_with_cycles_offers_the_active_one_then_the_next_six_and_a_team_without_offers_none() {
    let cycle = |id: &str, number: u32, name: &str| {
        format!(r#"{{"id":"{id}","number":{number}.0,"name":{name}}}"#)
    };
    let future: Vec<String> = [30, 26, 25, 27, 28, 29, 31, 32]
        .into_iter()
        .map(|n| cycle(&format!("c-{n}"), n, "null"))
        .collect();
    let on = options_with(&format!(
        r#","cyclesEnabled":true,"activeCycle":{},"cycles":{{"nodes":[{},{}]}}"#,
        cycle("c-24", 24, r#""Spring""#),
        cycle("c-24", 24, r#""Spring""#),
        future.join(",")
    ));
    assert!(on.cycles_enabled);
    let told: Vec<(u32, bool)> = on.cycles.iter().map(|c| (c.number, c.active)).collect();
    // The active one first (once), then by number; no more than six that are next.
    assert_eq!(
        told,
        [
            (24, true),
            (25, false),
            (26, false),
            (27, false),
            (28, false),
            (29, false),
            (30, false)
        ]
    );
    assert_eq!(on.cycles[0].name.as_deref(), Some("Spring"));
    assert_eq!(on.cycles[1].id, "c-25");

    let off = options_with(
        r#","cyclesEnabled":false,"activeCycle":{"id":"c-1","number":1,"name":null},"cycles":{"nodes":[]}"#,
    );
    assert!(!off.cycles_enabled && off.cycles.is_empty());
    // A team the wire says nothing of has none.
    let silent = asked_options(OPTIONS);
    assert!(!silent.cycles_enabled && silent.cycles.is_empty() && silent.estimates.is_empty());
}

#[test]
fn the_estimates_of_a_team_are_its_scale_with_t_shirts_sent_as_numbers_and_zero_only_where_allowed()
{
    let scale = |kind: &str, zero: bool, extended: bool| -> Vec<(u32, String)> {
        options_with(&format!(
            r#","issueEstimationType":"{kind}","issueEstimationAllowZero":{zero},"issueEstimationExtended":{extended}"#
        ))
        .estimates
        .into_iter()
        .map(|e| (e.value, e.label))
        .collect()
    };
    let plain = |values: &[u32]| -> Vec<(u32, String)> {
        values.iter().map(|v| (*v, v.to_string())).collect()
    };
    assert_eq!(scale("exponential", false, false), plain(&[1, 2, 4, 8, 16]));
    assert_eq!(
        scale("exponential", false, true),
        plain(&[1, 2, 4, 8, 16, 32, 64])
    );
    assert_eq!(scale("fibonacci", false, false), plain(&[1, 2, 3, 5, 8]));
    assert_eq!(
        scale("fibonacci", false, true),
        plain(&[1, 2, 3, 5, 8, 13, 21])
    );
    assert_eq!(scale("linear", false, false), plain(&[1, 2, 3, 4, 5]));
    assert_eq!(scale("linear", false, true), plain(&[1, 2, 3, 4, 5, 6, 7]));
    assert_eq!(scale("fibonacci", true, false), plain(&[0, 1, 2, 3, 5, 8]));
    // A t-shirt size is written as the Fibonacci number it stands for.
    let shirts = |values: &[(u32, &str)]| -> Vec<(u32, String)> {
        values.iter().map(|(v, l)| (*v, (*l).to_owned())).collect()
    };
    assert_eq!(
        scale("tShirt", false, false),
        shirts(&[(1, "XS"), (2, "S"), (3, "M"), (5, "L"), (8, "XL")])
    );
    assert_eq!(
        scale("tShirt", false, true),
        shirts(&[
            (1, "XS"),
            (2, "S"),
            (3, "M"),
            (5, "L"),
            (8, "XL"),
            (13, "XXL"),
            (21, "XXXL")
        ])
    );
    // Not estimating, and a way of estimating that is not known: no estimates, so no control.
    assert!(scale("notUsed", true, true).is_empty());
    assert!(scale("by-the-moon", false, false).is_empty());
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
        cycle_id: None,
        estimate: None,
        due_date: None,
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

const COMMENT_ID: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

const POSTED: &str = r#"{"data":{"commentCreate":{"success":true,"comment":{"id":"c-1","body":"Looks right to me","createdAt":"2026-11-12T09:30:00.000Z","user":{"displayName":"Ada"}}}}}"#;

#[test]
fn a_comment_is_sent_once_with_its_words_and_its_id_as_variables_and_not_in_the_text() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: POSTED.to_owned(),
    })]);
    let posted = comment(
        &transport,
        &key(),
        COMMENT_ID,
        "issue-412",
        "Looks right to me",
    )
    .unwrap();
    assert_eq!(posted.body, "Looks right to me");
    assert_eq!(posted.user.as_ref().unwrap().display_name, "Ada");
    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "one request, however it ends");
    let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(body["operationName"], "CommentCreate");
    let text = body["query"].as_str().unwrap();
    assert!(text.starts_with("mutation CommentCreate("));
    assert_eq!(body["variables"]["input"]["id"], COMMENT_ID);
    assert_eq!(body["variables"]["input"]["issueId"], "issue-412");
    assert_eq!(body["variables"]["input"]["body"], "Looks right to me");
    // The input is the three and nothing else: no parent, no author to pose as.
    assert_eq!(body["variables"]["input"].as_object().unwrap().len(), 3);
    // What the person typed is a variable: it is not in the text of the mutation.
    assert!(!text.contains("Looks right to me"));
}

#[test]
fn a_comment_that_says_it_was_not_posted_is_a_failure_that_leaves_nothing_open() {
    for body in [
        r#"{"data":{"commentCreate":{"success":false,"comment":null}}}"#,
        r#"{"data":{"commentCreate":{"success":true,"comment":null}}}"#,
        r#"{"data":{"commentCreate":null}}"#,
    ] {
        let transport = Scripted::new(vec![Ok(Reply {
            status: 200,
            body: body.to_owned(),
        })]);
        let failed = comment(&transport, &key(), COMMENT_ID, "issue-412", "x").unwrap_err();
        assert!(
            matches!(&failed, Error::Failed(why) if !left_open(why)),
            "{body}: {failed:?}"
        );
        assert_eq!(transport.sent().len(), 1);
    }
}

#[test]
fn a_key_that_may_not_comment_is_forbidden_and_one_linear_does_not_take_is_refused() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 403,
        body: r#"{"errors":[{"message":"Forbidden"}]}"#.to_owned(),
    })]);
    assert_eq!(
        comment(&transport, &key(), COMMENT_ID, "issue-412", "x"),
        Err(Error::Forbidden)
    );
    let transport = Scripted::new(vec![Ok(Reply {
        status: 401,
        body: String::new(),
    })]);
    assert_eq!(
        comment(&transport, &key(), COMMENT_ID, "issue-412", "x"),
        Err(Error::Refused)
    );
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

// --- The lists a person can go to: the issues of each, and their names --------------------------------

/// A page of the issues of one thing that has them (`root`: a team, a cycle, a view, a project, a
/// label): `count` of them, numbered from `from`.
fn owned_page(root: &str, from: usize, count: usize, more: bool) -> Reply {
    let nodes: Vec<String> = (from..from + count).map(issue).collect();
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"viewer":{{{VIEWER}}},"{root}":{{"issues":{{"nodes":[{}],"pageInfo":{{"hasNextPage":{more},"endCursor":"c{}"}}}}}}}}}}"#,
            nodes.join(","),
            from + count
        ),
    }
}

/// What a scope is asked with: the operation, and what the issues belong to in the answer.
fn sources() -> [(Scope, &'static str, &'static str); 5] {
    [
        (Scope::Team("t-acm".to_owned()), "TeamIssues", "team"),
        (Scope::Cycle("c-24".to_owned()), "CycleIssues", "cycle"),
        (Scope::View("v-1".to_owned()), "ViewIssues", "customView"),
        (Scope::Project("p-1".to_owned()), "ProjectIssues", "project"),
        (Scope::Label("l-1".to_owned()), "LabelIssues", "issueLabel"),
    ]
}

#[test]
fn each_source_asks_its_own_question_with_its_id_a_variable_and_the_open_filter() {
    for (scope, name, root) in sources() {
        let transport = Scripted::new(vec![
            Ok(owned_page(root, 0, 3, false)),
            Ok(owned_page(root, 100, 0, false)),
        ]);
        let fetched = fetch(
            &transport,
            &key(),
            &scope,
            &Filters::default(),
            Closed::Recent,
        )
        .unwrap();
        assert_eq!(fetched.listing.issues.len(), 3, "{name}");
        assert!(!fetched.listing.truncated, "{name}");
        assert_eq!(fetched.viewer.organization.url_key, "acme", "{name}");
        let sent = transport.sent();
        assert_eq!(sent.len(), 2, "{name}");
        // Then the closed ones, by the same question.
        let closed: serde_json::Value = serde_json::from_str(&sent[1]).unwrap();
        assert_eq!(closed["operationName"], name);
        assert_eq!(
            closed["variables"]["filter"]["state"]["type"]["in"],
            serde_json::json!(["completed", "canceled"]),
            "{name}"
        );
        let request: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
        assert_eq!(request["operationName"], name);
        let id = scope.id().unwrap();
        // The id is a variable, never a part of the text that is asked.
        assert_eq!(request["variables"]["id"], id, "{name}");
        assert!(!request["query"].as_str().unwrap().contains(id), "{name}");
        assert_eq!(
            request["variables"]["filter"]["state"]["type"]["nin"],
            serde_json::json!(["completed", "canceled"]),
            "{name}"
        );
        assert_eq!(request["variables"]["first"], 100, "{name}");
        assert!(!sent[0].to_lowercase().contains("mutation"), "{name}");
    }
}

#[test]
fn a_source_follows_the_cursor_and_stops_at_five_hundred_like_the_other_lists() {
    for (scope, name, root) in sources() {
        let transport = Scripted::new(
            (0..8)
                .map(|n| Ok(owned_page(root, n * 100, 100, true)))
                .collect(),
        );
        let fetched = fetch(
            &transport,
            &key(),
            &scope,
            &Filters::default(),
            Closed::Recent,
        )
        .unwrap();
        assert_eq!(fetched.listing.issues.len(), LIMIT, "{name}");
        assert!(fetched.listing.truncated, "{name}");
        assert_eq!(transport.sent().len(), 5, "{name}");
        // Four hundred open ones and the hundred closed ones that changed last is not a cut.
        let transport = Scripted::new(
            (0..5)
                .map(|n| Ok(owned_page(root, n * 100, 100, n < 3)))
                .collect(),
        );
        let fetched = fetch(
            &transport,
            &key(),
            &scope,
            &Filters::default(),
            Closed::Recent,
        )
        .unwrap();
        assert_eq!(fetched.listing.issues.len(), 500, "{name}");
        assert!(!fetched.listing.truncated, "{name}");
    }
}

#[test]
fn a_scope_is_told_and_read_by_its_kind_and_its_id() {
    let all = [
        (Scope::Mine, "mine", None),
        (Scope::All, "all", None),
        (Scope::Created, "created", None),
        (Scope::Subscribed, "subscribed", None),
        (Scope::Team("t".to_owned()), "team", Some("t")),
        (Scope::Cycle("c".to_owned()), "cycle", Some("c")),
        (Scope::View("v".to_owned()), "view", Some("v")),
        (Scope::Project("p".to_owned()), "project", Some("p")),
        (Scope::Label("l".to_owned()), "label", Some("l")),
    ];
    for (scope, kind, id) in all {
        assert_eq!(scope.kind(), kind);
        assert_eq!(scope.id(), id);
        assert_eq!(Scope::of(kind, id), Some(scope));
    }
    // A kind that is not one, an id where there is none, and no id where there must be one.
    assert_eq!(Scope::of("folder", Some("x")), None);
    assert_eq!(Scope::of("mine", Some("x")), None);
    assert_eq!(Scope::of("created", Some("x")), None);
    assert_eq!(Scope::of("view", None), None);
    assert_eq!(Scope::of("view", Some("")), None);
}

fn reply(body: &str) -> Reply {
    Reply {
        status: 200,
        body: body.to_owned(),
    }
}

fn empty(root: &str) -> Reply {
    reply(&format!(
        r#"{{"data":{{"{root}":{{"nodes":[],"pageInfo":{{"hasNextPage":false,"endCursor":null}}}}}}}}"#
    ))
}

#[test]
fn the_names_of_the_lists_come_from_four_questions_and_are_in_the_order_of_the_card() {
    let transport = Scripted::new(vec![
        // The favorites, out of their order, with a folder and a view of projects that are no lists,
        // a name with a line break in it and a kind of favorite that is not a list.
        Ok(reply(
            r#"{"data":{"favorites":{"nodes":[
                {"type":"project","sortOrder":3.0,"project":{"id":"p-bill","name":"Billing"}},
                {"type":"folder","sortOrder":0.5,"customView":null,"project":null,"cycle":null,"label":null},
                {"type":"customView","sortOrder":1.0,"customView":{"id":"v-bugs","name":"Bugs\nto triage","modelName":"Issue","team":null}},
                {"type":"customView","sortOrder":1.5,"customView":{"id":"v-proj","name":"All projects","modelName":"Project","team":null}},
                {"type":"customView","sortOrder":4.0,"customView":{"id":"v-ready","name":"Agent-ready","modelName":"Issue","team":{"key":"ACM"}}},
                {"type":"cycle","sortOrder":5.0,"cycle":{"id":"c-24","number":24.0,"name":null,"endsAt":"2026-11-15T00:00:00.000Z","team":{"key":"ACM"}}},
                {"type":"label","sortOrder":6.0,"label":{"id":"l-perf","name":"perf","team":null}},
                {"type":"predefinedView","sortOrder":7.0}
            ],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#,
        )),
        Ok(reply(
            r#"{"data":{"viewer":{"teamMemberships":{"nodes":[
                {"sortOrder":3.0,"team":{"id":"t-ops","key":"OPS","name":"Ops","activeCycle":null}},
                {"sortOrder":1.0,"team":{"id":"t-web","key":"WEB","name":"Web","activeCycle":{"id":"c-11","number":11.0,"name":"Spring","endsAt":"2026-11-18T00:00:00.000Z"}}},
                {"sortOrder":2.0,"team":{"id":"t-acm","key":"ACM","name":"Acme","activeCycle":{"id":"c-24","number":24.0,"name":null,"endsAt":"2026-11-15T00:00:00.000Z"}}}
            ],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#,
        )),
        Ok(reply(
            r#"{"data":{"customViews":{"nodes":[
                {"id":"v-stale","name":"Stale in review","modelName":"Issue","team":{"key":"ACM"}},
                {"id":"v-bugs","name":"Bugs to triage","modelName":"Issue","team":null},
                {"id":"v-docs","name":"Docs","modelName":"Project","team":null},
                {"id":"v-cust","name":"Bugs by customer","modelName":"Issue","team":null}
            ],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#,
        )),
        Ok(reply(
            r#"{"data":{"projects":{"nodes":[
                {"id":"p-bash","name":"Bug bash"},
                {"id":"p-bill","name":"Billing"}
            ],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#,
        )),
    ]);
    let entries = names(&transport, &key()).unwrap();
    let seen: Vec<(&str, &str, &str)> = entries
        .iter()
        .map(|entry| {
            (
                entry.scope.kind(),
                entry.section.name(),
                entry.title.as_str(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            // The favorites, by their sort order: the ones that are lists.
            ("view", "favorite", "Bugs to triage"),
            ("project", "favorite", "Billing"),
            ("view", "favorite", "Agent-ready"),
            ("cycle", "favorite", "ACM \u{b7} Cycle 24"),
            ("label", "favorite", "perf"),
            // The teams the person is a member of, in the order of their own list of teams (their
            // sort order, not the keys), each with its running cycle (a cycle that is a favorite is
            // not said twice, nor is a team without one) and its issues.
            ("cycle", "team", "WEB \u{b7} Current cycle"),
            ("team", "team", "WEB \u{b7} Issues"),
            ("team", "team", "ACM \u{b7} Issues"),
            ("team", "team", "OPS \u{b7} Issues"),
            // The views that are not favorites, by name, of issues only; then the projects.
            ("view", "view", "Bugs by customer"),
            ("view", "view", "Stale in review"),
            ("project", "project", "Bug bash"),
        ]
    );
    let sent = transport.sent();
    let named: Vec<String> = sent
        .iter()
        .map(|body| {
            serde_json::from_str::<serde_json::Value>(body).unwrap()["operationName"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        named,
        ["ListFavorites", "ListTeams", "ListViews", "ListProjects"]
    );
    for body in &sent {
        assert!(!body.to_lowercase().contains("mutation"), "{body}");
    }
    // The teams asked for are the person's memberships, the active ones.
    assert!(sent[1].contains("viewer { teamMemberships("), "{}", sent[1]);
    assert!(sent[1].contains("includeArchived: false"), "{}", sent[1]);
    // The projects asked for are the open ones.
    assert!(
        sent[3].contains(r#""completedAt":{"null":true}"#),
        "{}",
        sent[3]
    );
    // What a cycle is told with, and the team a list is of.
    let cycle = entries
        .iter()
        .find(|entry| entry.scope == Scope::Cycle("c-24".to_owned()))
        .unwrap();
    assert_eq!(cycle.cycle.as_ref().unwrap().number, 24);
    assert_eq!(
        cycle.cycle.as_ref().unwrap().ends_at,
        "2026-11-15T00:00:00.000Z"
    );
    assert_eq!(cycle.team.as_deref(), Some("ACM"));
    let ready = entries
        .iter()
        .find(|entry| entry.title == "Agent-ready")
        .unwrap();
    assert_eq!(ready.team.as_deref(), Some("ACM"));
    let spring = entries
        .iter()
        .find(|entry| entry.title == "WEB \u{b7} Current cycle")
        .unwrap();
    assert_eq!(spring.cycle.as_ref().unwrap().number, 11);
}

#[test]
fn the_names_are_taken_a_page_at_a_time_up_to_a_ceiling_and_a_failure_is_said() {
    let page = |from: usize, more: bool| {
        let nodes: Vec<String> = (from..from + 100)
            .map(|n| {
                format!(r#"{{"id":"v{n}","name":"View {n:04}","modelName":"Issue","team":null}}"#)
            })
            .collect();
        reply(&format!(
            r#"{{"data":{{"customViews":{{"nodes":[{}],"pageInfo":{{"hasNextPage":{more},"endCursor":"c{}"}}}}}}}}"#,
            nodes.join(","),
            from + 100
        ))
    };
    let transport = Scripted::new(vec![
        Ok(empty("favorites")),
        Ok(reply(
            r#"{"data":{"viewer":{"teamMemberships":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}}"#,
        )),
        Ok(page(0, true)),
        Ok(page(100, true)),
        Ok(empty("projects")),
    ]);
    let entries = names(&transport, &key()).unwrap();
    // Two hundred views are the most taken: the third page is not asked for.
    assert_eq!(entries.len(), names::NAMES_MOST);
    assert_eq!(transport.sent().len(), 5);
    assert!(transport.sent()[3].contains(r#""after":"c100""#));
    // A question that fails fails the names (the lists of issues do not wait for them).
    let transport = Scripted::new(vec![
        Ok(empty("favorites")),
        Err(Error::Unreachable("down".to_owned())),
    ]);
    assert!(matches!(
        names(&transport, &key()),
        Err(Error::Unreachable(_))
    ));
}

#[test]
fn the_choices_of_the_filters_leave_out_a_name_a_filter_would_refuse() {
    let long = "x".repeat(101);
    let body = format!(
        r##"{{"data":{{"workflowStates":{{"nodes":[
            {{"name":"Todo","type":"unstarted","position":1}},
            {{"name":"{long}","type":"started","position":2}}]}},
        "users":{{"nodes":[{{"id":"u1","displayName":"Ada","isMe":true}}]}},
        "issueLabels":{{"nodes":[{{"name":"bug","color":"#f00"}},{{"name":"{long}","color":"#0f0"}}]}}}}}}"##
    );
    let transport = Scripted::new(vec![Ok(Reply { status: 200, body })]);
    let options = filter_options(&transport, &key()).unwrap();
    assert_eq!(
        options
            .states
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Todo"]
    );
    assert_eq!(
        options
            .labels
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        ["bug"]
    );
}

/// `Issue` answered with the description `text` (or none).
fn detail_reply(n: usize, text: Option<&str>) -> Reply {
    let description = match text {
        Some(text) => serde_json::to_string(text).unwrap(),
        None => "null".to_owned(),
    };
    Reply {
        status: 200,
        body: format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            issue(n).replacen(
                r#""team":"#,
                &format!(r#""description":{description},"comments":{{"nodes":[],"pageInfo":{{"hasNextPage":false,"endCursor":null}}}},"team":"#),
                1
            )
        ),
    }
}

const LINE: &str = "![a.png](https://uploads.linear.app/stand-in/1)";

#[test]
fn pictures_go_to_the_end_of_the_description_as_linear_holds_it_now_and_nothing_else_is_written() {
    let transport = Scripted::new(vec![
        Ok(detail_reply(7, Some("The words\n\n- one\n- two\n"))),
        Ok(changed_reply(true, &[7])),
    ]);
    add_files(&transport, &key(), "ACM-7", &[LINE.to_owned()]).unwrap();
    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "the description is read, then written once");
    let read: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(read["operationName"], "Issue");
    let write: serde_json::Value = serde_json::from_str(&sent[1]).unwrap();
    assert_eq!(write["operationName"], "IssueBatchUpdate");
    // The issue by the id Linear gave, which is what the batch takes.
    assert_eq!(write["variables"]["ids"], serde_json::json!(["i7"]));
    let input = write["variables"]["input"].as_object().unwrap();
    assert_eq!(
        input.keys().collect::<Vec<_>>(),
        ["description"],
        "the description alone"
    );
    let description = input["description"].as_str().unwrap();
    // What was read is what is sent, to the letter, and the line after a blank one.
    assert!(
        description.starts_with("The words\n\n- one\n- two\n"),
        "{description}"
    );
    assert_eq!(description, format!("The words\n\n- one\n- two\n\n{LINE}"));
}

#[test]
fn a_description_there_is_not_is_the_pictures_alone() {
    let transport = Scripted::new(vec![
        Ok(detail_reply(7, None)),
        Ok(changed_reply(true, &[7])),
    ]);
    add_files(&transport, &key(), "i7", &[LINE.to_owned()]).unwrap();
    let write: serde_json::Value = serde_json::from_str(&transport.sent()[1]).unwrap();
    assert_eq!(write["variables"]["input"]["description"], LINE);
}

#[test]
fn a_description_that_cannot_be_read_or_would_be_too_long_is_not_written() {
    let transport = Scripted::new(vec![Err(Error::Unreachable("down".to_owned()))]);
    assert!(matches!(
        add_files(&transport, &key(), "i7", &[LINE.to_owned()]),
        Err(Step::Read(Error::Unreachable(_)))
    ));
    assert_eq!(transport.sent().len(), 1, "only the reading");
    let long = "x".repeat(images::DESCRIPTION_MOST);
    let transport = Scripted::new(vec![Ok(detail_reply(7, Some(&long)))]);
    assert!(matches!(
        add_files(&transport, &key(), "i7", &[LINE.to_owned()]),
        Err(Step::TooLong)
    ));
    assert_eq!(transport.sent().len(), 1);
    // No picture, no write at all.
    let transport = Scripted::new(vec![]);
    assert!(add_files(&transport, &key(), "i7", &[]).is_err());
    assert!(transport.sent().is_empty());
}

#[test]
fn a_description_that_was_sent_and_not_answered_is_a_failure_of_the_write() {
    let transport = Scripted::new(vec![
        Ok(detail_reply(7, Some("Words"))),
        Err(Error::Unreachable("timed out".to_owned())),
    ]);
    assert!(matches!(
        add_files(&transport, &key(), "i7", &[LINE.to_owned()]),
        Err(Step::Write(Error::Unreachable(_)))
    ));
    assert_eq!(transport.sent().len(), 2, "the write is not sent again");
}

/// A copy of `bytes` as the program keeps what it sends.
fn staged(bytes: &[u8]) -> super::staged::Staged {
    let staged = super::staged::Staged::new().unwrap();
    staged.copy_in(&mut &bytes[..], u64::MAX).unwrap();
    staged
}

#[test]
fn the_place_for_a_file_is_asked_with_what_the_file_is_and_never_with_its_name_in_the_text() {
    let transport = Scripted::new(vec![Ok(Reply {
        status: 200,
        body: r#"{"data":{"fileUpload":{"success":true,"uploadFile":{"uploadUrl":"https://storage.stand-in/x?sig=SECRET","assetUrl":"https://evil.example/a.png","headers":[]}}}}"#.to_owned(),
    })]);
    let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x02\x08\x06\0\0\0";
    let copy = staged(png);
    // An asset address that is not one of Linear's files is not taken, and the storage is not sent to.
    let failed = upload(
        &transport,
        &key(),
        "secret name.png",
        "image/png",
        copy.path(),
        png.len() as u64,
    );
    assert!(matches!(failed, Err(Error::Failed(_))), "{failed:?}");
    let sent = transport.sent();
    let body: serde_json::Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(body["operationName"], "FileUpload");
    assert_eq!(body["variables"]["contentType"], "image/png");
    assert_eq!(body["variables"]["filename"], "secret name.png");
    assert_eq!(body["variables"]["size"], png.len());
    assert!(!body["query"].as_str().unwrap().contains("secret name"));
    // Nothing, or more than 100 MiB, asks nothing.
    let transport = Scripted::new(vec![]);
    assert!(upload(&transport, &key(), "a.png", "image/png", copy.path(), 0).is_err());
    assert!(
        upload(
            &transport,
            &key(),
            "a.bin",
            "application/octet-stream",
            copy.path(),
            images::FILE_MOST + 1
        )
        .is_err()
    );
    assert!(transport.sent().is_empty());
}

#[test]
fn a_file_of_any_kind_is_sent_up_to_a_hundred_mib_and_nothing_else_is_by_whoever_asks() {
    let transport = Scripted::new(vec![]);
    assert_eq!(
        send_file(&transport, &key(), "a.png", staged(b"").path()).unwrap_err(),
        NotSent::Empty
    );
    assert_eq!(
        send_file(
            &transport,
            &key(),
            "a.png",
            std::path::Path::new("/nowhere/at/all")
        )
        .unwrap_err(),
        NotSent::Unreadable
    );
    assert!(transport.sent().is_empty(), "Linear is asked of nothing");
    // A file that is not a picture asks for the place with what it is: a PDF, or nothing it knows.
    for (bytes, content_type) in [
        (&b"%PDF-1.7\n..."[..], "application/pdf"),
        (&b"PK\x03\x04zip"[..], "application/octet-stream"),
        (&b"GIF89a...."[..], "image/gif"),
    ] {
        let transport = Scripted::new(vec![]);
        let _ = send_file(&transport, &key(), "x", staged(bytes).path());
        let body: serde_json::Value = serde_json::from_str(&transport.sent()[0]).unwrap();
        assert_eq!(body["variables"]["contentType"], content_type);
        assert_eq!(body["variables"]["size"], bytes.len());
    }
}

/// `Comment` answered: the comment `id` with `body`, the person's or not, on the issue `i7` (ACM-7).
fn comment_reply(id: &str, body: &str, mine: bool) -> Reply {
    Reply {
        status: 200,
        body: serde_json::json!({"data": {"comment": {
            "id": id, "body": body, "createdAt": "2026-11-10T10:00:00.000Z", "editedAt": null,
            "user": {"displayName": "Ada", "isMe": mine},
            "issue": {"id": "i7", "identifier": "ACM-7"}
        }}})
        .to_string(),
    }
}

fn comment_updated_reply(success: bool) -> Reply {
    Reply {
        status: 200,
        body: serde_json::json!({"data": {"commentUpdate": {"success": success, "comment": {
            "id": "c-1", "body": "New words", "createdAt": "2026-11-10T10:00:00.000Z",
            "editedAt": "2026-11-11T10:00:00.000Z", "user": {"displayName": "Ada", "isMe": true}
        }}}})
        .to_string(),
    }
}

fn operation(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap()
}

#[test]
fn an_edited_description_is_written_whole_and_alone_where_linear_holds_what_the_edit_began_from() {
    use super::edit::{Target, save};
    let transport = Scripted::new(vec![
        Ok(detail_reply(7, Some("Old words\n"))),
        Ok(changed_reply(true, &[7])),
    ]);
    save(
        &transport,
        &key(),
        "ACM-7",
        &Target::Description,
        "Old words\n",
        "New words",
    )
    .unwrap();
    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "read again, then written once");
    assert_eq!(operation(&sent[0])["operationName"], "Issue");
    let write = operation(&sent[1]);
    assert_eq!(write["operationName"], "IssueBatchUpdate");
    assert_eq!(write["variables"]["ids"], serde_json::json!(["i7"]));
    let input = write["variables"]["input"].as_object().unwrap();
    assert_eq!(input.keys().collect::<Vec<_>>(), ["description"]);
    assert_eq!(input["description"], "New words");
    // Emptied, it is an empty description.
    let transport = Scripted::new(vec![
        Ok(detail_reply(7, Some("Old"))),
        Ok(changed_reply(true, &[7])),
    ]);
    save(&transport, &key(), "i7", &Target::Description, "Old", "").unwrap();
    assert_eq!(
        operation(&transport.sent()[1])["variables"]["input"]["description"],
        ""
    );
}

#[test]
fn an_edit_of_what_someone_changed_meanwhile_is_not_written_and_says_what_linear_holds() {
    use super::edit::{Step, Target, save};
    let transport = Scripted::new(vec![Ok(detail_reply(7, Some("Their words")))]);
    assert_eq!(
        save(
            &transport,
            &key(),
            "i7",
            &Target::Description,
            "Old words",
            "Mine"
        ),
        Err(Step::Changed("Their words".to_owned()))
    );
    assert_eq!(transport.sent().len(), 1, "only the reading");
    // A comment the same.
    let transport = Scripted::new(vec![Ok(comment_reply("c-1", "Their words", true))]);
    assert_eq!(
        save(
            &transport,
            &key(),
            "i7",
            &Target::Comment("c-1".into()),
            "Old",
            "Mine"
        ),
        Err(Step::Changed("Their words".to_owned()))
    );
    assert_eq!(transport.sent().len(), 1);
}

#[test]
fn an_edit_linear_holds_already_is_not_sent_again() {
    use super::edit::{Target, save};
    // The answer to a write before did not come, and it had been made: Linear holds the words (with no
    // blank at their end).
    let transport = Scripted::new(vec![Ok(detail_reply(7, Some("New words")))]);
    save(
        &transport,
        &key(),
        "i7",
        &Target::Description,
        "Old",
        "New words\n",
    )
    .unwrap();
    assert_eq!(transport.sent().len(), 1, "nothing is written");
}

#[test]
fn an_edited_comment_is_its_body_alone_to_that_comment_and_only_the_persons_own() {
    use super::edit::{NOT_HERE, NOT_YOURS, Step, Target, save};
    let target = Target::Comment("c-1".into());
    let transport = Scripted::new(vec![
        Ok(comment_reply("c-1", "Old words", true)),
        Ok(comment_updated_reply(true)),
    ]);
    save(
        &transport,
        &key(),
        "ACM-7",
        &target,
        "Old words",
        "New words",
    )
    .unwrap();
    let sent = transport.sent();
    assert_eq!(sent.len(), 2);
    let read = operation(&sent[0]);
    assert_eq!(read["operationName"], "Comment");
    assert_eq!(read["variables"]["id"], "c-1");
    let write = operation(&sent[1]);
    assert_eq!(write["operationName"], "CommentUpdate");
    assert_eq!(write["variables"]["id"], "c-1");
    assert_eq!(
        write["variables"]["input"],
        serde_json::json!({"body": "New words"})
    );
    // Someone else's comment is not edited: nothing is sent after the reading.
    let transport = Scripted::new(vec![Ok(comment_reply("c-1", "Old words", false))]);
    assert_eq!(
        save(&transport, &key(), "i7", &target, "Old words", "Mine"),
        Err(Step::Read(Error::Failed(NOT_YOURS.to_owned())))
    );
    assert_eq!(transport.sent().len(), 1);
    // Nor a comment of another issue than the one named.
    let transport = Scripted::new(vec![Ok(comment_reply("c-1", "Old words", true))]);
    assert_eq!(
        save(&transport, &key(), "ACM-8", &target, "Old words", "Mine"),
        Err(Step::Read(Error::Failed(NOT_HERE.to_owned())))
    );
    assert_eq!(transport.sent().len(), 1);
    // Linear saying it did not is a no, and leaves nothing open.
    let transport = Scripted::new(vec![
        Ok(comment_reply("c-1", "Old words", true)),
        Ok(comment_updated_reply(false)),
    ]);
    let Err(Step::Write(Error::Failed(why))) =
        save(&transport, &key(), "i7", &target, "Old words", "Mine")
    else {
        panic!("not a failure of the write");
    };
    assert!(!left_open(&why));
}

#[test]
fn an_edit_that_was_sent_and_not_answered_is_not_sent_again() {
    use super::edit::{Step, Target, save};
    let transport = Scripted::new(vec![
        Ok(comment_reply("c-1", "Old", true)),
        Err(Error::Unreachable("timed out".to_owned())),
    ]);
    assert!(matches!(
        save(
            &transport,
            &key(),
            "i7",
            &Target::Comment("c-1".into()),
            "Old",
            "New"
        ),
        Err(Step::Write(Error::Unreachable(_)))
    ));
    assert_eq!(transport.sent().len(), 2);
    // What cannot be read again is not written.
    let transport = Scripted::new(vec![Err(Error::Unreachable("down".to_owned()))]);
    assert!(matches!(
        save(&transport, &key(), "i7", &Target::Description, "Old", "New"),
        Err(Step::Read(Error::Unreachable(_)))
    ));
    assert_eq!(transport.sent().len(), 1);
}
