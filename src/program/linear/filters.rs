//! What a person narrows a list with: the status, the priority, the assignee, the labels and the
//! project of the issues. They are sent to Linear, put with the list's own filter by "and", and
//! apply to every list (the one of a search too). Only these shapes are made, and each is one Linear
//! takes: `state.name in`, `priority in`, `assignee.isMe`, `assignee.id in`, `assignee null`,
//! `labels.some.name in`, `project.id in` and `project null`.

use serde_json::{Value, json};

/// The most values one kind of filter holds, and the most characters of one.
pub const VALUES_MOST: usize = 50;
pub const VALUE_MOST: usize = 100;

/// The word that stands for the person themself in the assignees, and the one for no one (no
/// assignee, no project).
pub const ME: &str = "me";
pub const NONE: &str = "none";

/// The priorities by the words the page says them in, and what Linear numbers them.
const PRIORITIES: [(&str, u8); 5] = [
    ("none", 0),
    ("urgent", 1),
    ("high", 2),
    ("medium", 3),
    ("low", 4),
];

/// What a list is narrowed by. A kind with no value does not narrow; several values of one kind are
/// "any of them", and the kinds are "all of them".
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Filters {
    /// The names of workflow states (`In Progress`): the teams each have their own, by one name.
    pub status: Vec<String>,
    /// `urgent`, `high`, `medium`, `low` or `none`.
    pub priority: Vec<String>,
    /// `me`, `none`, or Linear's id of a person.
    pub assignee: Vec<String>,
    /// The names of labels.
    pub labels: Vec<String>,
    /// `none`, or Linear's id of a project.
    pub project: Vec<String>,
}

/// Why a filter is not taken.
#[derive(Debug, PartialEq, Eq)]
pub struct Invalid(pub String);

fn plain(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= VALUE_MOST && !value.chars().any(char::is_control)
}

fn id_like(value: &str) -> bool {
    plain(value)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `values` once each, in the order given, if there are not too many and each is `fine`.
fn taken(
    kind: &str,
    values: Vec<String>,
    fine: impl Fn(&str) -> bool,
) -> Result<Vec<String>, Invalid> {
    let mut out: Vec<String> = Vec::new();
    for value in values {
        if !fine(&value) {
            return Err(Invalid(format!(
                "a {kind} filter is not one that can be asked for"
            )));
        }
        if !out.contains(&value) {
            out.push(value);
        }
    }
    if out.len() > VALUES_MOST {
        return Err(Invalid(format!(
            "a {kind} filter holds at most {VALUES_MOST} values"
        )));
    }
    Ok(out)
}

impl Filters {
    /// The filters the page named, checked: nothing that is not one of the shapes above.
    ///
    /// # Errors
    /// What is not a name, an id or a priority the program knows.
    pub fn new(
        status: Vec<String>,
        priority: Vec<String>,
        assignee: Vec<String>,
        labels: Vec<String>,
        project: Vec<String>,
    ) -> Result<Self, Invalid> {
        Ok(Self {
            status: taken("status", status, plain)?,
            priority: taken("priority", priority, |value| {
                PRIORITIES.iter().any(|(name, _)| *name == value)
            })?,
            assignee: taken("assignee", assignee, |value| {
                value == ME || value == NONE || id_like(value)
            })?,
            labels: taken("labels", labels, plain)?,
            project: taken("project", project, |value| value == NONE || id_like(value))?,
        })
    }

    /// Whether nothing is narrowed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.status.is_empty()
            && self.priority.is_empty()
            && self.assignee.is_empty()
            && self.labels.is_empty()
            && self.project.is_empty()
    }

    /// The filter Linear is given: `open` where the list holds the open issues only, and the
    /// person's own, each kind a part, "and"ed (so a closed state asked for in a list of the open
    /// issues finds nothing: it is a search's). No part is `null`; one part is that part (what was
    /// asked before the filters were).
    #[must_use]
    pub fn to_value(&self, open: bool) -> Value {
        let mut parts: Vec<Value> = Vec::new();
        if open {
            parts.push(json!({ "state": { "type": { "nin": ["completed", "canceled"] } } }));
        }
        if !self.status.is_empty() {
            parts.push(json!({ "state": { "name": { "in": self.status } } }));
        }
        if !self.priority.is_empty() {
            let numbers: Vec<u8> = self
                .priority
                .iter()
                .filter_map(|word| {
                    PRIORITIES
                        .iter()
                        .find(|(name, _)| name == word)
                        .map(|(_, n)| *n)
                })
                .collect();
            parts.push(json!({ "priority": { "in": numbers } }));
        }
        if !self.assignee.is_empty() {
            let mut any: Vec<Value> = Vec::new();
            let ids: Vec<&String> = self
                .assignee
                .iter()
                .filter(|value| *value != ME && *value != NONE)
                .collect();
            if self.assignee.iter().any(|value| value == ME) {
                any.push(json!({ "assignee": { "isMe": { "eq": true } } }));
            }
            if !ids.is_empty() {
                any.push(json!({ "assignee": { "id": { "in": ids } } }));
            }
            if self.assignee.iter().any(|value| value == NONE) {
                any.push(json!({ "assignee": { "null": true } }));
            }
            parts.push(one_of(any));
        }
        if !self.labels.is_empty() {
            parts.push(json!({ "labels": { "some": { "name": { "in": self.labels } } } }));
        }
        if !self.project.is_empty() {
            let mut any: Vec<Value> = Vec::new();
            let ids: Vec<&String> = self.project.iter().filter(|value| *value != NONE).collect();
            if !ids.is_empty() {
                any.push(json!({ "project": { "id": { "in": ids } } }));
            }
            if self.project.iter().any(|value| value == NONE) {
                any.push(json!({ "project": { "null": true } }));
            }
            parts.push(one_of(any));
        }
        match parts.len() {
            0 => Value::Null,
            1 => parts.remove(0),
            _ => json!({ "and": parts }),
        }
    }
}

/// One of `any`, as one filter: the one there is, or "or" of them.
fn one_of(mut any: Vec<Value>) -> Value {
    if any.len() == 1 {
        any.remove(0)
    } else {
        json!({ "or": any })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filters(status: &[&str], assignee: &[&str], labels: &[&str], project: &[&str]) -> Filters {
        let owned = |of: &[&str]| of.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
        Filters::new(
            owned(status),
            Vec::new(),
            owned(assignee),
            owned(labels),
            owned(project),
        )
        .expect("filters")
    }

    #[test]
    fn no_filter_is_the_open_filter_alone_as_it_was_asked_before_the_filters() {
        assert_eq!(
            Filters::default().to_value(true),
            json!({ "state": { "type": { "nin": ["completed", "canceled"] } } })
        );
        // A search holds closed issues too: with no filter there is nothing to send.
        assert_eq!(Filters::default().to_value(false), Value::Null);
    }

    #[test]
    fn the_kinds_are_and_ed_with_the_open_filter_and_the_values_of_a_kind_are_any_of_them() {
        let value = filters(&[], &["me", "u-deniz", "none"], &["bug"], &["none"]).to_value(true);
        assert_eq!(
            value,
            json!({ "and": [
                { "state": { "type": { "nin": ["completed", "canceled"] } } },
                { "or": [
                    { "assignee": { "isMe": { "eq": true } } },
                    { "assignee": { "id": { "in": ["u-deniz"] } } },
                    { "assignee": { "null": true } },
                ] },
                { "labels": { "some": { "name": { "in": ["bug"] } } } },
                { "project": { "null": true } },
            ] })
        );
    }

    #[test]
    fn states_asked_for_are_and_ed_with_the_open_filter_and_alone_in_a_search() {
        let narrowed = filters(&["Done", "In Progress"], &[], &[], &[]);
        assert_eq!(
            narrowed.to_value(true),
            json!({ "and": [
                { "state": { "type": { "nin": ["completed", "canceled"] } } },
                { "state": { "name": { "in": ["Done", "In Progress"] } } },
            ] })
        );
        assert_eq!(
            narrowed.to_value(false),
            json!({ "state": { "name": { "in": ["Done", "In Progress"] } } })
        );
    }

    #[test]
    fn priorities_go_as_the_numbers_linear_has_for_them() {
        let value = Filters::new(
            Vec::new(),
            vec!["urgent".to_owned(), "low".to_owned(), "none".to_owned()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("priorities")
        .to_value(false);
        assert_eq!(value, json!({ "priority": { "in": [1, 4, 0] } }));
    }

    #[test]
    fn what_is_not_a_shape_of_a_filter_is_not_taken() {
        let one = |word: &str| vec![word.to_owned()];
        let none = Vec::new;
        assert!(Filters::new(none(), one("soon"), none(), none(), none()).is_err());
        assert!(Filters::new(none(), none(), one("a b"), none(), none()).is_err());
        assert!(Filters::new(none(), none(), none(), none(), one("a/b")).is_err());
        assert!(Filters::new(one("In\nProgress"), none(), none(), none(), none()).is_err());
        assert!(Filters::new(none(), none(), none(), one(&"x".repeat(101)), none()).is_err());
        let many: Vec<String> = (0..51).map(|n| format!("label {n}")).collect();
        assert!(Filters::new(none(), none(), none(), many, none()).is_err());
        // A value given twice is one.
        let twice = Filters::new(
            none(),
            none(),
            none(),
            vec!["bug".into(), "bug".into()],
            none(),
        )
        .unwrap();
        assert_eq!(twice.labels, ["bug"]);
    }
}
