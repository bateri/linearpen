//! linearpen as a program with no view: the issues printed as text, through `curl` that is a
//! stand-in (`test/stand-in/`), in the environment of `common`. The key is looked at everywhere it
//! could be seen.

// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use common::{Home, KEY, stand_in_bin};

#[test]
fn the_environment_of_the_tests_touches_nothing_of_the_machine() {
    let home = Home::new();
    let mut env = home.command("/usr/bin/env");
    env.env("BATERI_TAB_URL", "must not survive")
        .env_remove("BATERI_TAB_URL");
    let said = home.run(env).stdout;
    let var = |name: &str| {
        said.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .map(str::to_owned)
    };
    let tmp = home.dir.path().to_str().unwrap().to_owned();
    assert!(
        var("PATH")
            .unwrap()
            .starts_with(stand_in_bin().to_str().unwrap())
    );
    for name in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_STATE_HOME",
        "LINEARPEN_CONFIG_DIR",
        "KABLO_DIR",
        "TMPDIR",
    ] {
        assert!(
            var(name).unwrap().starts_with(&tmp),
            "{name} is under the test's directory"
        );
    }
    assert!(
        var("LINEARPEN_API_URL")
            .unwrap()
            .starts_with("http://127.0.0.1:")
    );
    assert_eq!(var("BATERI_TAB_URL"), None);
    assert_eq!(var("LINEAR_API_KEY"), None);
    // `curl` is the stand-in.
    let which = home.run({
        let mut which = home.command("/bin/sh");
        which.args(["-c", "command -v curl"]);
        which
    });
    assert_eq!(
        which.stdout.trim(),
        stand_in_bin().join("curl").to_str().unwrap()
    );
}

#[test]
fn the_issues_are_printed_in_their_groups_and_the_run_ends_well() {
    let home = Home::new();
    let ran = home.run({
        let mut command = home.linearpen();
        command.arg("--text").env("LINEAR_API_KEY", KEY);
        command
    });
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let out = ran.stdout;
    assert!(out.starts_with("Acme - Ada\n"), "{out}");
    assert!(out.contains("My issues (14)"), "{out}");
    assert!(out.contains("All issues (19)"), "{out}");
    // In the order of the groups: what is started, what is not, then the backlog.
    let at = |text: &str| {
        out.find(text)
            .unwrap_or_else(|| panic!("{text} is not in:\n{out}"))
    };
    assert!(at("  In Progress (4)") < at("  In Review (2)"));
    assert!(at("  In Review (2)") < at("  Todo (6)"));
    assert!(at("  Todo (6)") < at("  Backlog (2)"));
    assert!(out.contains("ACM-412  High"), "{out}");
    assert!(
        out.contains("Add rate limiting to the public API  [api]"),
        "{out}"
    );
    assert!(!out.contains(KEY) && !ran.stderr.contains(KEY));
}

#[test]
fn the_list_of_the_person_can_be_empty_and_says_so() {
    let home = Home::new();
    let ran = home.run({
        let mut command = home.linearpen();
        command
            .arg("--text")
            .env("LINEAR_API_KEY", KEY)
            .env("STANDIN_MY", "empty");
        command
    });
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(
        ran.stdout
            .contains("My issues (0)\n  Nothing is assigned to you.\n"),
        "{}",
        ran.stdout
    );
    assert!(ran.stdout.contains("All issues (19)"));
}

#[test]
fn the_key_goes_to_curl_on_its_stdin_and_nowhere_else() {
    let home = Home::new();
    let ran = home.run({
        let mut command = home.linearpen();
        command.arg("--text").env("LINEAR_API_KEY", KEY);
        command
    });
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let calls = home.calls();
    // One question for each list: its open issues.
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].stdin.contains("operationName\\\":\\\"MyIssues"));
    assert!(calls[1].stdin.contains("operationName\\\":\\\"AllIssues"));
    assert!(
        ran.stdout
            .contains("Done and canceled issues are not listed: `linearpen list --status Done`"),
        "{}",
        ran.stdout
    );
    for call in &calls {
        // Not in what `ps` shows.
        assert_eq!(call.argv, ["-q", "--config", "-"]);
        // Not in the environment of curl, though the program's own had it.
        assert_eq!(call.env_key, "unset");
        // In the configuration curl reads from stdin, as the header, and nowhere else.
        assert!(
            call.stdin
                .contains(&format!("header = \"Authorization: {KEY}\"\n")),
            "{}",
            call.stdin
        );
        assert_eq!(call.stdin.matches(KEY).count(), 1);
        // The address is the one the tests gave, never Linear's.
        assert!(call.stdin.contains("url = \"http://127.0.0.1:9/graphql\""));
        assert!(!call.stdin.contains("api.linear.app"));
        // Only questions are sent.
        assert!(!call.stdin.to_lowercase().contains("mutation"));
    }
}

#[test]
fn a_key_in_the_file_is_used_and_a_loose_file_is_told_and_left_as_it_is() {
    let home = Home::new();
    fs::create_dir_all(home.config_dir()).unwrap();
    let file = home.config_dir().join("key");
    fs::write(&file, format!("{KEY}\n")).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    let ran = home.run({
        let mut command = home.linearpen();
        command.arg("--text");
        command
    });
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.contains("All issues (19)"));
    assert_eq!(ran.stderr.lines().count(), 1, "{}", ran.stderr);
    assert!(
        ran.stderr.contains("can be read by other users"),
        "{}",
        ran.stderr
    );
    assert!(!ran.stderr.contains(KEY));
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o644
    );
    assert!(home.calls()[0].stdin.contains(KEY));
}

#[test]
fn with_no_key_it_says_where_to_give_one_and_ends_badly_without_asking_linear() {
    let home = Home::new();
    let ran = home.run({
        let mut command = home.linearpen();
        command.arg("--text");
        command
    });
    assert_eq!(ran.status, Some(2));
    assert!(ran.stdout.is_empty());
    assert!(ran.stderr.contains("LINEAR_API_KEY"), "{}", ran.stderr);
    assert!(
        ran.stderr
            .contains(home.config_dir().join("key").to_str().unwrap()),
        "{}",
        ran.stderr
    );
    assert!(home.calls().is_empty());
}

#[test]
fn each_way_linear_can_fail_is_told_in_its_own_words_and_the_key_stays_unsaid() {
    for (mode, status, words) in [
        ("refused", 1, "did not accept the key from LINEAR_API_KEY"),
        (
            "down",
            1,
            "could not be reached: the host name was not found",
        ),
        ("errors", 1, "Linear said: Cannot query field"),
        ("broken", 1, "not what was asked for"),
    ] {
        let home = Home::new();
        let ran = home.run({
            let mut command = home.linearpen();
            command
                .arg("--text")
                .env("LINEAR_API_KEY", KEY)
                .env("STANDIN_MODE", mode);
            command
        });
        assert_eq!(ran.status, Some(status), "{mode}: {}", ran.stderr);
        assert!(ran.stdout.is_empty(), "{mode}");
        assert!(ran.stderr.contains(words), "{mode}: {}", ran.stderr);
        assert!(!ran.stderr.contains(KEY), "{mode}");
    }
}

#[test]
fn without_a_view_and_without_a_tty_the_text_is_what_a_script_is_given() {
    let home = Home::new();
    let ran = home.run({
        let mut command = home.linearpen();
        command.env("LINEAR_API_KEY", KEY);
        command
    });
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.starts_with("Acme - Ada\n"), "{}", ran.stdout);
    // A pipe is a script's: no words about views under the text.
    assert!(!ran.stdout.contains("shows no views"));
}

#[test]
fn help_and_version_say_and_an_unknown_option_is_refused() {
    let home = Home::new();
    let help = home.run({
        let mut command = home.linearpen();
        command.arg("--help");
        command
    });
    assert_eq!(help.status, Some(0));
    assert!(help.stdout.contains("usage: linearpen [--text]"));
    let version = home.run({
        let mut command = home.linearpen();
        command.arg("--version");
        command
    });
    assert_eq!(
        version.stdout,
        format!("linearpen {}\n", env!("CARGO_PKG_VERSION"))
    );
    let refused = home.run({
        let mut command = home.linearpen();
        command.arg("--nope");
        command
    });
    assert_eq!(refused.status, Some(2));
    assert!(refused.stderr.contains("not understood"));
    assert!(home.calls().is_empty());
}
