//! The call `issue`: one issue with what a row does not carry, its description and its comments.
//! They are asked of Linear when the issue is opened, and kept for the run.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issues::{Ids, LabelMessage, ids, labels};
use super::{Failure, call};
use crate::linear::types::{Comment, Neighbor};
use crate::linear::{Issue, plain};
use crate::model::time::short_date;
use crate::session::Session;

/// The longest id taken (Linear's are UUIDs, 36 characters; an identifier is shorter).
const ID_MOST: usize = 64;

/// The most characters of a description and of a comment that are told. The rest is in Linear.
pub const DESCRIPTION_MOST: usize = 20_000;
pub const COMMENT_MOST: usize = 2_000;
/// The most characters of the title of a related issue, and the most issues told of each kind of
/// surroundings (the sub-issues, and the ties together): Linear is asked for no more than this.
pub const NEIGHBOR_TITLE_MOST: usize = 300;
pub const NEIGHBORS_MOST: usize = 50;

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

/// An issue the open one is tied to: what names it and what opens it.
#[derive(Debug, Serialize)]
struct NeighborMessage {
    id: String,
    identifier: String,
    title: String,
    state: StateMessage,
}

/// How the open issue is tied to another (`Blocked by`), and that other.
#[derive(Debug, Serialize)]
struct TieMessage {
    label: &'static str,
    issue: NeighborMessage,
}

#[derive(Debug, Serialize)]
pub(super) struct CommentMessage {
    author: String,
    at: String,
    body: String,
}

/// A comment as the detail tells it, and whether its words were cut.
pub(super) fn comment_of(comment: &Comment) -> (CommentMessage, bool) {
    let (body, was_cut) = cut(&comment.body, COMMENT_MOST);
    (
        CommentMessage {
            author: comment
                .user
                .as_ref()
                .map_or_else(|| "Someone".to_owned(), |user| user.display_name.clone()),
            at: short_date(&comment.created_at),
            body,
        },
        was_cut,
    )
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IssueMessage {
    id: String,
    identifier: String,
    title: String,
    /// Where the issue is in Linear: the page opens it there.
    url: String,
    /// The branch Linear suggests for it, where it has one: the page copies it.
    branch_name: Option<String>,
    description: Option<String>,
    priority: &'static str,
    state: StateMessage,
    labels: Vec<LabelMessage>,
    /// The cycle's name, or `Cycle 24`.
    cycle: Option<String>,
    project: Option<String>,
    /// "you", a name, or nothing.
    assignee: Option<String>,
    #[serde(flatten)]
    ids: Ids,
    /// The issue it is a part of, its parts and what it is tied to (`Blocks`, `Blocked by`, ...).
    parent: Option<NeighborMessage>,
    children: Vec<NeighborMessage>,
    ties: Vec<TieMessage>,
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

fn neighbor(other: &Neighbor, more: &mut bool) -> NeighborMessage {
    let (title, was_cut) = cut(&other.title, NEIGHBOR_TITLE_MOST);
    *more |= was_cut;
    NeighborMessage {
        id: other.id.clone(),
        identifier: other.identifier.clone(),
        title,
        state: StateMessage {
            name: other.state.name.clone(),
            kind: other.state.kind.name(),
        },
    }
}

fn message(issue: &Issue) -> IssueMessage {
    let mut more = false;
    let parent = issue
        .parent
        .as_ref()
        .map(|other| neighbor(other, &mut more));
    // Linear was asked for the first fifty of each: as many as that may be all or may be a part.
    more |= issue.children.len() >= NEIGHBORS_MOST
        || issue.relations.len() >= NEIGHBORS_MOST
        || issue.inverse_relations.len() >= NEIGHBORS_MOST;
    let children = issue
        .children
        .iter()
        .take(NEIGHBORS_MOST)
        .map(|other| neighbor(other, &mut more))
        .collect();
    let all_ties = issue.ties();
    more |= all_ties.len() > NEIGHBORS_MOST;
    let ties = all_ties
        .into_iter()
        .take(NEIGHBORS_MOST)
        .map(|(tie, other)| TieMessage {
            label: tie.label(),
            issue: neighbor(other, &mut more),
        })
        .collect();
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
            let (told, was_cut) = comment_of(comment);
            more |= was_cut;
            told
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
        url: plain(&issue.url),
        branch_name: super::issues::branch_name(issue),
        description,
        priority: issue.priority.name(),
        state: StateMessage {
            name: issue.state.name.clone(),
            kind: issue.state.kind.name(),
        },
        labels: labels(issue),
        cycle: issue
            .cycle
            .as_ref()
            .map(crate::linear::types::CycleRef::label),
        project: issue.project.as_ref().map(|project| project.name.clone()),
        assignee: issue.assignee.as_ref().map(|who| {
            if who.is_me {
                "you".to_owned()
            } else {
                who.display_name.clone()
            }
        }),
        ids: ids(issue),
        parent,
        children,
        ties,
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
            "id is Linear's id of an issue or its identifier, up to {ID_MOST} letters, digits, dashes and underscores"
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
    use crate::linear::types::fixture_issue;

    #[test]
    fn the_issue_tells_the_ids_of_what_it_is_of_and_in() {
        let value = serde_json::to_value(message(&fixture_issue())).unwrap();
        assert_eq!(value["teamId"], "team-acm");
        assert_eq!(value["stateId"], "s1");
        assert_eq!(value["assigneeId"], "u-ada");
        assert!(value["projectId"].is_null());
        assert_eq!(value["labels"][0]["id"], "l-api");
        assert_eq!(value["assignee"], "you");
    }

    fn neighbor(n: u32, title: &str) -> crate::linear::types::Neighbor {
        serde_json::from_value(serde_json::json!({
            "id": format!("issue-{n}"), "identifier": format!("ACM-{n}"), "title": title,
            "state": {"id": "s", "name": "Todo", "type": "unstarted", "position": 1}
        }))
        .unwrap()
    }

    #[test]
    fn the_issue_tells_its_cycle_its_estimate_and_its_due_date() {
        let mut issue = fixture_issue();
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert_eq!(value["cycleId"], "c-12");
        assert_eq!(value["cycle"], "Cycle 12");
        assert!(value["estimate"].is_null() && value["dueDate"].is_null());
        issue.estimate = Some(5);
        issue.due_date = Some("2026-11-15".into());
        issue.cycle.as_mut().unwrap().name = Some("Spring".into());
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert_eq!(value["estimate"], 5);
        assert_eq!(value["dueDate"], "2026-11-15");
        assert_eq!(value["cycle"], "Spring");
    }

    #[test]
    fn the_surroundings_are_told_with_how_they_are_tied_and_none_is_an_empty_list() {
        let mut issue = fixture_issue();
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert!(value["parent"].is_null());
        assert_eq!(value["children"], serde_json::json!([]));
        assert_eq!(value["ties"], serde_json::json!([]));
        issue.parent = Some(neighbor(400, "Billing limits per plan"));
        issue.children = vec![neighbor(1, "Part one")];
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert_eq!(value["parent"]["identifier"], "ACM-400");
        assert_eq!(value["parent"]["id"], "issue-400");
        assert_eq!(value["parent"]["title"], "Billing limits per plan");
        assert_eq!(value["parent"]["state"]["kind"], "unstarted");
        assert_eq!(value["children"][0]["identifier"], "ACM-1");
    }

    #[test]
    fn a_tie_is_told_by_its_words_and_what_is_too_long_or_too_many_is_cut_and_says_so() {
        let mut issue = fixture_issue();
        issue.relations = vec![crate::linear::types::Relation {
            kind: "blocks".into(),
            issue: None,
            related_issue: Some(neighbor(7, &"x".repeat(NEIGHBOR_TITLE_MOST + 5))),
        }];
        issue.inverse_relations = vec![crate::linear::types::Relation {
            kind: "duplicate".into(),
            issue: Some(neighbor(8, "Same thing")),
            related_issue: None,
        }];
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert_eq!(value["ties"][0]["label"], "Blocks");
        assert_eq!(value["ties"][0]["issue"]["identifier"], "ACM-7");
        assert_eq!(
            value["ties"][0]["issue"]["title"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            NEIGHBOR_TITLE_MOST
        );
        assert_eq!(value["ties"][1]["label"], "Duplicated by");
        assert_eq!(value["more"], true);
        issue.relations.clear();
        issue.inverse_relations.clear();
        issue.children = (0..NEIGHBORS_MOST as u32 + 3)
            .map(|n| neighbor(n, "Part"))
            .collect();
        let value = serde_json::to_value(message(&issue)).unwrap();
        assert_eq!(value["children"].as_array().unwrap().len(), NEIGHBORS_MOST);
        assert_eq!(value["more"], true);
    }

    #[test]
    fn a_text_is_cut_on_a_character_and_says_so() {
        assert_eq!(cut("abc", 5), ("abc".to_owned(), false));
        assert_eq!(cut("abcdef", 3), ("abc".to_owned(), true));
        // A letter of two bytes is not cut in the middle.
        assert_eq!(cut("çççç", 2), ("çç".to_owned(), true));
        assert_eq!(cut("abc", 3), ("abc".to_owned(), false));
    }
}
