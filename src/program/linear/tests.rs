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
        "labels":{{"nodes":[]}},"team":{{"key":"ACM"}},"cycle":null,"project":null,"assignee":null}}"#
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
