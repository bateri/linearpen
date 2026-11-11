//! The change the person makes to issues: what it can be, and so everything that can be sent. The
//! `input` of `IssueBatchUpdate` is made of a `Change` and of nothing else, and a `Change` has no
//! member for the description or for the team: neither is written by any way there is. One change is
//! one field; to change two, the person changes twice.

use serde::Serialize;

use super::types::Priority;

/// One field of an issue, given a new value. Linear's ids are what name a state, a person, a label
/// or a project.
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
            Self::Title(title) => input.title = Some(title),
        }
        input
    }
}

/// The `input` of `IssueBatchUpdate`. Only a `Change` makes one (its members are not public), and
/// what is not changed is not in it; a person given to no one, or a project left, is a `null`.
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
