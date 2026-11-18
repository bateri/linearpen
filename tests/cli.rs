//! The commands of the command line as processes, in the environment of `common`: Linear is the
//! stand-in `curl`, the key is a fake one, and nothing reaches the machine or the network.

// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Home, KEY, Ran, run};

/// `linearpen` with `args`, a key in its environment and `input` on stdin.
fn linearpen(home: &Home, args: &[&str], env: &[(&str, &str)], input: Option<&str>) -> Ran {
    let mut command = home.linearpen();
    command.args(args).env("LINEAR_API_KEY", KEY);
    for (name, value) in env {
        command.env(name, value);
    }
    run(command, input, Duration::from_secs(60))
}

/// The bodies the stand-in `curl` was sent for the operation `name`.
fn sent(home: &Home, name: &str) -> Vec<serde_json::Value> {
    home.calls()
        .iter()
        .filter(|call| {
            call.stdin
                .contains(&format!("\\\"operationName\\\":\\\"{name}\\\""))
        })
        .map(|call| {
            // The body is the quoted line `data-raw = "..."` of the configuration.
            let line = call
                .stdin
                .lines()
                .find_map(|line| line.strip_prefix("data-raw = \""))
                .unwrap()
                .strip_suffix('"')
                .unwrap();
            let body = line.replace("\\\"", "\"").replace("\\\\", "\\");
            serde_json::from_str(&body).unwrap()
        })
        .collect()
}

fn writes(home: &Home) -> usize {
    home.calls()
        .iter()
        .filter(|call| call.stdin.contains("mutation "))
        .count()
}

#[test]
fn list_prints_my_issues_in_groups_and_as_json_with_the_key_nowhere() {
    let home = Home::new();
    let ran = linearpen(&home, &["list"], &[], None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.starts_with("My issues (14)\n"), "{}", ran.stdout);
    assert!(ran.stdout.contains("  In Progress (4)\n"), "{}", ran.stdout);
    assert!(ran.stdout.contains("ACM-412  High"), "{}", ran.stdout);
    assert!(!ran.stdout.contains("All issues"));
    assert_eq!(sent(&home, "MyIssues").len(), 1);
    assert_eq!(writes(&home), 0);

    let json = linearpen(&home, &["list", "--json"], &[], None);
    assert_eq!(json.status, Some(0), "{}", json.stderr);
    let listed: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(listed["list"], "My issues");
    assert_eq!(listed["truncated"], false);
    assert_eq!(listed["issues"].as_array().unwrap().len(), 14);
    let first = &listed["issues"][0];
    for field in [
        "id",
        "identifier",
        "title",
        "url",
        "state",
        "priority",
        "labels",
        "updated_at",
    ] {
        assert!(first.get(field).is_some(), "{field}: {first}");
    }
    for call in home.calls() {
        assert_eq!(call.argv, ["--config", "-"]);
        assert_eq!(call.env_key, "unset");
    }
    assert!(!ran.stdout.contains(KEY) && !json.stdout.contains(KEY) && !json.stderr.contains(KEY));
}

#[test]
fn update_with_two_fields_sends_one_batch_update_with_both_and_prints_the_issue_as_it_is_now() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &[
            "update",
            "ACM-412",
            "--status",
            "done",
            "--priority",
            "low",
            "--label",
            "bug",
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let batches = sent(&home, "IssueBatchUpdate");
    assert_eq!(batches.len(), 1);
    assert_eq!(writes(&home), 1);
    let variables = &batches[0]["variables"];
    assert_eq!(variables["ids"], serde_json::json!(["issue-412"]));
    assert_eq!(
        variables["input"],
        serde_json::json!({"stateId": "s-done", "priority": 4, "addedLabelIds": ["l-bug"]})
    );
    assert!(ran.stdout.starts_with("ACM-412  Done"), "{}", ran.stdout);
    assert_eq!(ran.stdout.lines().count(), 1);
}

#[test]
fn comment_takes_its_words_from_stdin_and_posts_once() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--body", "-"],
        &[],
        Some("Looks right\nto me\n"),
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "ACM-412: comment added\n");
    let posted = sent(&home, "CommentCreate");
    assert_eq!(posted.len(), 1);
    assert_eq!(
        posted[0]["variables"]["input"]["body"],
        "Looks right\nto me"
    );
    // The issue's own id, which the issue it was named by is asked for.
    assert_eq!(posted[0]["variables"]["input"]["issueId"], "issue-412");
    assert_eq!(writes(&home), 1);
}

#[test]
fn a_name_linear_has_not_ends_with_one_and_nothing_is_written() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["update", "ACM-412", "--status", "Doing"],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(1), "{}", ran.stderr);
    assert_eq!(
        ran.stderr,
        "linearpen: ACM has no status \"Doing\"; it has Backlog, Todo, In Progress, In Review, Done, Canceled\n"
    );
    assert!(ran.stdout.is_empty());
    assert_eq!(writes(&home), 0);
}

#[test]
fn a_write_whose_answer_never_came_ends_with_three_and_is_sent_once() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["update", "ACM-412", "--priority", "low"],
        &[("STANDIN_CHANGE", "lost")],
        None,
    );
    assert_eq!(ran.status, Some(3), "{}", ran.stderr);
    assert!(
        ran.stderr.contains(
            "it is not known whether Linear made it; look in Linear before running it again"
        ),
        "{}",
        ran.stderr
    );
    assert_eq!(sent(&home, "IssueBatchUpdate").len(), 1);

    let home = Home::new();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--body", "hi"],
        &[("STANDIN_COMMENT", "lost")],
        None,
    );
    assert_eq!(ran.status, Some(3), "{}", ran.stderr);
    assert_eq!(sent(&home, "CommentCreate").len(), 1);
}

#[test]
fn create_prints_the_identifier_and_the_link_and_a_lost_answer_is_found_by_the_draft_id() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["create", "--team", "acme", "--title", "Fix the login"],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(
        ran.stdout,
        "ACM-900  https://linear.app/acme/issue/ACM-900\n"
    );
    assert_eq!(sent(&home, "IssueCreate").len(), 1);

    let home = Home::new();
    let ran = linearpen(
        &home,
        &["create", "--team", "ACM", "--title", "Fix the login"],
        &[("STANDIN_CREATE", "lost")],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.starts_with("ACM-900  "), "{}", ran.stdout);
    assert_eq!(sent(&home, "IssueCreate").len(), 1);
    assert_eq!(sent(&home, "Issue").len(), 1);
}

#[test]
fn what_is_not_understood_ends_with_two_and_help_and_teams_work() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["update", "ACM-412", "--description", "x"],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2));
    assert!(
        ran.stderr
            .contains("never changes an issue's description or team"),
        "{}",
        ran.stderr
    );
    assert!(home.calls().is_empty());
    let help = linearpen(&home, &["help", "update"], &[], None);
    assert_eq!(help.status, Some(0));
    assert!(help.stdout.contains("linearpen update"));
    let teams = linearpen(&home, &["teams"], &[], None);
    assert_eq!(teams.stdout, "ACM  Acme\nWEB  web\n");
    // No key: 2, and nothing is asked.
    let home = Home::new();
    let mut command = home.linearpen();
    command.arg("teams");
    let ran = run(command, None, Duration::from_secs(60));
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(home.calls().is_empty());
}

const AGENT: &str = "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f10";

#[test]
fn in_an_agents_pane_a_write_it_may_not_make_stops_before_curl() {
    let home = Home::new();
    let made = linearpen(
        &home,
        &["create", "--team", "ACM", "--title", "x"],
        &[("LINEARPEN_AGENT", AGENT)],
        None,
    );
    assert_eq!(made.status, Some(1), "{}", made.stderr);
    assert!(
        made.stderr.contains("it makes no issues"),
        "{}",
        made.stderr
    );
    assert!(made.stdout.is_empty(), "{}", made.stdout);
    let more = linearpen(
        &home,
        &["update", "ACM-412", "--status", "Done", "--priority", "low"],
        &[("LINEARPEN_AGENT", AGENT)],
        None,
    );
    assert_eq!(more.status, Some(1), "{}", more.stderr);
    assert!(
        more.stderr.contains("it changes only the status"),
        "{}",
        more.stderr
    );
    // A variable that is set and holds no id is an agent's run that may write nothing.
    for value in ["", "ACM-412", "not-an-id"] {
        let update = linearpen(
            &home,
            &["update", "ACM-412", "--status", "Done"],
            &[("LINEARPEN_AGENT", value)],
            None,
        );
        assert_eq!(update.status, Some(1), "{value:?}: {}", update.stderr);
        assert!(
            update.stderr.contains("it writes nothing"),
            "{}",
            update.stderr
        );
        let comment = linearpen(
            &home,
            &["comment", "ACM-412", "--body", "-"],
            &[("LINEARPEN_AGENT", value)],
            Some("words"),
        );
        assert_eq!(comment.status, Some(1), "{value:?}: {}", comment.stderr);
    }
    // Not one of them reached `curl`.
    assert!(home.calls().is_empty(), "{:?}", home.calls().len());
}

#[test]
fn in_an_agents_pane_another_issue_is_read_and_then_refused_with_no_write_and_the_rest_is_read() {
    let home = Home::new();
    // The issue the stand-in has is `issue-412`, not the agent's.
    for args in [
        &["update", "ACM-412", "--status", "done"][..],
        &["comment", "ACM-412", "--body", "x"],
    ] {
        let ran = linearpen(&home, args, &[("LINEARPEN_AGENT", AGENT)], None);
        assert_eq!(ran.status, Some(1), "{args:?}: {}", ran.stderr);
        assert!(
            ran.stderr.contains("on another issue"),
            "{args:?}: {}",
            ran.stderr
        );
    }
    assert_eq!(writes(&home), 0);
    assert!(!sent(&home, "Issue").is_empty());

    let listed = linearpen(&home, &["list"], &[("LINEARPEN_AGENT", AGENT)], None);
    assert_eq!(listed.status, Some(0), "{}", listed.stderr);
    assert!(listed.stdout.starts_with("My issues"), "{}", listed.stdout);
    let shown = linearpen(
        &home,
        &["show", "ACM-412"],
        &[("LINEARPEN_AGENT", "")],
        None,
    );
    assert_eq!(shown.status, Some(0), "{}", shown.stderr);
    assert_eq!(writes(&home), 0);
}
