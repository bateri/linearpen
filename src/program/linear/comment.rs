//! The comment: the third thing the pen writes. The words are the person's, sent as they were written
//! as a variable of `CommentCreate` (never in its text), under an id the program made, to one issue.
//! It is sent once, whatever comes back; what is done of an answer that did not come is
//! `Session::comment`'s. Nothing else rides with it: no parent, no author to pose as.

use serde::{Deserialize, Serialize};

use super::query;
use super::types::Comment;
use super::{Error, Key, Transport, run};

/// What `CommentCreate` is given: the comment's own id (a UUID version 4, so that an answer that never
/// came can be asked about), the issue it is on and its words.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input<'a> {
    pub id: &'a str,
    pub issue_id: &'a str,
    pub body: &'a str,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "commentCreate")]
    created: Option<Payload>,
}

#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    comment: Option<Comment>,
}

/// What `post` says where Linear answered with data and not with `success`: no.
pub const NOT_POSTED: &str = "Linear did not post the comment";

/// Posts `body` as a comment of the issue with Linear's id `issue` (or its identifier), under `id`, and
/// gives the comment as Linear says it is.
pub fn post(
    transport: &dyn Transport,
    key: &Key,
    id: &str,
    issue: &str,
    body: &str,
) -> Result<Comment, Error> {
    let data: Data = run(
        transport,
        key,
        &query::COMMENT_CREATE,
        Some(query::Commented {
            input: Input {
                id,
                issue_id: issue,
                body,
            },
        }),
    )?;
    data.created
        .filter(|payload| payload.success)
        .and_then(|payload| payload.comment)
        .ok_or_else(|| Error::Failed(NOT_POSTED.to_owned()))
}
