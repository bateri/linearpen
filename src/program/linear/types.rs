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

    /// The priority named as `name` says it (`urgent`), or nothing where it is not a name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::None,
            Self::Urgent,
            Self::High,
            Self::Medium,
            Self::Low,
        ]
        .into_iter()
        .find(|priority| priority.name() == name)
    }

    /// The number Linear writes it as: 0 none, 1 urgent, 2 high, 3 medium, 4 low.
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }

    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::None
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
    /// Linear's id of the label: what a change adds or removes it by.
    pub id: String,
    pub name: String,
    /// `#rrggbb`, Linear's own colour of the label.
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignee {
    /// Linear's id of the person: what a change gives the issue by.
    pub id: String,
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

/// The team an issue is of: its id (a change that moves nothing between teams is told of one by it,
/// and a menu is of the issue's team) and the key its identifier begins with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TeamRef {
    pub id: String,
    /// `ACM`.
    pub key: String,
}

/// The project an issue is in, by Linear's id and by name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

/// The cycle an issue is in: its id (a change puts an issue in a cycle by it), its number and its
/// name where the team gave it one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleRef {
    pub id: String,
    pub number: u32,
    pub name: Option<String>,
}

impl CycleRef {
    /// What tells the cycle: its name where it has one, else `Cycle 24`.
    #[must_use]
    pub fn label(&self) -> String {
        cycle_label(self.number, self.name.as_deref())
    }
}

/// The words of a cycle (`Spring`, or `Cycle 24` where it has no name).
#[must_use]
pub fn cycle_label(number: u32, name: Option<&str>) -> String {
    match name.map(str::trim) {
        Some(name) if !name.is_empty() => name.to_owned(),
        _ => format!("Cycle {number}"),
    }
}

/// `{ id, number: 12, name }`, read as a cycle. A number is a whole count of cycles; what is not one
/// is no cycle.
fn cycle_ref<'de, D: Deserializer<'de>>(from: D) -> Result<Option<CycleRef>, D::Error> {
    #[derive(Deserialize)]
    struct Cycle {
        id: String,
        number: f64,
        #[serde(default)]
        name: Option<String>,
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(Option::<Cycle>::deserialize(from)?
        .filter(|c| c.number.fract() == 0.0 && c.number >= 0.0 && c.number <= f64::from(u32::MAX))
        .map(|c| CycleRef {
            id: c.id,
            number: c.number as u32,
            name: c.name,
        }))
}

/// A number of points as Linear writes an estimate (a float), read as a whole number of them; what
/// is not one is no estimate.
fn points<'de, D: Deserializer<'de>>(from: D) -> Result<Option<u32>, D::Error> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(Option::<f64>::deserialize(from)?
        .filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= f64::from(u32::MAX))
        .map(|n| n as u32))
}

/// An issue another issue is tied to: its parent, a sub-issue, or one it is blocked by. Enough to
/// name it and to open it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Neighbor {
    pub id: String,
    pub identifier: String,
    /// Untrusted text.
    pub title: String,
    pub state: State,
}

/// A relation as Linear writes it: seen from one of its two issues, the other is `issue` (where this
/// is the related one) or `related_issue` (where this is the first).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub issue: Option<Neighbor>,
    #[serde(default)]
    pub related_issue: Option<Neighbor>,
}

/// How an issue is tied to another, seen from the issue that is open, in the order they are told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tie {
    Blocks,
    BlockedBy,
    Related,
    DuplicateOf,
    DuplicatedBy,
}

impl Tie {
    /// The words the page and the file tell it by.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Blocks => "Blocks",
            Self::BlockedBy => "Blocked by",
            Self::Related => "Related",
            Self::DuplicateOf => "Duplicate of",
            Self::DuplicatedBy => "Duplicated by",
        }
    }
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
    #[serde(default)]
    pub team: Option<TeamRef>,
    #[serde(default, deserialize_with = "cycle_ref")]
    pub cycle: Option<CycleRef>,
    #[serde(default)]
    pub project: Option<ProjectRef>,
    /// Points, where the team estimates.
    #[serde(default, deserialize_with = "points")]
    pub estimate: Option<u32>,
    /// `2026-11-15`, the day it is due.
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub assignee: Option<Assignee>,
    /// Untrusted markdown. Only an issue asked for alone has it.
    #[serde(default)]
    pub description: Option<String>,
    /// Only an issue asked for alone has them.
    #[serde(default)]
    pub comments: Option<Comments>,
    /// Only an issue asked for alone has its surroundings: the issue it is a part of, its parts and
    /// what it is tied to (the first fifty of each).
    #[serde(default)]
    pub parent: Option<Neighbor>,
    #[serde(default, deserialize_with = "nodes")]
    pub children: Vec<Neighbor>,
    #[serde(default, deserialize_with = "nodes")]
    pub relations: Vec<Relation>,
    #[serde(default, deserialize_with = "nodes")]
    pub inverse_relations: Vec<Relation>,
}

impl Issue {
    /// What the issue is tied to, each with how, in the order the page tells them: blocks, blocked by,
    /// related, duplicate of, duplicated by. A type Linear has that is not one of these (`similar`,
    /// or one of the future) is left out.
    #[must_use]
    pub fn ties(&self) -> Vec<(Tie, &Neighbor)> {
        let mut out = Vec::new();
        for (tie, from_here) in [
            (Tie::Blocks, true),
            (Tie::BlockedBy, false),
            (Tie::Related, true),
            (Tie::Related, false),
            (Tie::DuplicateOf, true),
            (Tie::DuplicatedBy, false),
        ] {
            let kind = match tie {
                Tie::Blocks | Tie::BlockedBy => "blocks",
                Tie::Related => "related",
                Tie::DuplicateOf | Tie::DuplicatedBy => "duplicate",
            };
            let others: Vec<&Neighbor> = if from_here {
                self.relations
                    .iter()
                    .filter(|r| r.kind == kind)
                    .filter_map(|r| r.related_issue.as_ref())
                    .collect()
            } else {
                self.inverse_relations
                    .iter()
                    .filter(|r| r.kind == kind)
                    .filter_map(|r| r.issue.as_ref())
                    .collect()
            };
            out.extend(others.into_iter().map(|other| (tie, other)));
        }
        out
    }
}

/// Who wrote a comment (a comment of a bot or of someone gone has none), and whether it is the
/// person whose key it is: only they may edit it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub display_name: String,
    #[serde(default)]
    pub is_me: bool,
}

/// A comment of an issue. The text is untrusted.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// Linear's id of it: what an edit names.
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub body: String,
    pub created_at: String,
    /// When its author last edited it, where they did.
    #[serde(default)]
    pub edited_at: Option<String>,
    #[serde(default)]
    pub user: Option<Author>,
}

impl Comment {
    /// Whether the person whose key it is wrote it: a comment of a bot, of an integration or of someone
    /// gone is no one's.
    #[must_use]
    pub fn mine(&self) -> bool {
        self.user.as_ref().is_some_and(|user| user.is_me)
    }
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
            "labels": {"nodes": [{"id": "l-api", "name": "api", "color": "#3e63dd"}]},
            "team": {"id": "team-acm", "key": "ACM"}, "cycle": {"id": "c-12", "number": 12}, "project": null,
            "assignee": {"id": "u-ada", "displayName": "Ada", "isMe": true},
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
            "labels": {"nodes": [{"id": "l-api", "name": "api", "color": "#3e63dd"}]},
            "team": {"id": "team-acm", "key": "ACM"}, "cycle": {"id": "c-7", "number": 7, "name": null},
            "project": null, "estimate": 3, "dueDate": "2026-11-15",
            "assignee": {"id": "u-ada", "displayName": "Ada", "isMe": true}
        }"##;
        let issue: Issue = serde_json::from_str(wire).unwrap();
        assert_eq!(issue.identifier, "ACM-412");
        assert_eq!(issue.priority, Priority::High);
        assert_eq!(issue.state.kind, StateKind::Started);
        assert_eq!(issue.labels[0].color, "#3e63dd");
        assert_eq!(issue.team.as_ref().map(|t| t.key.as_str()), Some("ACM"));
        assert_eq!(issue.team.as_ref().map(|t| t.id.as_str()), Some("team-acm"));
        assert_eq!(issue.labels[0].id, "l-api");
        assert_eq!(
            issue.assignee.as_ref().map(|a| a.id.as_str()),
            Some("u-ada")
        );
        assert_eq!(
            issue.cycle,
            Some(CycleRef {
                id: "c-7".into(),
                number: 7,
                name: None
            })
        );
        assert_eq!(issue.estimate, Some(3));
        assert_eq!(issue.due_date.as_deref(), Some("2026-11-15"));
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
    fn a_cycle_is_told_by_its_name_or_else_by_its_number() {
        let label = |cycle: &str| -> Option<String> {
            let wire = format!(
                r#"{{"id": "i", "identifier": "ACM-1", "title": "t", "priority": 0, "updatedAt": "x",
                "url": "u", "state": {{"id": "s", "name": "Todo", "type": "unstarted"}},
                "labels": {{"nodes": []}}, "cycle": {cycle}}}"#
            );
            serde_json::from_str::<Issue>(&wire)
                .unwrap()
                .cycle
                .map(|c| c.label())
        };
        assert_eq!(
            label(r#"{"id": "c-1", "number": 24, "name": "Spring"}"#).as_deref(),
            Some("Spring")
        );
        assert_eq!(
            label(r#"{"id": "c-1", "number": 24.0}"#).as_deref(),
            Some("Cycle 24")
        );
        assert_eq!(
            label(r#"{"id": "c-1", "number": 24, "name": "  "}"#).as_deref(),
            Some("Cycle 24")
        );
        assert_eq!(label(r#"{"id": "c-1", "number": 2.5}"#), None);
        assert_eq!(label("null"), None);
    }

    /// An issue with its surroundings as `Issue` asks for them.
    fn around() -> Issue {
        let neighbor = |id: &str, name: &str| {
            format!(
                r#"{{"id": "{id}", "identifier": "{name}", "title": "Title of {name}",
                "state": {{"id": "s", "name": "Todo", "type": "unstarted", "position": 1}}}}"#
            )
        };
        let wire = format!(
            r##"{{
            "id": "i1", "identifier": "ACM-412", "title": "t", "priority": 0,
            "updatedAt": "2026-11-09T14:20:00.000Z", "url": "u",
            "state": {{"id": "s", "name": "Todo", "type": "unstarted", "position": 0}},
            "labels": {{"nodes": []}}, "team": null, "cycle": null, "project": null, "assignee": null,
            "parent": {parent},
            "children": {{"nodes": [{child_a}, {child_b}]}},
            "relations": {{"nodes": [
                {{"type": "related", "relatedIssue": {related}}},
                {{"type": "blocks", "relatedIssue": {blocked}}},
                {{"type": "duplicate", "relatedIssue": {original}}},
                {{"type": "similar", "relatedIssue": {similar}}}
            ]}},
            "inverseRelations": {{"nodes": [
                {{"type": "blocks", "issue": {blocker}}},
                {{"type": "duplicate", "issue": {copy}}},
                {{"type": "related", "issue": {mutual}}},
                {{"type": "somethingnew", "issue": {unknown}}}
            ]}}
        }}"##,
            parent = neighbor("i-400", "ACM-400"),
            child_a = neighbor("i-1", "ACM-1"),
            child_b = neighbor("i-2", "ACM-2"),
            related = neighbor("i-3", "ACM-3"),
            blocked = neighbor("i-4", "ACM-4"),
            original = neighbor("i-5", "ACM-5"),
            similar = neighbor("i-6", "ACM-6"),
            blocker = neighbor("i-7", "ACM-7"),
            copy = neighbor("i-8", "ACM-8"),
            mutual = neighbor("i-9", "ACM-9"),
            unknown = neighbor("i-10", "ACM-10"),
        );
        serde_json::from_str(&wire).unwrap()
    }

    #[test]
    fn the_surroundings_of_an_issue_are_read_from_the_wire() {
        let issue = around();
        assert_eq!(issue.parent.as_ref().unwrap().identifier, "ACM-400");
        assert_eq!(
            issue.parent.as_ref().unwrap().state.kind,
            StateKind::Unstarted
        );
        assert_eq!(
            issue
                .children
                .iter()
                .map(|c| c.identifier.as_str())
                .collect::<Vec<_>>(),
            ["ACM-1", "ACM-2"]
        );
        let bare: Issue = serde_json::from_str(
            r#"{"id": "i", "identifier": "ACM-1", "title": "t", "priority": 0, "updatedAt": "x",
            "url": "u", "state": {"id": "s", "name": "Todo", "type": "unstarted"},
            "labels": {"nodes": []}}"#,
        )
        .unwrap();
        assert!(bare.parent.is_none() && bare.children.is_empty() && bare.ties().is_empty());
    }

    #[test]
    fn ties_read_both_ways_and_in_one_order_and_leave_out_what_is_not_known() {
        let issue = around();
        let ties: Vec<(Tie, &str)> = issue
            .ties()
            .into_iter()
            .map(|(tie, issue)| (tie, issue.identifier.as_str()))
            .collect();
        // This issue blocks ACM-4 (`blocks` of its own relations); ACM-7 blocks this one (an inverse
        // `blocks`); `similar` and a type of the future are not told.
        assert_eq!(
            ties,
            [
                (Tie::Blocks, "ACM-4"),
                (Tie::BlockedBy, "ACM-7"),
                (Tie::Related, "ACM-3"),
                (Tie::Related, "ACM-9"),
                (Tie::DuplicateOf, "ACM-5"),
                (Tie::DuplicatedBy, "ACM-8"),
            ]
        );
        assert_eq!(
            [
                Tie::Blocks,
                Tie::BlockedBy,
                Tie::Related,
                Tie::DuplicateOf,
                Tie::DuplicatedBy
            ]
            .map(Tie::label),
            [
                "Blocks",
                "Blocked by",
                "Related",
                "Duplicate of",
                "Duplicated by"
            ]
        );
    }

    #[test]
    fn an_estimate_that_is_not_a_whole_number_of_points_is_none() {
        let read = |estimate: &str| -> Option<u32> {
            let wire = format!(
                r#"{{"id": "i", "identifier": "ACM-1", "title": "t", "priority": 0, "updatedAt": "x",
                "url": "u", "state": {{"id": "s", "name": "Todo", "type": "unstarted"}},
                "labels": {{"nodes": []}}, "estimate": {estimate}}}"#
            );
            serde_json::from_str::<Issue>(&wire).unwrap().estimate
        };
        assert_eq!(read("5.0"), Some(5));
        assert_eq!(read("0"), Some(0));
        assert_eq!(read("null"), None);
        assert_eq!(read("0.5"), None);
        assert_eq!(read("-2"), None);
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
