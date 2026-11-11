//! What Linear's answers hold, as plain Rust values: the fields of `docs/calls.md`'s issues are
//! made of these. The wire is nested (`labels { nodes }`, `team { key }`); here it is flat.

use serde::{Deserialize, Deserializer};

/// The kind of a workflow state, which is what groups and colours it (the names are the team's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateKind {
    Triage,
    Backlog,
    Unstarted,
    Started,
    Completed,
    Canceled,
    /// A kind this program does not know: it is shown last.
    Other,
}

impl StateKind {
    #[must_use]
    pub fn of(name: &str) -> Self {
        match name {
            "triage" => Self::Triage,
            "backlog" => Self::Backlog,
            "unstarted" => Self::Unstarted,
            "started" => Self::Started,
            "completed" => Self::Completed,
            "canceled" => Self::Canceled,
            _ => Self::Other,
        }
    }

    /// The name Linear gives the kind, and the page is told.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Triage => "triage",
            Self::Backlog => "backlog",
            Self::Unstarted => "unstarted",
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
            Self::Other => "other",
        }
    }

    /// Where its group is in the list: what is being done first, what waits last.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::Started => 0,
            Self::Unstarted => 1,
            Self::Backlog => 2,
            Self::Triage => 3,
            Self::Completed => 4,
            Self::Canceled => 5,
            Self::Other => 6,
        }
    }
}

impl<'de> Deserialize<'de> for StateKind {
    fn deserialize<D: Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        Ok(Self::of(&String::deserialize(from)?))
    }
}

/// Linear's priority: 0 is none, 1 urgent, 2 high, 3 medium, 4 low.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    None,
    Urgent,
    High,
    Medium,
    Low,
}

impl Priority {
    #[must_use]
    pub fn of(number: f64) -> Self {
        // The wire says a float; it is a whole number from 0 to 4.
        match number {
            1.0 => Self::Urgent,
            2.0 => Self::High,
            3.0 => Self::Medium,
            4.0 => Self::Low,
            _ => Self::None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Urgent => "urgent",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl<'de> Deserialize<'de> for Priority {
    fn deserialize<D: Deserializer<'de>>(from: D) -> Result<Self, D::Error> {
        Ok(Self::of(f64::deserialize(from)?))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct State {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: StateKind,
    /// Where the state is among its team's, ascending.
    #[serde(default)]
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub name: String,
    /// `#rrggbb`, Linear's own colour of the label.
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignee {
    pub display_name: String,
    #[serde(default)]
    pub is_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Organization {
    pub name: String,
    #[serde(rename = "urlKey")]
    pub url_key: String,
}

/// Whose key it is.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub organization: Organization,
}

/// `{ nodes: [...] }`, read as the list.
fn nodes<'de, D: Deserializer<'de>, T: Deserialize<'de>>(from: D) -> Result<Vec<T>, D::Error> {
    #[derive(Deserialize)]
    struct Nodes<T> {
        nodes: Vec<T>,
    }
    Ok(Option::<Nodes<T>>::deserialize(from)?.map_or_else(Vec::new, |n| n.nodes))
}

/// `{ key: "ACM" }`, read as the key.
fn team_key<'de, D: Deserializer<'de>>(from: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    struct Team {
        key: String,
    }
    Ok(Option::<Team>::deserialize(from)?.map(|t| t.key))
}

/// `{ number: 12 }`, read as the number.
fn cycle_number<'de, D: Deserializer<'de>>(from: D) -> Result<Option<u32>, D::Error> {
    #[derive(Deserialize)]
    struct Cycle {
        number: f64,
    }
    // A cycle's number is a whole count of cycles; what is not one is no cycle.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(Option::<Cycle>::deserialize(from)?
        .filter(|c| c.number >= 0.0 && c.number <= f64::from(u32::MAX))
        .map(|c| c.number as u32))
}

/// `{ name: "Billing" }`, read as the name.
fn project_name<'de, D: Deserializer<'de>>(from: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    struct Project {
        name: String,
    }
    Ok(Option::<Project>::deserialize(from)?.map(|p| p.name))
}

/// An issue as the list needs it. The description and the comments are not here: they are asked for
/// when an issue is opened (`docs/calls.md`), not for every row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    /// `ACM-412`.
    pub identifier: String,
    pub title: String,
    pub priority: Priority,
    /// ISO 8601 in UTC, as Linear writes it: it sorts as text.
    pub updated_at: String,
    pub url: String,
    /// The branch Linear names for the issue. Untrusted text: it is checked before `git` sees it.
    #[serde(default)]
    pub branch_name: Option<String>,
    pub state: State,
    #[serde(default, deserialize_with = "nodes")]
    pub labels: Vec<Label>,
    #[serde(default, rename = "team", deserialize_with = "team_key")]
    pub team: Option<String>,
    #[serde(default, rename = "cycle", deserialize_with = "cycle_number")]
    pub cycle: Option<u32>,
    #[serde(default, rename = "project", deserialize_with = "project_name")]
    pub project: Option<String>,
    #[serde(default)]
    pub assignee: Option<Assignee>,
    /// Untrusted markdown. Only an issue asked for alone has it.
    #[serde(default)]
    pub description: Option<String>,
    /// Only an issue asked for alone has them.
    #[serde(default)]
    pub comments: Option<Comments>,
}

/// Who wrote a comment (a comment of a bot or of someone gone has none).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub display_name: String,
}

/// A comment of an issue. The text is untrusted.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde(default)]
    pub body: String,
    pub created_at: String,
    #[serde(default)]
    pub user: Option<Author>,
}

/// The first comments of an issue, and whether there are more.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Comments {
    pub nodes: Vec<Comment>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    #[serde(default)]
    pub end_cursor: Option<String>,
}

/// One page of issues.
#[derive(Debug, Clone, Deserialize)]
pub struct Connection {
    pub nodes: Vec<Issue>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

/// An issue, as the tests of other parts make one: ACM-412, with a comment.
#[cfg(test)]
pub fn fixture_issue() -> Issue {
    serde_json::from_str(
        r##"{
            "id": "issue-412", "identifier": "ACM-412", "title": "Add rate limiting", "priority": 2,
            "updatedAt": "2026-11-09T14:20:00.000Z",
            "url": "https://linear.app/acme/issue/ACM-412/add-rate-limiting",
            "branchName": "ada/acm-412-add-rate-limiting",
            "state": {"id": "s1", "name": "In Progress", "type": "started", "position": 1},
            "labels": {"nodes": [{"name": "api", "color": "#3e63dd"}]},
            "team": {"key": "ACM"}, "cycle": {"number": 12}, "project": null,
            "assignee": {"displayName": "Ada", "isMe": true},
            "comments": {"nodes": [{"body": "Redis is already there.",
                "createdAt": "2026-11-06T09:00:00.000Z", "user": {"displayName": "Deniz"}}],
                "pageInfo": {"hasNextPage": false}}
        }"##,
    )
    .expect("the fixture issue")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_issue_is_read_flat_from_the_nested_wire() {
        let wire = r##"{
            "id": "i1", "identifier": "ACM-412", "title": "Add rate limiting", "priority": 2,
            "updatedAt": "2026-11-09T14:20:00.000Z", "url": "https://linear.app/acme/issue/ACM-412",
            "branchName": "acm-412-add-rate-limiting",
            "state": {"id": "s1", "name": "In Progress", "type": "started", "position": 1},
            "labels": {"nodes": [{"name": "api", "color": "#3e63dd"}]},
            "team": {"key": "ACM"}, "cycle": {"number": 7}, "project": null,
            "assignee": {"displayName": "Ada", "isMe": true}
        }"##;
        let issue: Issue = serde_json::from_str(wire).unwrap();
        assert_eq!(issue.identifier, "ACM-412");
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.state.kind, StateKind::Started);
        assert_eq!(issue.labels[0].color, "#3e63dd");
        assert_eq!(issue.team.as_deref(), Some("ACM"));
        assert_eq!(issue.cycle, Some(7));
        assert_eq!(issue.project, None);
        assert!(issue.assignee.is_some_and(|a| a.is_me));
    }

    #[test]
    fn what_an_issue_may_lack_is_read_as_lacking() {
        let wire = r#"{
            "id": "i1", "identifier": "ACM-1", "title": "t", "priority": 0,
            "updatedAt": "2026-11-09T14:20:00.000Z", "url": "u",
            "state": {"id": "s", "name": "Todo", "type": "unstarted", "position": 0},
            "labels": {"nodes": []}, "team": null, "cycle": null, "project": null, "assignee": null
        }"#;
        let issue: Issue = serde_json::from_str(wire).unwrap();
        assert_eq!(issue.priority, Priority::None);
        assert_eq!(issue.branch_name, None);
        assert!(issue.labels.is_empty());
        assert_eq!(issue.assignee, None);
    }

    #[test]
    fn the_kinds_of_a_state_are_ordered_and_an_unknown_one_is_last() {
        let order = [
            "started",
            "unstarted",
            "backlog",
            "triage",
            "completed",
            "canceled",
            "x",
        ]
        .map(|name| StateKind::of(name).rank());
        assert_eq!(order, [0, 1, 2, 3, 4, 5, 6]);
    }
}
