//! The calls `edit-open` and `edit-save`: the words of an issue's description, or of a comment of the
//! person's, edited in the pane. `edit-open` gives the text whole as Linear holds it now (what the page
//! shows of it is cut, and is never what is edited); `edit-save` writes the person's words over it,
//! where Linear holds it still. What happens is `linear/edit.rs`'s and the session's; this reads what
//! the page asks, checks it before anything is sent, and says what came of it.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issue::{IssueMessage, check_id, message};
use super::pictures::{Attached, given, send};
use super::{Failure, call};
use crate::linear::edit::{NOT_EDITED, NOT_HERE, NOT_YOURS, Step, Target};
use crate::linear::images;
use crate::linear::limits::{EDIT_MOST, is_ref};
use crate::linear::{Error, SAID};
use crate::session::{NOT_BEGUN, Session};

/// What is edited: the description of the issue, or the comment of it named.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    issue: String,
    #[serde(default)]
    comment: Option<String>,
}

#[derive(Debug, Serialize)]
struct Opened {
    text: String,
}

/// The same, and the words to write over it, and (for a description) the files that go at their end.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    issue: String,
    #[serde(default)]
    comment: Option<String>,
    text: String,
    #[serde(default)]
    files: Vec<Attached>,
}

#[derive(Debug, Serialize)]
struct Saved {
    /// The issue as Linear holds it now (as `issue` tells it), or none where it could not be asked for:
    /// the page asks for it again.
    issue: Option<IssueMessage>,
}

/// The issue and what of it is edited, or why they are not.
fn target(issue: &str, comment: Option<&str>) -> Result<Target, Failure> {
    check_id(issue)?;
    match comment {
        None => Ok(Target::Description),
        Some(id) if is_ref(id) => Ok(Target::Comment(id.to_owned())),
        Some(_) => Err(Failure::BadParams(
            "comment is Linear's id of a comment: letters, digits, dashes and underscores"
                .to_owned(),
        )),
    }
}

fn no_key(session: &Session) -> Result<(), Failure> {
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    Ok(())
}

/// What an edit is told where what is edited could not be read for it: a comment that is not the
/// person's, or not on the issue, is said as such; any other failure of Linear's is as for a read.
fn unread(error: Error) -> Failure {
    match error {
        Error::Failed(why) if why == NOT_YOURS => Failure::EditFailed(
            "This comment is not yours: Linear lets only the one who wrote it edit it".to_owned(),
        ),
        Error::Failed(why) if why == NOT_HERE => Failure::EditFailed(why),
        other => Failure::Linear(other),
    }
}

fn open(session: &Session, params: Open) -> Result<Opened, Failure> {
    let target = target(&params.issue, params.comment.as_deref())?;
    no_key(session)?;
    let text = session.edit_open(&params.issue, &target).map_err(unread)?;
    if text.chars().count() > EDIT_MOST {
        return Err(Failure::EditFailed(format!(
            "It is over {EDIT_MOST} characters: edit it in Linear"
        )));
    }
    Ok(Opened { text })
}

/// The words of what came of a save that did not write, for the person.
fn told(step: Step) -> Failure {
    match step {
        Step::Read(Error::Refused) | Step::Write(Error::Refused) => {
            Failure::Linear(Error::Refused)
        }
        Step::Read(Error::Failed(why)) if why == NOT_BEGUN => Failure::EditFailed(
            "This edit was not begun here: Cancel, then edit it again".to_owned(),
        ),
        Step::Read(Error::Failed(why)) if why == NOT_YOURS || why == NOT_HERE => {
            unread(Error::Failed(why))
        }
        Step::Read(error) => Failure::EditFailed(format!(
            "It could not be read again from Linear, so nothing was saved ({error})"
        )),
        Step::Changed(_) => Failure::EditFailed(
            "Someone changed it in Linear while you were editing, so nothing was saved: Save again to put yours in its place, or Cancel to see theirs"
                .to_owned(),
        ),
        Step::Write(error) => match error {
            Error::Refused => Failure::Linear(error),
            Error::Forbidden => {
                Failure::EditFailed("Linear did not save it: this key cannot edit".to_owned())
            }
            Error::Unreachable(_) => Failure::EditFailed(format!(
                "{error}; it may have been saved: Save again to make sure"
            )),
            Error::Unsure => Failure::EditFailed(
                "Linear did not answer whole, so it may have saved it: Save again to make sure"
                    .to_owned(),
            ),
            Error::Failed(why) if why == NOT_EDITED => {
                Failure::EditFailed("Linear did not save it".to_owned())
            }
            Error::Failed(why) => Failure::EditFailed(format!(
                "Linear did not save it: {}",
                why.strip_prefix(SAID).unwrap_or(&why)
            )),
        },
    }
}

/// The words and the files, or why they are not an edit: the words are the person's, as they were
/// written, up to the most an edit takes with the lines of the files; a comment keeps some words, and
/// takes no file (they go in a comment of their own).
fn checked(
    target: &Target,
    text: &str,
    files: &[Attached],
) -> Result<Vec<crate::pictures::Given>, Failure> {
    let pictures = given(files)?;
    if let Target::Comment(_) = target {
        if !pictures.is_empty() {
            return Err(Failure::BadParams(
                "files go with a description, not with a comment that is edited".to_owned(),
            ));
        }
        if text.trim().is_empty() {
            return Err(Failure::BadParams(
                "text is the words of the comment: a comment is not left empty".to_owned(),
            ));
        }
    }
    let names: Vec<&str> = pictures.iter().map(|one| one.name.as_str()).collect();
    if images::with_lines_most(text, &names) > EDIT_MOST {
        return Err(Failure::BadParams(format!(
            "text, with the lines of its files, is at most {EDIT_MOST} characters"
        )));
    }
    Ok(pictures)
}

fn save(session: &Session, params: Save) -> Result<Saved, Failure> {
    let target = target(&params.issue, params.comment.as_deref())?;
    let pictures = checked(&target, &params.text, &params.files)?;
    no_key(session)?;
    // The files are put in Linear's storage first: where one cannot be, nothing is written.
    let lines = send(session, &pictures, "saved")?;
    let text = if lines.is_empty() {
        params.text
    } else {
        images::description_with(&params.text, &lines)
    };
    let issue = session
        .edit_save(&params.issue, &target, &text)
        .map_err(told)?;
    Ok(Saved {
        issue: issue.as_ref().map(message),
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "edit-open", open);
    call(options, session, "edit-save", save)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse<P: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<P, String> {
        super::super::params::parse(value)
    }

    fn kind(failure: Failure) -> String {
        match failure {
            Failure::EditFailed(words) => words,
            Failure::BadParams(words) => format!("bad-params: {words}"),
            Failure::Linear(error) => error.kind().to_owned(),
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn an_edit_names_the_issue_and_a_comment_of_it_or_none_and_nothing_else() {
        let open: Open = parse(json!({"issue": "issue-412"})).unwrap();
        assert_eq!(
            target(&open.issue, open.comment.as_deref()).unwrap(),
            Target::Description
        );
        let open: Open = parse(json!({"issue": "issue-412", "comment": "c-7"})).unwrap();
        assert_eq!(
            target(&open.issue, open.comment.as_deref()).unwrap(),
            Target::Comment("c-7".into())
        );
        assert!(parse::<Open>(json!({"comment": "c-7"})).is_err());
        assert!(parse::<Open>(json!({"issue": "i", "text": "x"})).is_err());
        assert!(target("issue 1; x", None).is_err());
        assert!(target("i", Some("c 7")).is_err());
        assert!(target("i", Some("")).is_err());
        // What a save does not take: a team, a title, another issue's description.
        assert!(parse::<Save>(json!({"issue": "i", "text": "x", "team": "t"})).is_err());
        assert!(parse::<Save>(json!({"issue": "i", "text": "x", "title": "t"})).is_err());
        assert!(parse::<Save>(json!({"issue": "i"})).is_err());
    }

    #[test]
    fn a_description_may_be_emptied_and_take_files_and_a_comment_keeps_words_and_takes_none() {
        let file = vec![
            serde_json::from_value::<Attached>(json!({"handle": "ab12", "name": "shot.png"}))
                .unwrap(),
        ];
        assert!(checked(&Target::Description, "", &[]).is_ok());
        assert!(checked(&Target::Description, "Words", &file).is_ok());
        let comment = Target::Comment("c-1".into());
        assert!(checked(&comment, "Fixed", &[]).is_ok());
        assert!(checked(&comment, " \n ", &[]).is_err());
        assert!(checked(&comment, "Fixed", &file).is_err());
        let most = "\u{e7}".repeat(EDIT_MOST);
        assert!(checked(&Target::Description, &most, &[]).is_ok());
        assert!(checked(&Target::Description, &format!("{most}x"), &[]).is_err());
        assert!(checked(&Target::Description, &most, &file).is_err());
    }

    #[test]
    fn what_came_of_a_save_is_said_for_the_person_and_a_key_linear_does_not_take_is_refused() {
        assert_eq!(kind(told(Step::Write(Error::Refused))), "refused");
        assert_eq!(kind(told(Step::Read(Error::Refused))), "refused");
        assert!(kind(told(Step::Changed("theirs".into()))).contains("Save again to put yours"));
        assert!(kind(told(Step::Write(Error::Unsure))).contains("may have saved it"));
        assert!(
            kind(told(Step::Write(Error::Unreachable("timed out".into()))))
                .contains("may have been saved")
        );
        assert_eq!(
            kind(told(Step::Write(Error::Failed(format!(
                "{SAID}no such comment"
            ))))),
            "Linear did not save it: no such comment"
        );
        assert_eq!(
            kind(told(Step::Write(Error::Failed(NOT_EDITED.into())))),
            "Linear did not save it"
        );
        assert!(kind(told(Step::Write(Error::Forbidden))).contains("cannot edit"));
        assert!(kind(told(Step::Read(Error::Failed(NOT_YOURS.into())))).contains("not yours"));
        assert!(kind(told(Step::Read(Error::Failed(NOT_BEGUN.into())))).contains("edit it again"));
        assert!(
            kind(told(Step::Read(Error::Unreachable("down".into())))).contains("nothing was saved")
        );
        assert!(kind(unread(Error::Failed(NOT_YOURS.into()))).contains("not yours"));
        assert_eq!(
            kind(unread(Error::Unreachable("down".into()))),
            "unreachable"
        );
    }
}
