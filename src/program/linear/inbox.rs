//! The person's inbox, as Linear's Inbox shows it: one notification for each thing that had activity,
//! of which the ones about an issue are the rows of a list, each with a note of what it is about and
//! whether it was read. Read only: nothing here marks anything read.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::{Issue, Viewer};
use super::{Error, Fetched, Key, Listing, Transport, query, run};

/// How many of the inbox's notifications are asked for.
pub const MOST: u32 = 50;

/// What an issue of the inbox carries beside it: who caused its notification, what it is about, whether
/// the person has not read it, and when it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub who: Option<String>,
    pub what: &'static str,
    pub unread: bool,
    /// Linear's stamp of the notification (`2026-11-09T14:20:00.000Z`).
    pub at: String,
}

#[derive(Serialize)]
struct Variables {
    first: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    viewer: Viewer,
    inbox_notifications: Notifications,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Notifications {
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    #[serde(default)]
    has_next_page: bool,
}

#[derive(Deserialize)]
struct Named {
    #[serde(default, alias = "displayName")]
    name: Option<String>,
}

/// A notification as Linear tells it. Every member but the shared ones may be missing (a node of
/// another kind than an issue's), and the type and the category are read as words, so that a kind
/// Linear adds later is a note of its own words and not a refused answer.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Node {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    read_at: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    actor: Option<Named>,
    #[serde(default)]
    bot_actor: Option<Named>,
    #[serde(default)]
    external_user_actor: Option<Named>,
    /// The issue, kept as it came: one Linear tells in a shape that is not an issue's is let go of,
    /// and the rest of the inbox is read.
    #[serde(default)]
    issue: Option<serde_json::Value>,
}

/// The words of what a notification is about: by its type where Linear documents it, else by its
/// category, else `Update`. Each is true of the whole of what it names.
#[must_use]
pub fn what(kind: &str, category: &str) -> &'static str {
    match kind {
        "issueCreated" => return "Created",
        "issueReopened" => return "Reopened",
        "issueDue" => return "Due",
        "issueBlocking" => return "Blocking",
        "issueUnblocked" => return "Unblocked",
        "issuePriorityUrgent" => return "Urgent",
        "issueSlaBreached" => return "SLA breached",
        "issueSlaHighRisk" => return "SLA at risk",
        "issueThreadResolved" => return "Resolved",
        "issueAddedToTriage" | "triageResponsibilityIssueAddedToTriage" => return "Triage",
        _ => {}
    }
    match category {
        "mentions" => "Mention",
        "assignments" => "Assignee",
        "commentsAndReplies" => "Comment",
        "statusChanges" => "Status",
        "reactions" => "Reaction",
        "triage" => "Triage",
        "reminders" => "Reminder",
        _ => "Update",
    }
}

/// The name of whoever caused `node`: a person, a bot, someone of an integration; none where Linear
/// says no one.
fn who(node: &Node) -> Option<String> {
    [&node.actor, &node.bot_actor, &node.external_user_actor]
        .into_iter()
        .flatten()
        .filter_map(|named| named.name.as_deref())
        .map(super::plain)
        .find(|name| !name.is_empty())
}

/// The issues of the inbox and their notes, one for each issue in the order Linear tells them: a second
/// notification of an issue is one with the first (unread where either is, of the later moment), and
/// one that is not about an issue is not there. Whether Linear has more than were asked for.
fn issues_of(nodes: Vec<Node>) -> (Vec<Issue>, HashMap<String, Note>) {
    let mut issues: Vec<Issue> = Vec::new();
    let mut notes: HashMap<String, Note> = HashMap::new();
    for node in nodes {
        let Some(issue) = node
            .issue
            .clone()
            .and_then(|value| serde_json::from_value::<Issue>(value).ok())
        else {
            continue;
        };
        let note = Note {
            who: who(&node),
            what: what(&node.kind, &node.category),
            unread: node.read_at.is_none(),
            at: node.created_at.clone(),
        };
        match notes.get_mut(&issue.id) {
            Some(held) => {
                held.unread |= note.unread;
                if note.at > held.at {
                    *held = Note {
                        unread: held.unread,
                        ..note
                    };
                }
            }
            None => {
                notes.insert(issue.id.clone(), note);
                issues.push(issue);
            }
        }
    }
    (issues, notes)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkRead<'a> {
    input: Entity<'a>,
    read_at: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entity<'a> {
    issue_id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marked {
    notification_mark_read_all: Success,
}

#[derive(Deserialize)]
struct Success {
    success: bool,
}

/// Marks every notification of the issue `issue_id` (Linear's id of it) read in the person's inbox, at
/// `read_at` (`2026-11-24T20:00:00.000Z`). Sent once.
///
/// # Errors
///
/// As every question to Linear; `Error::Failed` where Linear answered that it did not.
pub fn mark_read(
    transport: &dyn Transport,
    key: &Key,
    issue_id: &str,
    read_at: &str,
) -> Result<(), Error> {
    let marked: Marked = run(
        transport,
        key,
        &query::MARK_READ,
        Some(MarkRead {
            input: Entity { issue_id },
            read_at,
        }),
    )?;
    if marked.notification_mark_read_all.success {
        Ok(())
    } else {
        Err(Error::Failed("Linear did not mark it read".to_owned()))
    }
}

/// A moment as Linear writes one, `2026-11-24T20:00:00.000Z`, from seconds since the epoch.
#[must_use]
pub fn moment(seconds: u64) -> String {
    // (`20261124T200000Z`, the work folder's stamp, in the parts Linear reads.)
    let stamp = crate::work::handoff::stamp(seconds);
    format!(
        "{}-{}-{}T{}:{}:{}.000Z",
        &stamp[0..4],
        &stamp[4..6],
        &stamp[6..8],
        &stamp[9..11],
        &stamp[11..13],
        &stamp[13..15]
    )
}

/// The person's inbox: its issues, each with its note.
///
/// # Errors
///
/// As every question to Linear.
pub fn fetch(transport: &dyn Transport, key: &Key) -> Result<Fetched, Error> {
    let data: Data = run(
        transport,
        key,
        &query::INBOX,
        Some(Variables { first: MOST }),
    )?;
    let more = data
        .inbox_notifications
        .page_info
        .is_some_and(|info| info.has_next_page);
    let (issues, notes) = issues_of(data.inbox_notifications.nodes);
    Ok(Fetched {
        viewer: data.viewer,
        listing: Listing {
            issues,
            truncated: more,
            notes,
            projects: Vec::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(value: serde_json::Value) -> Node {
        serde_json::from_value(value).unwrap()
    }

    fn issue(id: &str) -> serde_json::Value {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../test/stand-in/issue.json")).unwrap();
        let mut value = fixture["data"]["issue"].clone();
        value["id"] = id.into();
        value["identifier"] = format!("ACM-{}", id.len()).into();
        value
    }

    #[test]
    fn a_moment_is_written_as_linear_reads_one() {
        assert_eq!(moment(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(moment(1_795_550_400), "2026-11-24T20:00:00.000Z");
    }

    #[test]
    fn what_a_note_says_is_true_of_all_it_names_and_a_new_kind_is_an_update() {
        assert_eq!(what("issueCreated", "subscriptions"), "Created");
        assert_eq!(what("issueAssignedToYou", "assignments"), "Assignee");
        assert_eq!(what("issueUnassignedFromYou", "assignments"), "Assignee");
        assert_eq!(what("issueMention", "mentions"), "Mention");
        assert_eq!(what("issueNewComment", "commentsAndReplies"), "Comment");
        assert_eq!(what("somethingNew", "somethingNewer"), "Update");
        assert_eq!(what("", ""), "Update");
    }

    #[test]
    fn one_row_for_each_issue_unread_where_either_is_and_what_is_not_an_issues_is_not_there() {
        let nodes = vec![
            node(serde_json::json!({
                "type": "issueNewComment", "category": "commentsAndReplies", "readAt": "2026-11-10T00:00:00.000Z",
                "createdAt": "2026-11-10T00:00:00.000Z", "actor": { "displayName": "Deniz" },
                "issue": issue("a") })),
            // The same issue again, later and unread: one row, unread, of the later moment.
            node(serde_json::json!({
                "type": "issueMention", "category": "mentions", "readAt": null,
                "createdAt": "2026-11-11T00:00:00.000Z", "actor": null, "botActor": { "name": "Codex" },
                "issue": issue("a") })),
            // A pull request's and a project's: not about an issue.
            node(serde_json::json!({
                "type": "pullRequestReviewRequested", "category": "reviews", "createdAt": "2026-11-11T00:00:00.000Z" })),
            node(serde_json::json!({
                "type": "projectUpdateCreated", "category": "postsAndUpdates", "createdAt": "2026-11-11T00:00:00.000Z" })),
            // An issue in a shape that is not an issue's is let go of.
            node(
                serde_json::json!({ "type": "issueCreated", "category": "x", "createdAt": "2026-11-09T00:00:00.000Z",
                "issue": { "id": "broken" } }),
            ),
            // A kind Linear adds later, read, with no one who caused it.
            node(serde_json::json!({
                "type": "issueSomethingNew", "category": "somethingNew", "readAt": "2026-11-08T00:00:00.000Z",
                "createdAt": "2026-11-08T00:00:00.000Z", "issue": issue("bb") })),
        ];
        let (issues, notes) = issues_of(nodes);
        let ids: Vec<&str> = issues.iter().map(|issue| issue.id.as_str()).collect();
        assert_eq!(ids, ["a", "bb"]);
        assert_eq!(
            notes["a"],
            Note {
                who: Some("Codex".to_owned()),
                what: "Mention",
                unread: true,
                at: "2026-11-11T00:00:00.000Z".to_owned(),
            }
        );
        assert_eq!(
            notes["bb"],
            Note {
                who: None,
                what: "Update",
                unread: false,
                at: "2026-11-08T00:00:00.000Z".to_owned(),
            }
        );
    }
}
