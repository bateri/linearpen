//! The call of GitHub's: `pull`, what GitHub says of a pull request linked to the issue open.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::{Failure, call_stoppable};
use crate::github::{Problem, Pull};
use crate::linear::types::pull_request_number;
use crate::session::Session;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    url: String,
}

#[derive(Serialize)]
struct FileMessage {
    path: String,
    additions: u64,
    deletions: u64,
}

#[derive(Serialize)]
struct ChecksMessage {
    passed: usize,
    failed: Vec<String>,
    running: Vec<String>,
    skipped: usize,
}

#[derive(Serialize)]
struct ReviewedMessage {
    who: String,
    state: &'static str,
}

#[derive(Serialize)]
struct ReviewMessage {
    decision: Option<&'static str>,
    reviews: Vec<ReviewedMessage>,
    waiting: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PullMessage {
    number: u64,
    title: String,
    url: String,
    repository: String,
    state: &'static str,
    author: Option<String>,
    head: String,
    base: String,
    additions: u64,
    deletions: u64,
    changed_files: u64,
    files: Vec<FileMessage>,
    checks: ChecksMessage,
    review: ReviewMessage,
}

fn message(pull: Pull) -> PullMessage {
    PullMessage {
        number: pull.number,
        title: pull.title,
        url: pull.url,
        repository: pull.repository,
        state: pull.state,
        author: pull.author,
        head: pull.head,
        base: pull.base,
        additions: pull.additions,
        deletions: pull.deletions,
        changed_files: pull.changed_files,
        files: pull
            .files
            .into_iter()
            .map(|file| FileMessage {
                path: file.path,
                additions: file.additions,
                deletions: file.deletions,
            })
            .collect(),
        checks: ChecksMessage {
            passed: pull.checks.passed,
            failed: pull.checks.failed,
            running: pull.checks.running,
            skipped: pull.checks.skipped,
        },
        review: ReviewMessage {
            decision: pull.review.decision,
            reviews: pull
                .review
                .reviews
                .into_iter()
                .map(|(who, state)| ReviewedMessage { who, state })
                .collect(),
            waiting: pull.review.waiting,
        },
    }
}

fn pull(
    session: &Session,
    asked: Asked,
    stopped: &dyn Fn() -> bool,
) -> Result<PullMessage, Failure> {
    if pull_request_number(&asked.url).is_none() {
        return Err(Failure::BadParams(
            "url is not the address of a pull request of GitHub's".to_owned(),
        ));
    }
    match session.pull(&asked.url, stopped) {
        Ok(found) => Ok(message(found)),
        Err(Problem::Missing) => Err(Failure::Unavailable(
            "gh, GitHub's command, is not installed".to_owned(),
        )),
        Err(Problem::SignedOut(_)) => Err(Failure::Unavailable(
            "gh is not signed in to GitHub: run gh auth login".to_owned(),
        )),
        Err(Problem::Failed(words)) => Err(Failure::PullFailed(words)),
    }
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    // (Given up where the page lets the pull request go: `gh` is stopped.)
    call_stoppable(options, session, "pull", pull)
}
