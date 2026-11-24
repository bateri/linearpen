//! What GitHub says of a pull request linked to an issue, asked with `gh`, GitHub's own command, as
//! the person signed in to it: the program holds no key of GitHub's, gives `gh` nothing of Linear's,
//! and only reads. Where `gh` is not there, nothing here is run (`here`), and the page opens the
//! address in the browser.

use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::linear::plain;
use crate::linear::types::pull_request_address;
use crate::work::runner::{Ran, Runner, command_of};

/// What is asked of `gh pr view --json`.
pub const FIELDS: &str = "number,title,state,isDraft,author,headRefName,baseRefName,additions,\
deletions,changedFiles,files,reviewDecision,latestReviews,reviewRequests,statusCheckRollup,url";

/// The most of a word of GitHub's told, the files listed, and the checks named of each kind.
const WORDS_MOST: usize = 300;
pub const FILES_MOST: usize = 100;
const CHECKS_MOST: usize = 50;

/// What `gh` is run with: it asks the person nothing, tells of no update, and writes no colours.
const QUIET: [(&str, &str); 4] = [
    ("GH_PROMPT_DISABLED", "1"),
    ("GH_NO_UPDATE_NOTIFIER", "1"),
    ("NO_COLOR", "1"),
    ("CLICOLOR", "0"),
];

/// Why GitHub did not tell a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// `gh` is not there.
    Missing,
    /// `gh` is there and not signed in to GitHub: what it says.
    SignedOut(String),
    /// `gh` ran and GitHub did not tell it: what `gh` says.
    Failed(String),
}

/// A file the pull request changes, and how many lines it adds and takes away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
    pub additions: u64,
    pub deletions: u64,
}

/// The checks of its last commit: how many passed and were skipped, and the names of those that failed
/// and those still running.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checks {
    pub passed: usize,
    pub failed: Vec<String>,
    pub running: Vec<String>,
    pub skipped: usize,
}

/// The review: what GitHub decided of it, the last review of each who reviewed, and who is asked and has
/// not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Review {
    pub decision: Option<&'static str>,
    pub reviews: Vec<(String, &'static str)>,
    pub waiting: Vec<String>,
}

/// A pull request as GitHub tells it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pull {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub repository: String,
    /// `open`, `draft`, `merged` or `closed`.
    pub state: &'static str,
    pub author: Option<String>,
    pub head: String,
    pub base: String,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub files: Vec<File>,
    pub checks: Checks,
    pub review: Review,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Raw {
    number: u64,
    title: String,
    state: String,
    is_draft: bool,
    author: Option<Login>,
    head_ref_name: String,
    base_ref_name: String,
    additions: u64,
    deletions: u64,
    changed_files: u64,
    files: Vec<RawFile>,
    review_decision: Option<String>,
    latest_reviews: Vec<RawReview>,
    review_requests: Vec<RawRequest>,
    status_check_rollup: Vec<RawCheck>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Login {
    login: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawFile {
    path: String,
    additions: u64,
    deletions: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawReview {
    author: Option<Login>,
    state: String,
}

/// One asked to review: a person (`login`) or a team (`name`).
#[derive(Deserialize, Default)]
#[serde(default)]
struct RawRequest {
    login: Option<String>,
    name: Option<String>,
}

/// A check: a run of a workflow (`CheckRun`: `name`, `status`, `conclusion`) or a status of the commit
/// (`StatusContext`: `context`, `state`).
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawCheck {
    name: Option<String>,
    context: Option<String>,
    workflow_name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    state: Option<String>,
}

/// A word of GitHub's as it is told: no control character, cut.
fn words(text: &str) -> String {
    let plain = plain(text);
    match plain.char_indices().nth(WORDS_MOST) {
        Some((end, _)) => plain[..end].to_owned(),
        None => plain,
    }
}

/// How a check came out: `passed`, `failed`, `skipped`, or `running` (also what it says that is not
/// known: never counted as passed).
fn outcome(check: &RawCheck) -> &'static str {
    if let Some(state) = check.state.as_deref() {
        return match state {
            "SUCCESS" => "passed",
            "FAILURE" | "ERROR" => "failed",
            _ => "running",
        };
    }
    if check.status.as_deref() != Some("COMPLETED") {
        return "running";
    }
    match check.conclusion.as_deref() {
        Some("SUCCESS") => "passed",
        Some("SKIPPED" | "NEUTRAL") => "skipped",
        Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE") => {
            "failed"
        }
        _ => "running",
    }
}

/// The checks, the last run of each (a workflow runs a check again: the run told last is the one
/// that stands).
fn checks_of(raw: &[RawCheck]) -> Checks {
    let mut last: Vec<(String, &'static str)> = Vec::new();
    for (index, check) in raw.iter().enumerate() {
        let name = check.name.as_deref().or(check.context.as_deref());
        // (A check with no name is one of its own: it is not the run again of another with none.)
        let key = match name {
            Some(name) => format!(
                "{}\u{1f}{name}",
                check.workflow_name.as_deref().unwrap_or("")
            ),
            None => format!("\u{1e}{index}\u{1f}A check"),
        };
        let came = outcome(check);
        match last.iter_mut().find(|(held, _)| *held == key) {
            Some(held) => held.1 = came,
            None => last.push((key, came)),
        }
    }
    let mut checks = Checks::default();
    for (key, came) in last {
        let name = words(key.split('\u{1f}').nth(1).unwrap_or(""));
        match came {
            "passed" => checks.passed += 1,
            "skipped" => checks.skipped += 1,
            "failed" if checks.failed.len() < CHECKS_MOST => checks.failed.push(name),
            "running" if checks.running.len() < CHECKS_MOST => checks.running.push(name),
            _ => {}
        }
    }
    checks
}

fn review_of(raw: &Raw) -> Review {
    let decision = match raw.review_decision.as_deref() {
        Some("APPROVED") => Some("approved"),
        Some("CHANGES_REQUESTED") => Some("changesRequested"),
        Some("REVIEW_REQUIRED") => Some("required"),
        _ => None,
    };
    let reviews = raw
        .latest_reviews
        .iter()
        .filter_map(|review| {
            let state = match review.state.as_str() {
                "APPROVED" => "approved",
                "CHANGES_REQUESTED" => "changesRequested",
                "COMMENTED" => "commented",
                "DISMISSED" => "dismissed",
                _ => return None,
            };
            let who = review.author.as_ref()?.login.as_deref()?;
            Some((words(who), state))
        })
        .take(CHECKS_MOST)
        .collect();
    let waiting = raw
        .review_requests
        .iter()
        .filter_map(|asked| asked.login.as_deref().or(asked.name.as_deref()))
        .map(words)
        .filter(|who| !who.is_empty())
        .take(CHECKS_MOST)
        .collect();
    Review {
        decision,
        reviews,
        waiting,
    }
}

/// The pull request `gh` told, at the address `url` it was asked for.
fn pull_of(text: &str, url: &str) -> Result<Pull, Problem> {
    let raw: Raw = serde_json::from_str(text)
        .map_err(|_| Problem::Failed("GitHub's answer could not be read".to_owned()))?;
    let state = match (raw.state.as_str(), raw.is_draft) {
        ("MERGED", _) => "merged",
        ("CLOSED", _) => "closed",
        (_, true) => "draft",
        _ => "open",
    };
    let repository = url
        .strip_prefix("https://github.com/")
        .and_then(|rest| rest.rsplit_once("/pull/"))
        .map_or_else(String::new, |(repository, _)| repository.to_owned());
    Ok(Pull {
        number: raw.number,
        title: words(&raw.title),
        url: url.to_owned(),
        repository,
        state,
        author: raw
            .author
            .as_ref()
            .and_then(|author| author.login.as_deref())
            .map(words),
        head: words(&raw.head_ref_name),
        base: words(&raw.base_ref_name),
        additions: raw.additions,
        deletions: raw.deletions,
        changed_files: raw.changed_files,
        files: raw
            .files
            .iter()
            .take(FILES_MOST)
            .map(|file| File {
                path: words(&file.path),
                additions: file.additions,
                deletions: file.deletions,
            })
            .collect(),
        checks: checks_of(&raw.status_check_rollup),
        review: review_of(&raw),
    })
}

/// Whether what `gh` said on failing is that it is not signed in to GitHub.
fn signed_out(err: &str) -> bool {
    let err = err.to_ascii_lowercase();
    err.contains("gh auth login")
        || err.contains("not logged")
        || err.contains("authentication required")
}

/// Whether `gh` is there to ask: `gh --version`, which asks GitHub nothing.
#[must_use]
pub fn here(runner: &dyn Runner) -> bool {
    runner
        .run_with("gh", &["--version".to_owned()], None, &QUIET)
        .is_ok_and(|ran| ran.ok)
}

/// How long `gh` is given to say its version, and to tell a pull request (it asks GitHub).
// (A test build never runs the system's `gh`: what times it is the program's alone.)
#[cfg_attr(test, allow(dead_code))]
const VERSION_SECONDS: u64 = 3;
const VIEW_SECONDS: u64 = 20;

/// The system's `gh`, given a time: one that has not ended by then is stopped, and that is a failure.
/// A `gh` that waits on a network that never answers does not leave the panel asking for ever, nor the
/// page's first answer waiting for its version.
#[cfg_attr(test, allow(dead_code))]
pub struct Timed;

impl Runner for Timed {
    fn run_with(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &[(&str, &str)],
    ) -> io::Result<Ran> {
        self.run_until(program, args, cwd, env, &|| false)
    }

    fn run_until(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &[(&str, &str)],
        stopped: &dyn Fn() -> bool,
    ) -> io::Result<Ran> {
        let seconds = if args.first().map(String::as_str) == Some("--version") {
            VERSION_SECONDS
        } else {
            VIEW_SECONDS
        };
        run_for(
            program,
            args,
            cwd,
            env,
            Duration::from_secs(seconds),
            stopped,
        )
    }
}

/// Runs `program` as the system's runner does (no shell, nothing on its stdin, without the key), and
/// stops it where it has not ended in `limit` (`ErrorKind::TimedOut`) or where `stopped` says so (the
/// page let the call go: `ErrorKind::Interrupted`).
fn run_for(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    env: &[(&str, &str)],
    limit: Duration,
    stopped: &dyn Fn() -> bool,
) -> io::Result<Ran> {
    let mut command = command_of(program, args, cwd);
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn()?;
    // (Each pipe is read as it fills, so that a child that writes much is not held on a full pipe.)
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    };
    let out = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let err = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait()? {
            let text = |reader: std::thread::JoinHandle<Vec<u8>>| {
                String::from_utf8_lossy(&reader.join().unwrap_or_default()).into_owned()
            };
            return Ok(Ran {
                ok: status.success(),
                out: text(out),
                err: text(err),
            });
        }
        let late = Instant::now() >= deadline;
        if late || stopped() {
            let _ = child.kill();
            let _ = child.wait();
            // (The readers are let go: a child of its own that holds the pipes is not waited for.)
            return Err(if late {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{program} did not end in {} seconds", limit.as_secs()),
                )
            } else {
                io::Error::new(io::ErrorKind::Interrupted, format!("{program} was stopped"))
            });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What GitHub says of the pull request at `url`, which must be the address of one of GitHub's and
/// nothing more (else `gh` is not run).
///
/// # Errors
///
/// `gh` is not there, not signed in, or GitHub did not tell it.
#[cfg(test)]
pub fn view(runner: &dyn Runner, url: &str) -> Result<Pull, Problem> {
    view_until(runner, url, &|| false)
}

/// As `view`, stopping `gh` where `stopped` says so (the page let the call go).
///
/// # Errors
///
/// As `view`; stopped, `Problem::Failed`.
pub fn view_until(
    runner: &dyn Runner,
    url: &str,
    stopped: &dyn Fn() -> bool,
) -> Result<Pull, Problem> {
    let Some(address) = pull_request_address(url) else {
        return Err(Problem::Failed(
            "not the address of a pull request of GitHub's".to_owned(),
        ));
    };
    let args: Vec<String> = ["pr", "view", address.as_str(), "--json", FIELDS]
        .into_iter()
        .map(str::to_owned)
        .collect();
    match runner.run_until("gh", &args, None, &QUIET, stopped) {
        Err(why) if why.kind() == io::ErrorKind::NotFound => Err(Problem::Missing),
        Err(why) if why.kind() == io::ErrorKind::TimedOut => Err(Problem::Failed(format!(
            "GitHub did not answer in {VIEW_SECONDS} seconds"
        ))),
        Err(why) => Err(Problem::Failed(format!("gh could not be run: {why}"))),
        Ok(ran) if !ran.ok => {
            let why = words(&ran.reason("GitHub did not tell the pull request"));
            Err(if signed_out(&ran.err) {
                Problem::SignedOut(why)
            } else {
                Problem::Failed(why)
            })
        }
        Ok(ran) => pull_of(&ran.out, url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gh_that_does_not_end_in_its_time_is_stopped_and_that_is_said() {
        let began = Instant::now();
        let ran = run_for(
            "sleep",
            &["5".to_owned()],
            None,
            &[],
            Duration::from_millis(300),
            &|| false,
        );
        let why = ran.expect_err("stopped");
        assert_eq!(why.kind(), io::ErrorKind::TimedOut);
        assert!(
            began.elapsed() < Duration::from_secs(3),
            "{:?}",
            began.elapsed()
        );
        // One that ends in its time is read as it ended.
        let ran = run_for(
            "sh",
            &["-c".to_owned(), "echo out; echo err >&2".to_owned()],
            None,
            &[],
            Duration::from_secs(5),
            &|| false,
        )
        .expect("it ends");
        assert!(ran.ok);
        assert_eq!(ran.out, "out\n");
        assert_eq!(ran.err, "err\n");
        // One the page let go of is stopped at once.
        let began = Instant::now();
        let why = run_for(
            "sleep",
            &["5".to_owned()],
            None,
            &[],
            Duration::from_secs(20),
            &|| began.elapsed() > Duration::from_millis(200),
        )
        .expect_err("let go");
        assert_eq!(why.kind(), io::ErrorKind::Interrupted);
        assert!(
            began.elapsed() < Duration::from_secs(3),
            "{:?}",
            began.elapsed()
        );
    }

    #[test]
    fn checks_with_no_name_are_each_their_own() {
        let raw: Vec<RawCheck> = serde_json::from_str(
            r#"[{"status":"COMPLETED","conclusion":"FAILURE"},{"status":"COMPLETED","conclusion":"SUCCESS"}]"#,
        )
        .unwrap();
        let checks = checks_of(&raw);
        assert_eq!(checks.passed, 1);
        assert_eq!(checks.failed.len(), 1);
    }
    use crate::work::runner::Ran;
    use std::sync::Mutex;

    /// A call: the program, its arguments and the variables it was given.
    type Call = (String, Vec<String>, Vec<(String, String)>);

    /// `gh` as a test says: what it answers, and the calls it was asked.
    struct Gh {
        answer: io::Result<Ran>,
        asked: Mutex<Vec<Call>>,
    }

    impl Runner for Gh {
        fn run_with(
            &self,
            program: &str,
            args: &[String],
            _: Option<&std::path::Path>,
            env: &[(&str, &str)],
        ) -> io::Result<Ran> {
            self.asked.lock().unwrap().push((
                program.to_owned(),
                args.to_vec(),
                env.iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
            ));
            match &self.answer {
                Ok(ran) => Ok(ran.clone()),
                Err(why) => Err(io::Error::new(why.kind(), why.to_string())),
            }
        }
    }

    fn gh(answer: io::Result<Ran>) -> Gh {
        Gh {
            answer,
            asked: Mutex::new(Vec::new()),
        }
    }

    fn said(ok: bool, out: &str, err: &str) -> io::Result<Ran> {
        Ok(Ran {
            ok,
            out: out.to_owned(),
            err: err.to_owned(),
        })
    }

    const URL: &str = "https://github.com/acme/api/pull/231";

    #[test]
    fn a_pull_request_is_read_from_what_gh_tells_with_the_last_run_of_each_check() {
        let runner = gh(said(
            true,
            include_str!("../../../test/stand-in/gh-pr-view.json"),
            "",
        ));
        let pull = view(&runner, URL).unwrap();
        assert_eq!(pull.number, 231);
        assert_eq!(pull.repository, "acme/api");
        assert_eq!(pull.state, "open");
        assert_eq!(pull.author.as_deref(), Some("ada"));
        assert_eq!(
            (pull.head.as_str(), pull.base.as_str()),
            ("ada/acm-412-rate-limits", "main")
        );
        assert_eq!(
            (pull.additions, pull.deletions, pull.changed_files),
            (214, 31, 6)
        );
        assert_eq!(pull.files.len(), 6);
        // `lint` failed once and passed when run again: it passed. `test (linux)` failed.
        assert_eq!(pull.checks.failed, ["test (linux)"]);
        assert_eq!(pull.checks.running, ["deploy preview"]);
        assert_eq!((pull.checks.passed, pull.checks.skipped), (3, 1));
        assert_eq!(pull.review.decision, Some("changesRequested"));
        assert_eq!(
            pull.review.reviews,
            [
                ("deniz".to_owned(), "changesRequested"),
                ("copilot-pull-request-reviewer".to_owned(), "commented")
            ]
        );
        assert_eq!(pull.review.waiting, ["mert", "api-reviewers"]);
        // Asked as one list of words, quietly, never through a shell.
        let asked = runner.asked.lock().unwrap();
        assert_eq!(asked[0].0, "gh");
        assert_eq!(asked[0].1, ["pr", "view", URL, "--json", FIELDS]);
        assert!(
            asked[0]
                .2
                .contains(&("GH_PROMPT_DISABLED".to_owned(), "1".to_owned()))
        );
    }

    #[test]
    fn a_state_or_a_conclusion_not_known_is_never_counted_as_passed() {
        let checks = checks_of(&[
            RawCheck {
                name: Some("new".to_owned()),
                status: Some("COMPLETED".to_owned()),
                conclusion: Some("SOMETHING_NEW".to_owned()),
                ..RawCheck::default()
            },
            RawCheck {
                context: Some("ci/legacy".to_owned()),
                state: Some("EXPECTED".to_owned()),
                ..RawCheck::default()
            },
            RawCheck {
                context: Some("ci/ok".to_owned()),
                state: Some("SUCCESS".to_owned()),
                ..RawCheck::default()
            },
        ]);
        assert_eq!(checks.passed, 1);
        assert_eq!(checks.running, ["new", "ci/legacy"]);
    }

    #[test]
    fn without_gh_or_signed_out_or_refused_it_says_which_and_a_bad_address_runs_nothing() {
        let missing = gh(Err(io::Error::new(io::ErrorKind::NotFound, "no gh")));
        assert_eq!(view(&missing, URL), Err(Problem::Missing));
        assert!(!here(&missing));
        let out = gh(said(
            false,
            "",
            "To get started with GitHub CLI, please run:  gh auth login\n",
        ));
        assert!(matches!(view(&out, URL), Err(Problem::SignedOut(_))));
        let refused = gh(said(
            false,
            "",
            "GraphQL: Could not resolve to a PullRequest\n",
        ));
        assert_eq!(
            view(&refused, URL),
            Err(Problem::Failed(
                "GraphQL: Could not resolve to a PullRequest".to_owned()
            ))
        );
        let never = gh(said(true, "{}", ""));
        for bad in [
            "https://github.com/acme/api/pull/231 --web",
            "https://evil.com/acme/api/pull/231",
            "-R x",
        ] {
            assert!(view(&never, bad).is_err(), "{bad}");
        }
        assert!(never.asked.lock().unwrap().is_empty());
        let there = gh(said(true, "gh version 2.82.1\n", ""));
        assert!(here(&there));
    }
}
