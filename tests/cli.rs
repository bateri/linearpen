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
    // The open issues, then the closed ones.
    assert_eq!(sent(&home, "MyIssues").len(), 2);
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
        assert_eq!(call.argv, ["-q", "--config", "-"]);
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
    let ran = linearpen(&home, &["update", "ACM-412", "--team", "WEB"], &[], None);
    assert_eq!(ran.status, Some(2));
    assert!(
        ran.stderr.contains("never moves an issue to another team"),
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

// --- Files ---------------------------------------------------------------------------------------

const PNG: &[u8] = include_bytes!("../test/stand-in/picture.png");
const JPG: &[u8] = include_bytes!("../test/stand-in/picture.jpg");
const PICTURES: [(&str, &str); 1] = [("STANDIN_ISSUE", "pictures")];

/// A PNG file of the home, by `name`, and its path.
fn picture(home: &Home, name: &str) -> String {
    let path = home.path(name);
    std::fs::write(&path, PNG).unwrap();
    path.display().to_string()
}

/// A file of the home with these `bytes`, by `name`, and its path.
fn file(home: &Home, name: &str, bytes: &[u8]) -> String {
    let path = home.path(name);
    std::fs::write(&path, bytes).unwrap();
    path.display().to_string()
}

/// What the stand-in storage was sent as its `n`th file (from 1).
fn stored(home: &Home, n: usize) -> Vec<u8> {
    std::fs::read(format!("{}.put.{n}", home.log().display())).unwrap()
}

/// The address each call of the stand-in `curl` had.
fn addresses(home: &Home) -> Vec<String> {
    home.calls()
        .iter()
        .filter_map(|call| {
            call.stdin
                .lines()
                .find_map(|line| line.strip_prefix("url = \"")?.strip_suffix('"'))
                .map(str::to_owned)
        })
        .collect()
}

#[test]
fn show_numbers_the_files_of_an_issue_and_fetches_none_until_asked_to() {
    let home = Home::new();
    let ran = linearpen(&home, &["show", "ACM-412"], &PICTURES, None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    for placeholder in [
        "[image 1: Checkout error]",
        "[image 2: trace.jpg]",
        "[image 3: elsewhere]",
        "[image 4: huge]",
        "[image 5: odd]",
        "[image 6: missing.png]",
        "[file 7: build log.txt]",
        "[a site](https://example.com/z)",
    ] {
        assert!(
            ran.stdout.contains(placeholder),
            "{placeholder}: {}",
            ran.stdout
        );
    }
    assert!(
        ran.stdout
            .ends_with("\n7 files: linearpen show ACM-412 --files DIR saves them\n"),
        "{}",
        ran.stdout
    );
    // Only the issue was asked for: no address of the files.
    assert!(
        addresses(&home)
            .iter()
            .all(|url| url == "http://127.0.0.1:9/graphql"),
        "{:?}",
        addresses(&home)
    );
    // The JSON keeps the texts as Linear wrote them and lists the files.
    let json = linearpen(&home, &["show", "ACM-412", "--json"], &PICTURES, None);
    let value: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert!(
        value["description"]
            .as_str()
            .unwrap()
            .contains("![Checkout error](")
    );
    assert_eq!(value["files"].as_array().unwrap().len(), 7);
    assert_eq!(value["files"][5]["in"], "comment");
    assert_eq!(value["files"][6]["kind"], "file");
    assert_eq!(value["files"][6]["name"], "build log.txt");
}

#[test]
fn files_saves_the_files_of_linear_sends_the_key_to_that_host_only_and_a_second_run_changes_nothing()
 {
    let home = Home::new();
    let dir = home.path("pics/of/it");
    let args = ["show", "ACM-412", "--files", dir.to_str().unwrap()];
    let ran = linearpen(&home, &args, &PICTURES, None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(
        std::fs::read(dir.join("1-Checkout-error.png")).unwrap(),
        PNG
    );
    assert_eq!(std::fs::read(dir.join("2-trace.jpg")).unwrap(), JPG);
    // A file that is not a picture is saved as it is, with the extension its name has; and what the
    // stand-in gave for `text.png` (words, not a picture) is a file all the same.
    assert_eq!(
        std::fs::read(dir.join("7-build-log.txt")).unwrap(),
        b"line one\nline two\n"
    );
    assert_eq!(
        std::fs::read(dir.join("5-odd.png")).unwrap(),
        b"this is not a picture"
    );
    for line in [
        "  3  description  not on Linear  https://example.com/y.png\n",
        "  4  description  too large  https://uploads.linear.app/stand-in/big.png\n",
        "  6  comment by Deniz  could not be fetched  https://uploads.linear.app/stand-in/missing.png\n",
    ] {
        assert!(ran.stdout.contains(line), "{line}: {}", ran.stdout);
    }
    assert!(
        ran.stdout.contains(&format!(
            "  1  description  {}",
            dir.join("1-Checkout-error.png").display()
        )),
        "{}",
        ran.stdout
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    // The key went to Linear's two hosts and example.com was not asked at all.
    let urls = addresses(&home);
    assert!(
        urls.iter().all(|url| url == "http://127.0.0.1:9/graphql"
            || url.starts_with("https://uploads.linear.app/stand-in/")),
        "{urls:?}"
    );
    for call in home.calls() {
        // Every curl is `curl -q --config -`: `-q` first, so that no `.curlrc` of the person's is read.
        assert_eq!(call.argv, ["-q", "--config", "-"], "{}", call.stdin);
        // `curl` follows no redirect: the program does.
        assert!(!call.stdin.contains("location"), "{}", call.stdin);
    }
    assert_eq!(writes(&home), 0);
    // The same again prints the same paths and makes no more files.
    let again = linearpen(&home, &args, &PICTURES, None);
    assert_eq!(again.stdout, ran.stdout);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
}

#[test]
fn a_file_sent_on_elsewhere_is_followed_by_the_program_and_the_key_stays_at_linear() {
    let home = Home::new();
    let dir = home.path("moved");
    let ran = linearpen(
        &home,
        &["show", "ACM-412", "--files", dir.to_str().unwrap()],
        &[("STANDIN_ISSUE", "moved")],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    // `.../moved.png` is sent on to https://cdn.stand-in/moved.png: the file is what is there, and the
    // body of the redirect is not in it.
    assert_eq!(
        std::fs::read(dir.join("1-Checkout-error.png")).unwrap(),
        PNG
    );
    // A place that is not https is not followed, and a file that sends itself on is given up on.
    assert!(!dir.join("2-trace.jpg").exists());
    assert!(!dir.join("7-build-log.txt").exists());
    let calls = home.calls();
    let to = |url: &str| -> Vec<&common::Call> {
        calls
            .iter()
            .filter(|call| {
                call.stdin
                    .lines()
                    .any(|line| line == format!("url = \"{url}\""))
            })
            .collect()
    };
    let cdn = to("https://cdn.stand-in/moved.png");
    assert_eq!(cdn.len(), 1);
    // The key went to Linear's file and not to the place it was sent on to.
    assert!(!cdn[0].stdin.contains("Authorization"), "{}", cdn[0].stdin);
    assert!(!cdn[0].stdin.contains("lin_api"), "{}", cdn[0].stdin);
    let first = to("https://uploads.linear.app/stand-in/moved.png");
    assert_eq!(first.len(), 1);
    assert!(first[0].stdin.contains("Authorization"));
    // Nothing was asked of the place that is not https, and a file that sends itself to itself was
    // asked for five times and no more.
    assert!(to("http://cdn.stand-in/away.png").is_empty());
    let forever = addresses(&home)
        .iter()
        .filter(|url| url.contains("moved-forever"))
        .count();
    assert_eq!(forever, 5);
    assert_eq!(writes(&home), 0);
}

#[test]
fn comment_with_files_puts_them_in_storage_first_and_posts_once_with_their_lines() {
    let home = Home::new();
    let shot = picture(&home, "shot one.png");
    let ran = linearpen(
        &home,
        &[
            "comment",
            "ACM-412",
            "--body",
            "Seen here",
            "--attach",
            &shot,
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "ACM-412: comment added with 1 file\n");
    let posted = sent(&home, "CommentCreate");
    assert_eq!(posted.len(), 1);
    assert_eq!(
        posted[0]["variables"]["input"]["body"],
        "Seen here\n\n![shot one.png](https://uploads.linear.app/stand-in/1)"
    );
    assert_eq!(stored(&home, 1), PNG);
    // The key went to Linear and not to the storage; its signed address is nowhere said.
    let calls = home.calls();
    let put = calls
        .iter()
        .find(|call| call.stdin.contains("request = \"PUT\""))
        .expect("a PUT");
    assert!(!put.stdin.contains(KEY), "{}", put.stdin);
    assert!(
        put.stdin.contains("https://storage.stand-in/put/1"),
        "{}",
        put.stdin
    );
    assert!(!ran.stdout.contains("SECRET") && !ran.stderr.contains("SECRET"));
    // Files only is a comment too.
    let home = Home::new();
    let shot = picture(&home, "a.png");
    let ran = linearpen(&home, &["comment", "ACM-412", "--attach", &shot], &[], None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(
        sent(&home, "CommentCreate")[0]["variables"]["input"]["body"],
        "![a.png](https://uploads.linear.app/stand-in/1)"
    );
}

#[test]
fn a_comment_or_an_issue_that_would_be_too_long_with_the_lines_of_its_files_is_refused_before_any_is_sent()
 {
    let home = Home::new();
    let shot = picture(&home, "shot.png");
    // Under the most as words, over it with a file's line (which is at most ~250 characters).
    let body = "x".repeat(9_900);
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--body", &body, "--attach", &shot],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(
        ran.stderr.contains("with the lines of its files"),
        "{}",
        ran.stderr
    );
    // Nothing was asked of Linear, nothing was stored, nothing was posted.
    assert!(home.calls().is_empty(), "{:?}", addresses(&home));
    let description = "y".repeat(49_900);
    let ran = linearpen(
        &home,
        &[
            "create",
            "--team",
            "ACM",
            "--title",
            "T",
            "--description",
            &description,
            "--attach",
            &shot,
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(
        ran.stderr.contains("with the lines of its files"),
        "{}",
        ran.stderr
    );
    assert!(home.calls().is_empty(), "{:?}", addresses(&home));
    // Words that leave room go through.
    let ran = linearpen(
        &home,
        &[
            "comment",
            "ACM-412",
            "--body",
            &"x".repeat(9_000),
            "--attach",
            &shot,
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
}

#[test]
fn a_file_of_any_kind_is_sent_as_a_link_by_what_it_holds_and_one_that_cannot_be_sent_ends_with_two()
{
    let home = Home::new();
    // A PDF by its first bytes though its name says nothing, a log, bytes that are nothing known, a
    // GIF (a picture) under a name that is not one: each is told to be what it is.
    let pdf = file(
        &home,
        "report",
        b"%PDF-1.7\n1 0 obj\n<< /Type /Pages /Count 2 >>\nendobj\n",
    );
    let log = file(&home, "build log.txt", b"line one\nline two\n");
    let blob = file(&home, "blob.bin", &[0, 1, 2, 3, 4, 5]);
    let gif = file(&home, "anim.dat", b"GIF89a.......");
    let ran = linearpen(
        &home,
        &[
            "comment", "ACM-412", "--body", "Files", "--attach", &pdf, "--attach", &log,
            "--attach", &blob, "--attach", &gif,
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "ACM-412: comment added with 4 files\n");
    let posted = sent(&home, "CommentCreate");
    assert_eq!(
        posted[0]["variables"]["input"]["body"],
        "Files\n\n[report](https://uploads.linear.app/stand-in/1)\n[build log.txt](https://uploads.linear.app/stand-in/2)\n[blob.bin](https://uploads.linear.app/stand-in/3)\n![anim.dat](https://uploads.linear.app/stand-in/4)"
    );
    assert_eq!(stored(&home, 2), b"line one\nline two\n");
    assert_eq!(stored(&home, 3), [0, 1, 2, 3, 4, 5]);
    let types: Vec<String> = sent(&home, "FileUpload")
        .iter()
        .map(|call| {
            call["variables"]["contentType"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(
        types,
        [
            "application/pdf",
            "application/octet-stream",
            "application/octet-stream",
            "image/gif"
        ]
    );
    // The copy that was sent is not left behind: the run's temporary folder is as it was.
    assert_eq!(std::fs::read_dir(home.path("tmp")).unwrap().count(), 0);
}

#[test]
fn a_file_that_is_empty_or_a_folder_or_not_there_or_too_large_ends_with_two_and_asks_nothing() {
    let home = Home::new();
    let empty = file(&home, "empty.txt", b"");
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--attach", &empty],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(ran.stderr.contains("is empty"), "{}", ran.stderr);
    let folder = home.path("a folder");
    std::fs::create_dir(&folder).unwrap();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--attach", folder.to_str().unwrap()],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(ran.stderr.contains("is not a file"), "{}", ran.stderr);
    // A hundred MiB and a byte: nothing of it is read (the file is sparse), and nothing is asked.
    let big = home.path("big.bin");
    let opened = std::fs::File::create(&big).unwrap();
    opened.set_len(100 * 1024 * 1024 + 1).unwrap();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--attach", big.to_str().unwrap()],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(ran.stderr.contains("is over 100 MiB"), "{}", ran.stderr);
    let missing = linearpen(
        &home,
        &[
            "comment",
            "ACM-412",
            "--body",
            "x",
            "--attach",
            "/nowhere/a.png",
        ],
        &[],
        None,
    );
    assert_eq!(missing.status, Some(2), "{}", missing.stderr);
    // None of them asked Linear anything.
    assert!(home.calls().is_empty());
    // A hundred MiB exactly is a file: it is sent, and its transfer is given time by its size.
    let exact = home.path("exact.bin");
    let opened = std::fs::File::create(&exact).unwrap();
    opened.set_len(100 * 1024 * 1024).unwrap();
    let ran = linearpen(
        &home,
        &[
            "comment",
            "ACM-412",
            "--body",
            "x",
            "--attach",
            exact.to_str().unwrap(),
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let calls = home.calls();
    let put = calls
        .iter()
        .find(|call| call.stdin.contains("request = \"PUT\""))
        .expect("a PUT");
    assert!(put.stdin.contains("max-time = 1168"), "{}", put.stdin);
    assert_eq!(
        std::fs::metadata(format!("{}.put.1", home.log().display()))
            .unwrap()
            .len(),
        100 * 1024 * 1024
    );
}

#[test]
fn a_file_that_cannot_be_stored_writes_nothing() {
    let home = Home::new();
    let shot = picture(&home, "a.png");
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--body", "x", "--attach", &shot],
        &[("STANDIN_PUT", "refused")],
        None,
    );
    assert_eq!(ran.status, Some(1), "{}", ran.stderr);
    assert!(ran.stderr.contains("nothing was written"), "{}", ran.stderr);
    assert!(!ran.stderr.contains("SECRET") && !ran.stderr.contains("storage.stand-in"));
    assert!(sent(&home, "CommentCreate").is_empty());
}

#[test]
fn create_with_files_ends_its_description_with_their_lines() {
    let home = Home::new();
    let shot = picture(&home, "a.png");
    let ran = linearpen(
        &home,
        &[
            "create",
            "--team",
            "ACM",
            "--title",
            "Fix it",
            "--description",
            "Look",
            "--attach",
            &shot,
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.starts_with("ACM-900  "), "{}", ran.stdout);
    assert!(
        ran.stdout.trim_end().ends_with("with 1 file"),
        "{}",
        ran.stdout
    );
    let made = sent(&home, "IssueCreate");
    assert_eq!(made.len(), 1);
    assert_eq!(
        made[0]["variables"]["input"]["description"],
        "Look\n\n![a.png](https://uploads.linear.app/stand-in/1)"
    );
    // The files first, then the issue.
    let order: Vec<_> = home
        .calls()
        .iter()
        .filter_map(|call| {
            ["FileUpload", "IssueCreate"].into_iter().find(|name| {
                call.stdin
                    .contains(&format!("operationName\\\":\\\"{name}"))
            })
        })
        .collect();
    assert_eq!(order, ["FileUpload", "IssueCreate"]);
}

#[test]
fn update_with_files_adds_them_to_the_whole_description_read_from_linear_just_now() {
    let home = Home::new();
    let shot = picture(&home, "a.png");
    let ran = linearpen(&home, &["update", "ACM-412", "--attach", &shot], &[], None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.starts_with("ACM-412  "), "{}", ran.stdout);
    assert!(
        ran.stdout.trim_end().ends_with("with 1 file"),
        "{}",
        ran.stdout
    );
    let batches = sent(&home, "IssueBatchUpdate");
    assert_eq!(batches.len(), 1);
    let input = &batches[0]["variables"]["input"];
    // The description is all there is of it, and nothing else is in the change.
    assert_eq!(input.as_object().unwrap().len(), 1, "{input}");
    let description = input["description"].as_str().unwrap();
    assert!(
        description.starts_with("The public API has no limit per key."),
        "{description}"
    );
    assert!(description.contains("(ACM-371)"), "{description}");
    assert!(
        description.ends_with("\n\n![a.png](https://uploads.linear.app/stand-in/1)"),
        "{description}"
    );
    assert_eq!(
        batches[0]["variables"]["ids"],
        serde_json::json!(["issue-412"])
    );
    // Where the answer to the write is lost, the end is 3 and it was sent once.
    let home = Home::new();
    let shot = picture(&home, "a.png");
    let lost = linearpen(
        &home,
        &["update", "ACM-412", "--attach", &shot],
        &[("STANDIN_CHANGE", "lost")],
        None,
    );
    assert_eq!(lost.status, Some(3), "{}", lost.stderr);
    assert_eq!(sent(&home, "IssueBatchUpdate").len(), 1);
}

#[test]
fn update_description_writes_the_words_whole_and_alone_to_the_one_issue() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["update", "ACM-412", "--description", "-"],
        &[],
        Some("New words\n\n- one\n"),
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let batches = sent(&home, "IssueBatchUpdate");
    assert_eq!(batches.len(), 1);
    assert_eq!(writes(&home), 1);
    assert_eq!(
        batches[0]["variables"]["ids"],
        serde_json::json!(["issue-412"])
    );
    assert_eq!(
        batches[0]["variables"]["input"],
        serde_json::json!({"description": "New words\n\n- one"})
    );
    assert!(ran.stdout.starts_with("ACM-412  "), "{}", ran.stdout);
    assert!(
        ran.stdout.ends_with("description written\n"),
        "{}",
        ran.stdout
    );
    // Not with another field: nothing is asked of Linear.
    let home = Home::new();
    let ran = linearpen(
        &home,
        &[
            "update",
            "ACM-412",
            "--description",
            "x",
            "--status",
            "Done",
        ],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(2), "{}", ran.stderr);
    assert!(home.calls().is_empty());
}

#[test]
fn comment_edit_last_puts_the_words_in_the_place_of_the_persons_last_comment() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--edit", "last", "--body", "-"],
        &[("STANDIN_ISSUE", "mine")],
        Some("Keep a fallback, and log it.\n"),
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "ACM-412: comment edited\n");
    assert!(
        sent(&home, "CommentCreate").is_empty(),
        "no comment is posted"
    );
    let edited = sent(&home, "CommentUpdate");
    assert_eq!(edited.len(), 1);
    assert_eq!(writes(&home), 1);
    assert_eq!(edited[0]["variables"]["id"], "c-mert");
    assert_eq!(
        edited[0]["variables"]["input"],
        serde_json::json!({"body": "Keep a fallback, and log it."})
    );
    // Linear is asked whose the comment is before it is written.
    assert_eq!(sent(&home, "Comment").len(), 1);
}

#[test]
fn comment_edit_of_someone_elses_comment_or_of_none_writes_nothing() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &[
            "comment", "ACM-412", "--edit", "c-mert", "--body", "Mine now",
        ],
        &[("STANDIN_COMMENT", "theirs")],
        None,
    );
    assert_eq!(ran.status, Some(1), "{}", ran.stderr);
    assert!(ran.stderr.contains("not yours"), "{}", ran.stderr);
    assert_eq!(writes(&home), 0);
    // No comment on the issue is the person's.
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--edit", "last", "--body", "x"],
        &[],
        None,
    );
    assert_eq!(ran.status, Some(1), "{}", ran.stderr);
    assert!(
        ran.stderr.contains("no comment on ACM-412"),
        "{}",
        ran.stderr
    );
    assert_eq!(writes(&home), 0);
}

#[test]
fn comment_edit_whose_answer_is_lost_ends_with_three_and_is_not_sent_again() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["comment", "ACM-412", "--edit", "c-mert", "--body", "New"],
        &[("STANDIN_EDIT", "lost")],
        None,
    );
    assert_eq!(ran.status, Some(3), "{}", ran.stderr);
    assert_eq!(sent(&home, "CommentUpdate").len(), 1);
}

#[test]
fn show_json_tells_each_comment_by_its_id_and_whether_it_is_the_persons() {
    let home = Home::new();
    let ran = linearpen(
        &home,
        &["show", "ACM-412", "--json"],
        &[("STANDIN_ISSUE", "mine")],
        None,
    );
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let shown: serde_json::Value = serde_json::from_str(&ran.stdout).unwrap();
    let comments = shown["comments"].as_array().unwrap();
    assert_eq!(comments[0]["id"], "c-deniz");
    assert_eq!(comments[0]["mine"], false);
    assert_eq!(comments[1]["id"], "c-mert");
    assert_eq!(comments[1]["mine"], true);
    assert!(comments[1]["edited_at"].is_null());
}
