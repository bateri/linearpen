//! The call `comment`: the words the person wrote, posted as a comment of one issue. What happens is
//! `linear/`'s and the session's; this reads what the page asks, checks it before anything is sent,
//! makes the id the comment is posted under, and tells the comment as Linear holds it.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issue::{CommentMessage, check_id, comment_of};
use super::pictures::{Attached, given, send, with_lines};
use super::{Failure, call};
use crate::linear::images;
use crate::linear::limits::BODY_MOST;
use crate::linear::{Error, NOT_POSTED};
use crate::session::Session;
use crate::state::draft;

/// The issue, by Linear's id or its identifier, and the words: exactly these two.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Write {
    issue: String,
    body: String,
    /// The files that go with it (up to ten), put in Linear's storage before the comment is posted.
    #[serde(default)]
    files: Vec<Attached>,
}

#[derive(Debug, Serialize)]
struct PostedMessage {
    comment: CommentMessage,
}

/// The issue, the words and the files, or why they are not one comment. The words are the
/// person's, as they were written: only a comment of no words at all and no file, or of too many
/// words, is not one.
fn read(params: Write) -> Result<(String, String, Vec<crate::pictures::Given>), Failure> {
    check_id(&params.issue)?;
    let pictures = given(&params.files)?;
    let length = params.body.chars().count();
    if (params.body.trim().is_empty() && pictures.is_empty()) || length > BODY_MOST {
        return Err(Failure::BadParams(format!(
            "body is the words of the comment, from one to {BODY_MOST} characters (empty only where there are files)"
        )));
    }
    // The text that is posted is the words and the lines of the files: it is held to the same bound
    // before any file is sent, not after.
    let names: Vec<&str> = pictures.iter().map(|one| one.name.as_str()).collect();
    if images::with_lines_most(&params.body, &names) > BODY_MOST {
        return Err(Failure::BadParams(format!(
            "body with the lines of its files is at most {BODY_MOST} characters"
        )));
    }
    Ok((params.issue, params.body, pictures))
}

/// The words of why a comment was not posted, for the person.
pub(super) fn said(error: &Error) -> String {
    match error {
        Error::Refused | Error::Unreachable(_) => error.to_string(),
        Error::Forbidden => "Linear did not post it: this key cannot comment".to_owned(),
        Error::Unsure => "Linear did not answer whole, so it may have posted it: look at the comments before writing it again"
            .to_owned(),
        Error::Failed(why) if why == NOT_POSTED => "Linear did not post it".to_owned(),
        Error::Failed(why) => format!(
            "Linear did not post it: {}",
            why.strip_prefix("Linear said: ").unwrap_or(why)
        ),
    }
}

/// What a call that wrote tells of a failure: a key Linear does not take is said as it is for every
/// call; Linear not posting the comment, and a network that failed after it was sent, are
/// `comment-failed`, the second saying that it may have been posted.
fn told(error: Error) -> Failure {
    match error {
        Error::Refused => Failure::Linear(error),
        Error::Unreachable(_) => Failure::CommentFailed(format!(
            "{}; it may have been posted: look at the comments before writing it again",
            error
        )),
        other => Failure::CommentFailed(said(&other)),
    }
}

fn comment(session: &Session, params: Write) -> Result<PostedMessage, Failure> {
    let (issue, body, pictures) = read(params)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    // The files are put in Linear's storage first: where one cannot be, nothing is posted.
    let lines = send(session, &pictures, "posted")?;
    let body = if lines.is_empty() {
        body
    } else {
        with_lines(&body, &lines)
    };
    let id = draft::new_id().map_err(|why| Failure::Linear(Error::Failed(why)))?;
    let posted = session.comment(&id, &issue, &body).map_err(told)?;
    Ok(PostedMessage {
        comment: comment_of(&posted).0,
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "comment", comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(params: serde_json::Value) -> Result<(String, String), String> {
        let write: Write = super::super::params::parse(params)?;
        read(write)
            .map(|(issue, body, _)| (issue, body))
            .map_err(|failure| format!("{failure:?}"))
    }

    #[test]
    fn the_issue_and_the_words_are_all_the_call_takes_and_the_words_go_as_they_were_written() {
        let (issue, body) =
            write(json!({"issue": "issue-412", "body": "  Two\n\nparagraphs \n"})).unwrap();
        assert_eq!(issue, "issue-412");
        assert_eq!(body, "  Two\n\nparagraphs \n");
        let bad = [
            json!({"issue": "issue-412"}),
            json!({"body": "x"}),
            json!({"issue": "issue-412", "body": ""}),
            json!({"issue": "issue-412", "body": " \n\t "}),
            json!({"issue": "", "body": "x"}),
            json!({"issue": "issue 412; drop", "body": "x"}),
            json!({"issue": "issue-412", "body": 12}),
            // What the call does not take: a parent to reply under, an author to pose as, an id.
            json!({"issue": "issue-412", "body": "x", "parentId": "c-1"}),
            json!({"issue": "issue-412", "body": "x", "createAsUser": "Ada"}),
            json!({"issue": "issue-412", "body": "x", "id": "11111111-2222-4333-8444-555555555555"}),
        ];
        for params in bad {
            assert!(write(params.clone()).is_err(), "{params}");
        }
    }

    #[test]
    fn ten_thousand_characters_is_the_most_and_they_are_counted_as_characters() {
        let most = "\u{e7}".repeat(BODY_MOST);
        assert!(write(json!({"issue": "i", "body": most})).is_ok());
        let more = "\u{e7}".repeat(BODY_MOST + 1);
        assert!(write(json!({"issue": "i", "body": more})).is_err());
    }

    #[test]
    fn a_comment_that_would_be_too_long_with_the_lines_of_its_files_is_refused_before_they_are_sent()
     {
        let file = |name: &str| json!({"handle": "ab12", "name": name});
        let with = |words: &str, files: Vec<serde_json::Value>| {
            write(json!({"issue": "i", "body": words, "files": files}))
        };
        // The words alone are within the most; their files' lines are what would take it over.
        let nearly = "x".repeat(BODY_MOST - 100);
        assert!(with(&nearly, vec![]).is_ok());
        assert!(with(&nearly, vec![file("shot.png")]).is_err());
        // With room it goes, and a name that is long is cut to what a line takes.
        let room = "x".repeat(BODY_MOST - 1000);
        assert!(with(&room, vec![file("shot.png")]).is_ok());
        assert!(with(&room, vec![file(&"n".repeat(255))]).is_ok());
        // Ten files are ten lines.
        assert!(with(&room, (0..10).map(|_| file("shot.png")).collect()).is_err());
        // Files and no words are the lines alone.
        assert!(with("", vec![file("shot.png")]).is_ok());
    }

    #[test]
    fn linear_not_posting_is_comment_failed_and_a_key_it_does_not_take_is_refused() {
        let name = |failure: Failure| match failure {
            Failure::CommentFailed(words) => words,
            Failure::Linear(error) => error.kind().to_owned(),
            other => format!("{other:?}"),
        };
        assert_eq!(
            name(told(Error::Failed("Linear said: no such issue".into()))),
            "Linear did not post it: no such issue"
        );
        assert_eq!(
            name(told(Error::Failed(NOT_POSTED.into()))),
            "Linear did not post it"
        );
        assert_eq!(name(told(Error::Refused)), "refused");
        assert!(name(told(Error::Forbidden)).contains("cannot comment"));
        // An answer that did not come, or cannot be read, may have been a comment posted: it says so.
        assert!(name(told(Error::Unsure)).contains("may have posted it"));
        assert!(
            name(told(Error::Unreachable("timed out".into()))).contains("may have been posted")
        );
    }
}
