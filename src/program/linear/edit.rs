//! The edit: the words of an issue's description, or of a comment of the person's, written again as the
//! person changed them. The text an edit begins from is read whole from Linear (`current`): what the
//! page shows is cut, and is never what is written over. Before the words are written the text is read
//! again, and where it is not what the edit began from, someone changed it in Linear meanwhile and
//! nothing is written (`Step::Changed`, with what Linear holds now). A comment Linear does not say is
//! the person's is not edited. Each write is sent once, whatever comes back; what is done of an answer
//! that did not come is `Session::edit_save`'s.

use serde::{Deserialize, Serialize};

use super::types::Comment;
use super::{Error, Key, Transport, describe, fetch_issue, query, run};

/// What is edited: the description of the issue, or one of its comments, by Linear's id of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    Description,
    Comment(String),
}

/// The text of what is edited as Linear holds it now, and Linear's id of the issue it is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    pub issue: String,
    pub text: String,
}

/// What `current` says of a comment Linear does not say is the person's.
pub const NOT_YOURS: &str = "Linear lets only the one who wrote a comment edit it";
/// What it says of a comment that is on another issue than the one named.
pub const NOT_HERE: &str = "That comment is not on this issue";
/// What `save` says where Linear answered with data and not with `success`: no.
pub const NOT_EDITED: &str = "Linear did not edit the comment";

/// A comment as `Comment` (the question) tells it: the comment, and the issue it is on.
#[derive(Deserialize)]
struct Asked {
    #[serde(flatten)]
    comment: Comment,
    #[serde(default)]
    issue: Option<IssueOf>,
}

#[derive(Deserialize)]
struct IssueOf {
    id: String,
    identifier: String,
}

#[derive(Deserialize)]
struct CommentData {
    comment: Option<Asked>,
}

/// The variables of `CommentUpdate`: the comment, and its body and nothing else.
#[derive(Debug, Serialize)]
struct Update<'a> {
    id: &'a str,
    input: Body<'a>,
}

#[derive(Debug, Serialize)]
struct Body<'a> {
    body: &'a str,
}

#[derive(Deserialize)]
struct UpdateData {
    #[serde(rename = "commentUpdate")]
    updated: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    comment: Option<Comment>,
}

/// The comment with Linear's id `id`, whole, where it is the person's and is on the issue `issue`
/// (Linear's id of it, or its identifier).
pub fn comment_of(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    id: &str,
) -> Result<(Comment, String), Error> {
    let data: CommentData = run(transport, key, &query::COMMENT, Some(query::Lookup { id }))?;
    let asked = data
        .comment
        .ok_or_else(|| Error::Failed("Linear has no such comment".to_owned()))?;
    let Some(on) = asked.issue else {
        return Err(Error::Failed(NOT_HERE.to_owned()));
    };
    if !on.id.eq_ignore_ascii_case(issue) && !on.identifier.eq_ignore_ascii_case(issue) {
        return Err(Error::Failed(NOT_HERE.to_owned()));
    }
    if !asked.comment.mine() {
        return Err(Error::Failed(NOT_YOURS.to_owned()));
    }
    Ok((asked.comment, on.id))
}

/// What is edited, as Linear holds it now: the description of the issue `issue` (Linear's id of it, or
/// its identifier; none is an empty text), or the comment, where it is the person's.
pub fn current(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    target: &Target,
) -> Result<Current, Error> {
    match target {
        Target::Description => {
            let read = fetch_issue(transport, key, issue)?;
            Ok(Current {
                text: read.description.unwrap_or_default(),
                issue: read.id,
            })
        }
        Target::Comment(id) => {
            let (comment, issue) = comment_of(transport, key, issue, id)?;
            Ok(Current {
                issue,
                text: comment.body,
            })
        }
    }
}

/// Where `save` stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing was sent: what is edited could not be read again.
    Read(Error),
    /// Nothing was sent: Linear holds another text than the edit began from (here), so someone changed
    /// it meanwhile.
    Changed(String),
    /// The write was sent, and what came back is `Error`.
    Write(Error),
}

/// Whether two texts are the same words: Linear keeps a text with no blank at its end.
fn same(a: &str, b: &str) -> bool {
    a.trim_end() == b.trim_end()
}

/// Writes `text` over what is edited, where Linear holds `base` (what the edit began from) now. Where it
/// holds `text` already (a write before whose answer did not come had made it) nothing is sent.
pub fn save(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    target: &Target,
    base: &str,
    text: &str,
) -> Result<(), Step> {
    let now = current(transport, key, issue, target).map_err(Step::Read)?;
    if same(&now.text, text) {
        return Ok(());
    }
    if now.text != base {
        return Err(Step::Changed(now.text));
    }
    write(transport, key, &now.issue, target, text)
}

/// Writes `text` over what is edited, whatever Linear holds: the command line's edit, which is given the
/// words whole and began from nothing it read. A comment is the person's still, as for `save`, and a text
/// Linear holds already is not sent.
pub fn overwrite(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    target: &Target,
    text: &str,
) -> Result<(), Step> {
    let now = current(transport, key, issue, target).map_err(Step::Read)?;
    if same(&now.text, text) {
        return Ok(());
    }
    write(transport, key, &now.issue, target, text)
}

/// Sends `text` as it is, and alone: the description as the one member of an `IssueBatchUpdate` of the
/// one issue (Linear's id `issue`), a comment's as the body of a `CommentUpdate` of that comment.
fn write(
    transport: &dyn Transport,
    key: &Key,
    issue: &str,
    target: &Target,
    text: &str,
) -> Result<(), Step> {
    match target {
        Target::Description => describe::write(transport, key, issue, text)
            .map(|_| ())
            .map_err(Step::Write),
        Target::Comment(id) => {
            let data: UpdateData = run(
                transport,
                key,
                &query::COMMENT_UPDATE,
                Some(Update {
                    id,
                    input: Body { body: text },
                }),
            )
            .map_err(Step::Write)?;
            data.updated
                .filter(|payload| payload.success && payload.comment.is_some())
                .map(|_| ())
                .ok_or_else(|| Step::Write(Error::Failed(NOT_EDITED.to_owned())))
        }
    }
}
