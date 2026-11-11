//! The call `issue`: one issue with what a row does not carry, its description and its comments.
//! They are asked of Linear when the issue is opened, and kept for the run.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issues::LabelMessage;
use super::{Failure, call};
use crate::linear::Issue;
use crate::model::time::short_date;
use crate::session::Session;

/// The longest id taken (Linear's are UUIDs, 36 characters; an identifier is shorter).
const ID_MOST: usize = 64;

/// The most characters of a description and of a comment that are told. The rest is in Linear.
pub const DESCRIPTION_MOST: usize = 20_000;
pub const COMMENT_MOST: usize = 2_000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Lookup {
    id: String,
}

#[derive(Debug, Serialize)]
struct StateMessage {
    name: String,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct CommentMessage {
    author: String,
    at: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct IssueMessage {
    id: String,
    identifier: String,
    title: String,
    /// Where the issue is in Linear: the page opens it there.
    url: String,
    description: Option<String>,
    priority: &'static str,
    state: StateMessage,
    labels: Vec<LabelMessage>,
    cycle: Option<u32>,
    project: Option<String>,
    /// "you", a name, or nothing.
    assignee: Option<String>,
    comments: Vec<CommentMessage>,
    /// There is more in Linear than is told here: comments past the first fifty, or text cut.
    more: bool,
}

/// `text` cut to `most` characters, and whether it was.
fn cut(text: &str, most: usize) -> (String, bool) {
    match text.char_indices().nth(most) {
        Some((end, _)) => (text[..end].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

fn message(issue: &Issue) -> IssueMessage {
    let mut more = false;
    let description = issue.description.as_deref().map(|text| {
        let (text, was_cut) = cut(text, DESCRIPTION_MOST);
        more |= was_cut;
        text
    });
    let mut comments: Vec<_> = issue.comments.iter().flat_map(|c| &c.nodes).collect();
    // The oldest first, as a conversation reads (the stamps sort as text).
    comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let comments = comments
        .into_iter()
        .map(|comment| {
            let (body, was_cut) = cut(&comment.body, COMMENT_MOST);
            more |= was_cut;
            CommentMessage {
                author: comment
                    .user
                    .as_ref()
                    .map_or_else(|| "Someone".to_owned(), |user| user.display_name.clone()),
                at: short_date(&comment.created_at),
                body,
            }
        })
        .collect();
    more |= issue
        .comments
        .as_ref()
        .is_some_and(|comments| comments.page_info.has_next_page);
    IssueMessage {
        id: issue.id.clone(),
        identifier: issue.identifier.clone(),
        title: issue.title.clone(),
        url: issue.url.clone(),
        description,
        priority: issue.priority.name(),
        state: StateMessage {
            name: issue.state.name.clone(),
            kind: issue.state.kind.name(),
        },
        labels: issue
            .labels
            .iter()
            .map(|label| LabelMessage {
                name: label.name.clone(),
                color: label.color.clone(),
            })
            .collect(),
        cycle: issue.cycle,
        project: issue.project.clone(),
        assignee: issue.assignee.as_ref().map(|who| {
            if who.is_me {
                "you".to_owned()
            } else {
                who.display_name.clone()
            }
        }),
        comments,
        more,
    }
}

/// Whether `id` is Linear's id of an issue or its identifier.
pub(super) fn check_id(id: &str) -> Result<(), Failure> {
    if id.is_empty()
        || id.chars().count() > ID_MOST
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Failure::BadParams(format!(
            "id is Linear's id of an issue or its identifier, up to {ID_MOST} letters, digits and dashes"
        )));
    }
    Ok(())
}

fn issue(session: &Session, params: Lookup) -> Result<IssueMessage, Failure> {
    let id = params.id;
    check_id(&id)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    session
        .detail(&id)
        .map(|found| message(&found))
        .map_err(Failure::Linear)
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "issue", issue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_is_cut_on_a_character_and_says_so() {
        assert_eq!(cut("abc", 5), ("abc".to_owned(), false));
        assert_eq!(cut("abcdef", 3), ("abc".to_owned(), true));
        // A letter of two bytes is not cut in the middle.
        assert_eq!(cut("çççç", 2), ("çç".to_owned(), true));
        assert_eq!(cut("abc", 3), ("abc".to_owned(), false));
    }
}
