//! The groups of the list: the open issues, grouped by the state they are in, and sorted.
//!
//! The groups go by the kind of the state (`started`, `unstarted`, `backlog`, `triage`), and within a
//! kind by the position of the state; states of the same name in different teams are one group. In
//! a group the issue updated last is first. `backlog` and `triage` begin folded.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::filter::Filter;
use crate::linear::Issue;
use crate::linear::types::StateKind;

/// One group of the list: the issues of the states that bear a name.
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'a> {
    pub name: &'a str,
    pub kind: StateKind,
    /// Where the group's state is among the others of its kind (the least of its states').
    pub position: f64,
    pub issues: Vec<&'a Issue>,
}

impl Group<'_> {
    /// Whether the group begins folded: what waits, and what is closed (a list holds the closed
    /// issues only where it is a search).
    #[must_use]
    pub fn folded_at_first(&self) -> bool {
        matches!(
            self.kind,
            StateKind::Backlog | StateKind::Triage | StateKind::Completed | StateKind::Canceled
        )
    }
}

/// The groups of `issues` that `filter` lets through; a group with none left is not there.
#[must_use]
pub fn group<'a>(issues: &'a [Issue], filter: &Filter) -> Vec<Group<'a>> {
    let mut at: HashMap<(StateKind, &str), usize> = HashMap::new();
    let mut groups: Vec<Group<'a>> = Vec::new();
    for issue in issues.iter().filter(|issue| filter.matches(issue)) {
        let key = (issue.state.kind, issue.state.name.as_str());
        let index = *at.entry(key).or_insert_with(|| {
            groups.push(Group {
                name: &issue.state.name,
                kind: issue.state.kind,
                position: issue.state.position,
                issues: Vec::new(),
            });
            groups.len() - 1
        });
        let group = &mut groups[index];
        group.position = group.position.min(issue.state.position);
        group.issues.push(issue);
    }
    for group in &mut groups {
        group.issues.sort_by(|a, b| newest_first(a, b));
    }
    groups.sort_by(|a, b| {
        a.kind
            .rank()
            .cmp(&b.kind.rank())
            .then_with(|| a.position.total_cmp(&b.position))
            .then_with(|| a.name.cmp(b.name))
    });
    groups
}

/// The issue updated last first; the same moment, by identifier, so the order does not shake.
fn newest_first(a: &Issue, b: &Issue) -> Ordering {
    b.updated_at
        .cmp(&a.updated_at)
        .then_with(|| a.identifier.cmp(&b.identifier))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::linear::types::{Priority, State};

    /// An issue in a state, for the tests of the model.
    pub fn issue(
        identifier: &str,
        title: &str,
        state: &str,
        kind: &str,
        position: f64,
        updated: &str,
    ) -> Issue {
        Issue {
            id: format!("id-{identifier}"),
            identifier: identifier.to_owned(),
            title: title.to_owned(),
            priority: Priority::None,
            updated_at: format!("{updated}T10:00:00.000Z"),
            url: format!("https://linear.app/x/issue/{identifier}"),
            branch_name: None,
            state: State {
                id: format!("state-{state}"),
                name: state.to_owned(),
                kind: StateKind::of(kind),
                position,
            },
            labels: Vec::new(),
            team: None,
            cycle: None,
            project: None,
            estimate: None,
            due_date: None,
            assignee: None,
            description: None,
            comments: None,
            parent: None,
            children: Vec::new(),
            relations: Vec::new(),
            inverse_relations: Vec::new(),
        }
    }

    fn names<'a>(groups: &[Group<'a>]) -> Vec<&'a str> {
        groups.iter().map(|g| g.name).collect()
    }

    #[test]
    fn groups_go_by_the_kind_then_the_position_of_the_state() {
        let issues = [
            issue("A-1", "a", "Todo", "unstarted", 0.0, "2026-11-01"),
            issue("A-2", "b", "Backlog", "backlog", 0.0, "2026-11-01"),
            issue("A-3", "c", "In Review", "started", 2.0, "2026-11-01"),
            issue("A-4", "d", "In Progress", "started", 1.0, "2026-11-01"),
            issue("A-5", "e", "Triage", "triage", 0.0, "2026-11-01"),
        ];
        let groups = group(&issues, &Filter::default());
        assert_eq!(
            names(&groups),
            ["In Progress", "In Review", "Todo", "Backlog", "Triage"]
        );
    }

    #[test]
    fn states_of_one_name_in_two_teams_are_one_group() {
        let issues = [
            issue("A-1", "a", "In Progress", "started", 1.0, "2026-11-01"),
            issue("B-1", "b", "In Progress", "started", 3.0, "2026-11-02"),
            issue("B-2", "c", "Doing", "started", 2.0, "2026-11-02"),
        ];
        let groups = group(&issues, &Filter::default());
        assert_eq!(names(&groups), ["In Progress", "Doing"]);
        assert_eq!(groups[0].issues.len(), 2);
        // The least position of its states places the group.
        assert!((groups[0].position - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_kind_with_the_same_name_as_another_is_its_own_group() {
        let issues = [
            issue("A-1", "a", "Todo", "unstarted", 0.0, "2026-11-01"),
            issue("A-2", "b", "Todo", "backlog", 0.0, "2026-11-01"),
        ];
        assert_eq!(group(&issues, &Filter::default()).len(), 2);
    }

    #[test]
    fn in_a_group_the_issue_updated_last_is_first() {
        let issues = [
            issue("A-1", "old", "Todo", "unstarted", 0.0, "2026-10-01"),
            issue("A-3", "same b", "Todo", "unstarted", 0.0, "2026-11-05"),
            issue("A-2", "same a", "Todo", "unstarted", 0.0, "2026-11-05"),
            issue("A-4", "new", "Todo", "unstarted", 0.0, "2026-11-09"),
        ];
        let groups = group(&issues, &Filter::default());
        let order: Vec<&str> = groups[0]
            .issues
            .iter()
            .map(|i| i.identifier.as_str())
            .collect();
        assert_eq!(order, ["A-4", "A-2", "A-3", "A-1"]);
    }

    #[test]
    fn backlog_and_triage_begin_folded_and_the_rest_open() {
        let issues = [
            issue("A-1", "a", "Todo", "unstarted", 0.0, "2026-11-01"),
            issue("A-2", "b", "Backlog", "backlog", 0.0, "2026-11-01"),
            issue("A-3", "c", "Triage", "triage", 0.0, "2026-11-01"),
            issue("A-4", "d", "In Progress", "started", 1.0, "2026-11-01"),
        ];
        let folded: Vec<(&str, bool)> = group(&issues, &Filter::default())
            .iter()
            .map(|g| (g.name, g.folded_at_first()))
            .collect();
        assert_eq!(
            folded,
            [
                ("In Progress", false),
                ("Todo", false),
                ("Backlog", true),
                ("Triage", true)
            ]
        );
    }

    #[test]
    fn the_closed_groups_of_a_search_begin_folded() {
        let issues = [
            issue("A-1", "a", "Done", "completed", 0.0, "2026-11-01"),
            issue("A-2", "b", "Canceled", "canceled", 0.0, "2026-11-01"),
            issue("A-3", "c", "Todo", "unstarted", 0.0, "2026-11-01"),
        ];
        let folded: Vec<(&str, bool)> = group(&issues, &Filter::default())
            .iter()
            .map(|g| (g.name, g.folded_at_first()))
            .collect();
        assert_eq!(
            folded,
            [("Todo", false), ("Done", true), ("Canceled", true)]
        );
    }

    #[test]
    fn the_filter_leaves_out_issues_and_the_groups_it_empties() {
        let issues = [
            issue("A-1", "Fix login", "Todo", "unstarted", 0.0, "2026-11-01"),
            issue("A-2", "Fix cart", "Backlog", "backlog", 0.0, "2026-11-01"),
            issue(
                "A-3",
                "Write docs",
                "In Progress",
                "started",
                1.0,
                "2026-11-01",
            ),
        ];
        let groups = group(&issues, &Filter::new("FIX"));
        assert_eq!(names(&groups), ["Todo", "Backlog"]);
        assert!(group(&issues, &Filter::new("nothing like it")).is_empty());
    }
}
