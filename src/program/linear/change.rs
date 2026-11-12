//! The change the person makes to issues: what it can be, and so everything that can be sent. The
//! `input` of `IssueBatchUpdate` is made of a `Change` and of nothing else, and a `Change` has no
//! member for the description or for the team: neither is written by any way there is. One change is
//! one field; to change two, the person changes twice.

use serde::Serialize;

use super::types::Priority;

/// One field of an issue, given a new value. Linear's ids are what name a state, a person, a label,
/// a project or a cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The workflow state, by id.
    State(String),
    Priority(Priority),
    /// Given to the person with this id, or to no one.
    Assignee(Option<String>),
    /// A label the issue gets, by id.
    AddLabel(String),
    /// A label the issue loses, by id.
    RemoveLabel(String),
    /// Put in the project with this id, or in none.
    Project(Option<String>),
    /// Put in the cycle with this id, or in none.
    Cycle(Option<String>),
    /// The estimate in points (what the team's scale sends: a t-shirt size is its Fibonacci number),
    /// or none.
    Estimate(Option<u32>),
    /// The day it is due, written `2026-11-15`, or none.
    DueDate(Option<String>),
    Title(String),
}

impl Change {
    /// The title `text` makes: one line, with no white space at its ends and none doubled, or
    /// nothing where it is empty.
    #[must_use]
    pub fn title(text: &str) -> Option<Self> {
        let one_line = text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        (!one_line.is_empty()).then_some(Self::Title(one_line))
    }

    /// The due date `text` is: a day written as year, month and day (`2026-11-15`) that is on the
    /// calendar, or nothing where it is not one.
    #[must_use]
    pub fn due_date(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
        let whole = |part: &str, width: usize| {
            (part.len() == width && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        };
        let (year, month, day) = (whole(year, 4)?, whole(month, 2)?, whole(day, 2)?);
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let last = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        ((1..=last).contains(&day) && parts.next().is_none())
            .then(|| Self::DueDate(Some(text.to_owned())))
    }

    /// What is sent of the change: its one field.
    #[must_use]
    pub fn input(&self) -> Input<'_> {
        let mut input = Input::default();
        match self {
            Self::State(id) => input.state_id = Some(id),
            Self::Priority(priority) => input.priority = Some(priority.number()),
            Self::Assignee(who) => input.assignee_id = Some(who.as_deref()),
            Self::AddLabel(id) => input.added_label_ids = vec![id],
            Self::RemoveLabel(id) => input.removed_label_ids = vec![id],
            Self::Project(project) => input.project_id = Some(project.as_deref()),
            Self::Cycle(cycle) => input.cycle_id = Some(cycle.as_deref()),
            Self::Estimate(points) => input.estimate = Some(*points),
            Self::DueDate(day) => input.due_date = Some(day.as_deref()),
            Self::Title(title) => input.title = Some(title),
        }
        input
    }
}

/// The `input` of `IssueBatchUpdate`. Only a `Change` makes one (its members are not public), and
/// what is not changed is not in it; a person given to no one, or a project, a cycle, an estimate or a
/// due date taken away, is a `null`.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    state_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignee_id: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    added_label_ids: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    removed_label_ids: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cycle_id: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimate: Option<Option<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    due_date: Option<Option<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn input_of(change: &Change) -> Value {
        serde_json::to_value(change.input()).unwrap()
    }

    /// Every kind of change, with the input it makes.
    fn all() -> Vec<(Change, Value)> {
        vec![
            (Change::State("s-done".into()), json!({"stateId": "s-done"})),
            (Change::Priority(Priority::Urgent), json!({"priority": 1})),
            (Change::Priority(Priority::None), json!({"priority": 0})),
            (
                Change::Assignee(Some("u-ada".into())),
                json!({"assigneeId": "u-ada"}),
            ),
            (Change::Assignee(None), json!({"assigneeId": null})),
            (
                Change::AddLabel("l-bug".into()),
                json!({"addedLabelIds": ["l-bug"]}),
            ),
            (
                Change::RemoveLabel("l-bug".into()),
                json!({"removedLabelIds": ["l-bug"]}),
            ),
            (
                Change::Project(Some("p-1".into())),
                json!({"projectId": "p-1"}),
            ),
            (Change::Project(None), json!({"projectId": null})),
            (
                Change::Cycle(Some("c-24".into())),
                json!({"cycleId": "c-24"}),
            ),
            (Change::Cycle(None), json!({"cycleId": null})),
            (Change::Estimate(Some(5)), json!({"estimate": 5})),
            (Change::Estimate(Some(0)), json!({"estimate": 0})),
            (Change::Estimate(None), json!({"estimate": null})),
            (
                Change::DueDate(Some("2026-11-15".into())),
                json!({"dueDate": "2026-11-15"}),
            ),
            (Change::DueDate(None), json!({"dueDate": null})),
            (Change::Title("New".into()), json!({"title": "New"})),
        ]
    }

    #[test]
    fn each_change_is_exactly_one_field_of_the_input() {
        for (change, expected) in all() {
            let input = input_of(&change);
            assert_eq!(input, expected, "{change:?}");
            assert_eq!(input.as_object().unwrap().len(), 1, "{change:?}");
        }
    }

    /// What the program may not write is in no body it sends, whatever the change.
    #[test]
    fn no_change_carries_a_description_or_a_team() {
        for (change, _) in all() {
            let body = serde_json::to_string(&crate::linear::query::Batch {
                ids: &["issue-1".to_owned()],
                input: change.input(),
            })
            .unwrap()
            .to_ascii_lowercase();
            for word in ["description", "teamid", "team"] {
                assert!(!body.contains(word), "{change:?} sends {word}: {body}");
            }
        }
        // And the type has no member for either.
        let source = include_str!("change.rs");
        let (code, _) = source.split_once("#[cfg(test)]").unwrap();
        let code = code.to_ascii_lowercase();
        for word in ["description:", "team_id", "teamid"] {
            assert!(!code.contains(word), "change.rs has {word}");
        }
    }

    #[test]
    fn a_due_date_is_a_day_of_the_calendar_written_as_year_month_day() {
        assert_eq!(
            Change::due_date("2026-11-15"),
            Some(Change::DueDate(Some("2026-11-15".into())))
        );
        assert!(Change::due_date("2028-02-29").is_some());
        for bad in [
            "",
            "tomorrow",
            "2026-11",
            "2026-11-15-1",
            "2026-13-01",
            "2026-00-10",
            "2026-11-31",
            "2027-02-29",
            "2026-11-00",
            "26-11-15",
            "2026/11/15",
            "2026-1-5",
            "2026-11-15T00:00",
            "+026-11-15",
            " 2026-11-15",
        ] {
            assert_eq!(Change::due_date(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_title_is_one_line_with_no_white_space_at_its_ends() {
        assert_eq!(
            Change::title("  Fix the login \n"),
            Some(Change::Title("Fix the login".into()))
        );
        assert_eq!(
            Change::title("one\n  two \r\nthree"),
            Some(Change::Title("one two three".into()))
        );
        assert_eq!(Change::title(" \n\t "), None);
    }
}
