//! The call `change`: one field of one issue or of several, given a new value. What happens is
//! `linear/`'s and the session's; this reads what the page asks, checks it before anything is sent,
//! and tells which issues were changed and whether they are still in the list.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Deserializer, Serialize};

use super::create::{TITLE_MOST, check_ref};
use super::{Failure, call};
use crate::linear::{BATCH_MOST, Change, Error, Priority};
use crate::session::{Changed, Session};

/// A member that may be `null` (to no one, to no project), told from one that is not there.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    from: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(from).map(Some)
}

/// The issues, and the one field that is changed in them: exactly one of the members after `ids`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Edit {
    ids: Vec<String>,
    /// The id of a state.
    status: Option<String>,
    priority: Option<String>,
    /// The id of a person, or `null` for no one.
    #[serde(default, deserialize_with = "present")]
    assignee: Option<Option<String>>,
    /// The id of a label the issues get.
    add_label: Option<String>,
    /// The id of a label the issues lose.
    remove_label: Option<String>,
    /// The id of a project, or `null` for none.
    #[serde(default, deserialize_with = "present")]
    project: Option<Option<String>>,
    /// The id of a cycle, or `null` for none.
    #[serde(default, deserialize_with = "present")]
    cycle: Option<Option<String>>,
    /// Points, or `null` for none.
    #[serde(default, deserialize_with = "present")]
    estimate: Option<Option<f64>>,
    /// A day, `2026-11-15`, or `null` for none.
    #[serde(default, deserialize_with = "present")]
    due_date: Option<Option<String>>,
    title: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChangedMessage {
    id: String,
    /// The list the page is looking at holds the issue still.
    listed: bool,
}

#[derive(Debug, Serialize)]
struct ChangeMessage {
    issues: Vec<ChangedMessage>,
}

/// The most points an estimate can be: Linear's scales end far below it.
const POINTS_MOST: u32 = 1_000;

/// `points` as a whole number of points, from 0 to `POINTS_MOST`, or nothing.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_points(points: f64) -> Option<u32> {
    (points.fract() == 0.0 && (0.0..=f64::from(POINTS_MOST)).contains(&points))
        .then_some(points as u32)
}

/// The change the params name, and the issues, or why they are not one.
fn read(params: Edit) -> Result<(Vec<String>, Change), Failure> {
    let Edit {
        ids,
        status,
        priority,
        assignee,
        add_label,
        remove_label,
        project,
        cycle,
        estimate,
        due_date,
        title,
    } = params;
    let named = [
        status.is_some(),
        priority.is_some(),
        assignee.is_some(),
        add_label.is_some(),
        remove_label.is_some(),
        project.is_some(),
        cycle.is_some(),
        estimate.is_some(),
        due_date.is_some(),
        title.is_some(),
    ]
    .into_iter()
    .filter(|named| *named)
    .count();
    if named != 1 {
        return Err(Failure::BadParams(
            "one of status, priority, assignee, addLabel, removeLabel, project, cycle, estimate, dueDate and title is changed at a time"
                .to_owned(),
        ));
    }
    let mut kept: Vec<String> = Vec::new();
    for id in ids {
        check_ref("an id", &id)?;
        if !kept.contains(&id) {
            kept.push(id);
        }
    }
    if kept.is_empty() || kept.len() > BATCH_MOST {
        return Err(Failure::BadParams(format!(
            "ids are from one to {BATCH_MOST} issues"
        )));
    }
    let change = if let Some(id) = status {
        check_ref("status", &id)?;
        Change::State(id)
    } else if let Some(name) = priority {
        Change::Priority(Priority::parse(&name).ok_or_else(|| {
            Failure::BadParams("priority is none, urgent, high, medium or low".to_owned())
        })?)
    } else if let Some(who) = assignee {
        if let Some(id) = &who {
            check_ref("assignee", id)?;
        }
        Change::Assignee(who)
    } else if let Some(id) = add_label {
        check_ref("addLabel", &id)?;
        Change::AddLabel(id)
    } else if let Some(id) = remove_label {
        check_ref("removeLabel", &id)?;
        Change::RemoveLabel(id)
    } else if let Some(project) = project {
        if let Some(id) = &project {
            check_ref("project", id)?;
        }
        Change::Project(project)
    } else if let Some(cycle) = cycle {
        if let Some(id) = &cycle {
            check_ref("cycle", id)?;
        }
        Change::Cycle(cycle)
    } else if let Some(points) = estimate {
        Change::Estimate(match points {
            None => None,
            Some(points) => Some(whole_points(points).ok_or_else(|| {
                Failure::BadParams(format!(
                    "estimate is a whole number of points from 0 to {POINTS_MOST}, or null"
                ))
            })?),
        })
    } else if let Some(day) = due_date {
        match day {
            None => Change::DueDate(None),
            Some(day) => Change::due_date(&day).ok_or_else(|| {
                Failure::BadParams("dueDate is a day written 2026-11-15, or null".to_owned())
            })?,
        }
    } else {
        let text = title.unwrap_or_default();
        match Change::title(&text) {
            Some(Change::Title(one_line)) if one_line.chars().count() <= TITLE_MOST => {
                Change::Title(one_line)
            }
            _ => {
                return Err(Failure::BadParams(format!(
                    "title is one to {TITLE_MOST} characters"
                )));
            }
        }
    };
    Ok((kept, change))
}

/// The words of why a change was not made, for the person.
pub(super) fn said(error: &Error) -> String {
    match error {
        Error::Refused | Error::Unreachable(_) => error.to_string(),
        Error::Forbidden => "Linear did not change it: this key cannot change issues".to_owned(),
        Error::Unsure => "Linear did not answer whole, so it may have changed it".to_owned(),
        Error::Failed(why) => format!(
            "Linear did not change it: {}",
            why.strip_prefix("Linear said: ").unwrap_or(why)
        ),
    }
}

/// What a call that wrote tells of a failure: a key Linear does not take, and a network that failed,
/// are said as they are for every call; Linear not making the change is `change-failed`.
fn told(error: Error) -> Failure {
    match error {
        Error::Refused | Error::Unreachable(_) => Failure::Linear(error),
        other => Failure::ChangeFailed(said(&other)),
    }
}

fn change(session: &Session, params: Edit) -> Result<ChangeMessage, Failure> {
    let (ids, change) = read(params)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let changed = session.change(&ids, &change).map_err(told)?;
    Ok(ChangeMessage {
        issues: changed
            .into_iter()
            .map(|Changed { id, listed }| ChangedMessage { id, listed })
            .collect(),
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "change", change)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn edit(params: serde_json::Value) -> Result<(Vec<String>, Change), String> {
        let edit: Edit = super::super::params::parse(params)?;
        read(edit).map_err(|failure| format!("{failure:?}"))
    }

    #[test]
    fn each_field_is_one_change() {
        let ids = json!(["issue-1", "issue-2"]);
        let cases = [
            (
                json!({"ids": ids, "status": "s-done"}),
                Change::State("s-done".into()),
            ),
            (
                json!({"ids": ids, "priority": "high"}),
                Change::Priority(Priority::High),
            ),
            (
                json!({"ids": ids, "assignee": "u-ada"}),
                Change::Assignee(Some("u-ada".into())),
            ),
            (
                json!({"ids": ids, "assignee": null}),
                Change::Assignee(None),
            ),
            (
                json!({"ids": ids, "addLabel": "l-bug"}),
                Change::AddLabel("l-bug".into()),
            ),
            (
                json!({"ids": ids, "removeLabel": "l-bug"}),
                Change::RemoveLabel("l-bug".into()),
            ),
            (
                json!({"ids": ids, "project": "p-1"}),
                Change::Project(Some("p-1".into())),
            ),
            (json!({"ids": ids, "project": null}), Change::Project(None)),
            (
                json!({"ids": ids, "cycle": "c-24"}),
                Change::Cycle(Some("c-24".into())),
            ),
            (json!({"ids": ids, "cycle": null}), Change::Cycle(None)),
            (
                json!({"ids": ids, "estimate": 5}),
                Change::Estimate(Some(5)),
            ),
            (
                json!({"ids": ids, "estimate": 5.0}),
                Change::Estimate(Some(5)),
            ),
            (
                json!({"ids": ids, "estimate": 0}),
                Change::Estimate(Some(0)),
            ),
            (
                json!({"ids": ids, "estimate": null}),
                Change::Estimate(None),
            ),
            (
                json!({"ids": ids, "dueDate": "2026-11-15"}),
                Change::DueDate(Some("2026-11-15".into())),
            ),
            (json!({"ids": ids, "dueDate": null}), Change::DueDate(None)),
            (
                json!({"ids": ids, "title": "  A new\ntitle "}),
                Change::Title("A new title".into()),
            ),
        ];
        for (params, expected) in cases {
            let (got_ids, change) =
                edit(params.clone()).unwrap_or_else(|why| panic!("{params}: {why}"));
            assert_eq!(change, expected, "{params}");
            assert_eq!(got_ids, ["issue-1", "issue-2"]);
        }
    }

    #[test]
    fn what_is_not_one_field_of_some_issues_is_bad_params() {
        let many: Vec<String> = (0..51).map(|n| format!("issue-{n}")).collect();
        let bad = [
            // None, or two.
            json!({"ids": ["i"]}),
            json!({"ids": ["i"], "status": "s", "priority": "high"}),
            json!({"ids": ["i"], "addLabel": "l", "removeLabel": "l"}),
            // The issues.
            json!({"ids": [], "status": "s"}),
            json!({"ids": many, "status": "s"}),
            json!({"ids": ["a b"], "status": "s"}),
            json!({"status": "s"}),
            // The values.
            json!({"ids": ["i"], "status": ""}),
            json!({"ids": ["i"], "priority": "huge"}),
            json!({"ids": ["i"], "assignee": "x y"}),
            json!({"ids": ["i"], "cycle": "x y"}),
            json!({"ids": ["i"], "cycle": ""}),
            json!({"ids": ["i"], "estimate": -1}),
            json!({"ids": ["i"], "estimate": 2.5}),
            json!({"ids": ["i"], "estimate": 100_000}),
            json!({"ids": ["i"], "estimate": "5"}),
            json!({"ids": ["i"], "dueDate": "tomorrow"}),
            json!({"ids": ["i"], "dueDate": "2026-02-30"}),
            json!({"ids": ["i"], "dueDate": ""}),
            json!({"ids": ["i"], "cycle": "c", "estimate": 3}),
            json!({"ids": ["i"], "title": " \n "}),
            json!({"ids": ["i"], "title": "x".repeat(256)}),
            // What the call does not take: a description and a team are not changed.
            json!({"ids": ["i"], "description": "x"}),
            json!({"ids": ["i"], "team": "t"}),
        ];
        for params in bad {
            assert!(edit(params.clone()).is_err(), "{params}");
        }
        // Fifty is the most, and an id said twice is one.
        let fifty: Vec<String> = (0..50).map(|n| format!("issue-{n}")).collect();
        assert!(edit(json!({"ids": fifty, "status": "s"})).is_ok());
        let (ids, _) = edit(json!({"ids": ["i", "i"], "status": "s"})).unwrap();
        assert_eq!(ids, ["i"]);
    }

    #[test]
    fn linear_not_changing_is_change_failed_and_a_lost_answer_is_unreachable() {
        let name = |failure: Failure| match failure {
            Failure::ChangeFailed(words) => words,
            Failure::Linear(error) => error.kind().to_owned(),
            other => format!("{other:?}"),
        };
        assert_eq!(
            name(told(Error::Failed("Linear said: no such state".into()))),
            "Linear did not change it: no such state"
        );
        assert_eq!(name(told(Error::Unreachable("t".into()))), "unreachable");
        assert_eq!(name(told(Error::Refused)), "refused");
        assert!(name(told(Error::Forbidden)).contains("cannot change"));
    }
}
