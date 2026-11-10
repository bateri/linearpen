//! linearpen in a terminal pane, end to end: the view is opened, the page's calls and topic reach the
//! program through the tty, Linear is the stand-in `curl`, the ways out (⌃C, the view closed) end it,
//! and a terminal that shows no views is given the text. See `common` for how the stand-in terminal
//! is found and why the environment is the one it is.

// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Home, KEY, Script, answers, host, program, pushes, run};
use serde_json::json;

/// The script of a run that has a key in its environment, as one that is started from a shell.
fn keyed() -> Script {
    Script::new().env("LINEAR_API_KEY", KEY)
}

#[test]
fn the_view_is_opened_and_the_page_boots() {
    let home = Home::new();
    let log = home.record(&keyed().call("boot", json!({})));
    assert!(
        log.lines().any(
            |line| line.starts_with("program: open id=") && line.contains(" title=\"Linear\"")
        ),
        "{log}"
    );
    assert!(
        log.contains("view: title=\"Linear\" program=\"linearpen\""),
        "{log}"
    );
    assert!(log.contains("view: icons png 32x32 dark"), "{log}");
    let heard = answers(&log);
    assert_eq!(heard.len(), 1, "{log}");
    let boot = heard[0].result();
    assert_eq!(boot["version"], "0.1.0");
    assert_eq!(boot["key"], "environment");
    assert!(boot["problem"].is_null());
}

#[test]
fn without_a_key_the_boot_says_so_and_the_lists_are_not_asked_for() {
    let home = Home::new();
    let heard = home.ask(&Script::new().call("boot", json!({})));
    assert_eq!(heard[0].result()["key"], "none");
    assert!(home.calls().is_empty(), "nothing is asked without a key");
}

#[test]
fn the_issues_topic_tells_the_lists_that_curl_brought_and_the_key_never_leaves_its_stdin() {
    let home = Home::new();
    let log = home.record(
        &keyed()
            // However many times the topic tells (the lists come in the second or the third), the
            // stand-in listens until its time is up.
            .host_args(&["--subscribe", "issues", "--cancel-after", "99"])
            .then(&["--then", "wait"])
            .limit(5),
    );
    let told = pushes(&log);
    let loaded = told
        .iter()
        .find(|list| list["loaded"] == true)
        .unwrap_or_else(|| panic!("a list that was loaded: {told:?}\n{log}"));
    assert_eq!(loaded["counts"]["mine"], 14);
    assert_eq!(loaded["counts"]["all"], 19);
    assert_eq!(loaded["scope"], "mine");
    assert_eq!(loaded["groups"][0]["name"], "In Progress");
    assert_eq!(loaded["groups"][0]["rows"][0]["identifier"], "ACM-412");
    // The backlog begins folded: its count, no rows.
    let backlog = loaded["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["name"] == "Backlog")
        .expect("a backlog");
    assert_eq!(backlog["closed"], true);
    assert_eq!(backlog["count"], 2);
    assert!(backlog["rows"].is_null() || backlog["rows"].as_array().unwrap().is_empty());
    let calls = home.calls();
    assert!(calls.len() >= 2, "{calls:?}");
    for call in &calls {
        assert_eq!(call.argv, ["--config", "-"], "the key is not an argument");
        assert_eq!(
            call.env_key, "unset",
            "the key is not in curl's environment"
        );
        assert!(call.stdin.contains(KEY), "{}", call.stdin);
        assert!(!call.stdin.contains("mutation"), "{}", call.stdin);
    }
    assert!(!log.contains(KEY), "the key is not in what the page heard");
}

#[test]
fn the_view_call_takes_a_scope_a_filter_and_the_groups_folded_and_refuses_the_rest() {
    let home = Home::new();
    let long = "x".repeat(201);
    let heard = home.ask(
        &keyed()
            .call(
                "view",
                json!({ "scope": "all", "filter": "bug", "closed": { "started:In Progress": true } }),
            )
            .call("view", json!({ "scope": "everyone" }))
            .call("view", json!({ "filter": long }))
            .call("view", json!({ "speed": 1 }))
            .call("view", json!({})),
    );
    let view = heard[0].result();
    assert_eq!(view["scope"], "all");
    assert_eq!(view["filter"], "bug");
    assert!(heard[1].failed("bad-params"), "{:?}", heard[1]);
    assert!(heard[2].failed("bad-params"), "{:?}", heard[2]);
    assert!(heard[3].failed("bad-params"), "{:?}", heard[3]);
    // What was refused changed nothing.
    assert_eq!(heard[4].result()["scope"], "all");
    assert_eq!(heard[4].result()["filter"], "bug");
}

#[test]
fn connect_keeps_the_key_linear_takes_with_a_closed_file_and_refuses_what_is_not_one() {
    let home = Home::new();
    let heard = home.ask(
        &Script::new()
            .call("connect", json!({ "key": "has a space" }))
            .call("connect", json!({ "key": "" }))
            .call("connect", json!({ "key": KEY })),
    );
    assert!(heard[0].failed("bad-params"), "{:?}", heard[0]);
    assert!(heard[1].failed("bad-params"), "{:?}", heard[1]);
    assert_eq!(heard[2].result()["organization"], "Acme");
    assert_eq!(heard[2].result()["saved"], true);
    let file = home.config_dir().join("key");
    assert_eq!(
        std::fs::read_to_string(&file).expect("the key file").trim(),
        KEY
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode =
            |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&home.config_dir()), 0o700);
    }
    // The next run finds it.
    let heard = home.ask(&Script::new().call("boot", json!({})));
    assert_eq!(heard[0].result()["key"], "file");
}

#[test]
fn a_key_linear_refuses_is_said_and_not_kept() {
    let home = Home::new();
    let heard = home.ask(
        &Script::new()
            .env("STANDIN_MODE", "refused")
            .call("connect", json!({ "key": KEY })),
    );
    assert!(heard[0].failed("refused"), "{:?}", heard[0]);
    assert!(!heard[0].message().contains(KEY));
    assert!(!home.config_dir().join("key").exists());
}

#[test]
fn linear_that_is_not_reached_is_unreachable_and_refresh_needs_a_key() {
    let home = Home::new();
    let heard = home.ask(
        &Script::new()
            .env("STANDIN_MODE", "down")
            .call("refresh", json!({}))
            .call("connect", json!({ "key": KEY })),
    );
    assert!(heard[0].failed("no-key"), "{:?}", heard[0]);
    assert!(heard[1].failed("unreachable"), "{:?}", heard[1]);
}

#[test]
fn a_call_the_program_does_not_know_is_not_found() {
    let home = Home::new();
    let heard = home.ask(&Script::new().call("mutate", json!({})));
    assert!(heard[0].failed("not-found"), "{:?}", heard[0]);
}

#[test]
fn interrupt_ends_linearpen_as_a_shell_says() {
    let home = Home::new();
    let log = home.record(&keyed().call("boot", json!({})));
    assert!(log.contains("program: exited with status 130"), "{log}");
}

#[test]
fn closing_the_view_ends_linearpen_with_success() {
    let home = Home::new();
    let log = home.record(&keyed().then(&["--then", "close"]));
    assert!(log.contains("reason=user"), "{log}");
    assert!(log.contains("program: exited with status 0"), "{log}");
}

#[test]
fn a_terminal_without_pencere_is_given_the_text_and_the_words_that_say_why() {
    let home = Home::new();
    let mut command = home.command(host());
    command
        .env("LINEAR_API_KEY", KEY)
        .args(["--no-host", "--passthrough", "--limit", "15", "--"])
        .arg(program());
    let ran = run(command, None, Duration::from_secs(30));
    assert!(ran.stdout.contains("In Progress"), "{}", ran.stdout);
    assert!(ran.stdout.contains("ACM-412"), "{}", ran.stdout);
    assert!(
        ran.stdout.contains("This terminal shows no views"),
        "{}",
        ran.stdout
    );
    assert!(
        ran.stderr.contains("exited with status 0"),
        "{}",
        ran.stderr
    );
}
