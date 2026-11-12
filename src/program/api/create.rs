//! The calls `new_options` and `create`: what a new issue can be given, and making it. What happens
//! is `linear/`'s and the session's; this reads what the page asks, checks it before anything is
//! sent, and tells what came.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::{Failure, call};
use crate::linear::{Created, Error, NewIssue, Priority, StateChoice, Team};
use crate::session::Session;
use crate::state::draft;

/// The longest title, in characters (Linear's own limit), the longest description, and the most
/// labels one issue is given.
pub(super) const TITLE_MOST: usize = 255;
const DESCRIPTION_MOST: usize = 50_000;
const LABELS_MOST: usize = 50;
/// The longest id or key taken (Linear's ids are UUIDs, 36 characters).
const REF_MOST: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Which {
    team: Option<String>,
}

#[derive(Debug, Serialize)]
struct TeamMessage {
    id: String,
    key: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct StateMessage {
    id: String,
    name: String,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct LabelMessage {
    id: String,
    name: String,
    color: String,
}

#[derive(Debug, Serialize)]
struct ProjectMessage {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct MemberMessage {
    id: String,
    name: String,
    me: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OptionsMessage {
    /// A new id for a draft; the page keeps the first it was given until the issue is made.
    draft: String,
    teams: Vec<TeamMessage>,
    /// The id of the team the choices are of.
    team: String,
    default_state: Option<String>,
    /// The open states: what a new issue begins in.
    states: Vec<StateMessage>,
    /// Every state an issue can be put in, the open ones and then done and canceled.
    all_states: Vec<StateMessage>,
    labels: Vec<LabelMessage>,
    projects: Vec<ProjectMessage>,
    members: Vec<MemberMessage>,
    /// Whether the team has cycles: where it has not, an issue of it is put in none.
    cycles_enabled: bool,
    /// The cycle that is running, then the next ones.
    cycles: Vec<CycleMessage>,
    /// The scale the team estimates by, smallest first; none where it does not estimate.
    estimates: Vec<EstimateMessage>,
}

/// Whether `text` is an id or a key of Linear: letters, digits, dashes and underscores.
pub(super) fn check_ref(what: &str, text: &str) -> Result<(), Failure> {
    if text.is_empty()
        || text.chars().count() > REF_MOST
        || !text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(Failure::BadParams(format!(
            "{what} is an id of Linear: up to {REF_MOST} letters, digits, dashes and underscores"
        )));
    }
    Ok(())
}

/// The team the page named (by id or by key, whatever the case), else the first.
fn team_of<'a>(teams: &'a [Team], named: Option<&str>) -> Option<&'a Team> {
    named
        .and_then(|named| {
            teams
                .iter()
                .find(|team| team.id == named || team.key.eq_ignore_ascii_case(named))
        })
        .or_else(|| teams.first())
}

#[derive(Debug, Serialize)]
struct CycleMessage {
    id: String,
    /// Its name, or `Cycle 24` where it has none.
    label: String,
    /// The cycle that is running now.
    active: bool,
}

/// A number of points the team estimates by, and what it calls it.
#[derive(Debug, Serialize)]
struct EstimateMessage {
    value: u32,
    label: String,
}

fn state_message(state: &StateChoice) -> StateMessage {
    StateMessage {
        id: state.id.clone(),
        name: state.name.clone(),
        kind: state.kind.name(),
    }
}

fn new_options(session: &Session, params: Which) -> Result<OptionsMessage, Failure> {
    if let Some(named) = &params.team {
        check_ref("team", named)?;
    }
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let teams = session.teams().map_err(Failure::Linear)?;
    let Some(team) = team_of(&teams, params.team.as_deref()) else {
        return Err(Failure::Linear(Error::Failed(
            "there is no team to make an issue in".to_owned(),
        )));
    };
    let options = session.options(&team.id).map_err(Failure::Linear)?;
    let draft = draft::new_id().map_err(|why| Failure::Linear(Error::Failed(why)))?;
    Ok(OptionsMessage {
        draft,
        team: team.id.clone(),
        teams: teams
            .iter()
            .map(|team| TeamMessage {
                id: team.id.clone(),
                key: team.key.clone(),
                name: team.name.clone(),
            })
            .collect(),
        default_state: options.default_state,
        states: options.states.iter().map(state_message).collect(),
        all_states: options.all_states.iter().map(state_message).collect(),
        labels: options
            .labels
            .into_iter()
            .map(|label| LabelMessage {
                id: label.id,
                name: label.name,
                color: label.color,
            })
            .collect(),
        projects: options
            .projects
            .into_iter()
            .map(|project| ProjectMessage {
                id: project.id,
                name: project.name,
            })
            .collect(),
        members: options
            .members
            .into_iter()
            .map(|member| MemberMessage {
                id: member.id,
                name: member.name,
                me: member.me,
            })
            .collect(),
        cycles_enabled: options.cycles_enabled,
        cycles: options
            .cycles
            .into_iter()
            .map(|cycle| CycleMessage {
                label: crate::linear::types::cycle_label(cycle.number, cycle.name.as_deref()),
                id: cycle.id,
                active: cycle.active,
            })
            .collect(),
        estimates: options
            .estimates
            .into_iter()
            .map(|estimate| EstimateMessage {
                value: estimate.value,
                label: estimate.label,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Make {
    id: String,
    team: String,
    title: String,
    description: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    assignee: Option<String>,
    labels: Option<Vec<String>>,
    project: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatedMessage {
    id: String,
    identifier: String,
}

/// The input of the mutation that the params make, or why they are not an issue.
fn input_of(params: Make) -> Result<NewIssue, Failure> {
    if !draft::is_uuid(&params.id) {
        return Err(Failure::BadParams(
            "id is the id of the draft, a UUID (new_options gives one)".to_owned(),
        ));
    }
    check_ref("team", &params.team)?;
    let title = params.title.trim();
    if title.is_empty() || title.chars().count() > TITLE_MOST {
        return Err(Failure::BadParams(format!(
            "title is one to {TITLE_MOST} characters"
        )));
    }
    if params
        .description
        .as_ref()
        .is_some_and(|text| text.chars().count() > DESCRIPTION_MOST)
    {
        return Err(Failure::BadParams(format!(
            "description is at most {DESCRIPTION_MOST} characters"
        )));
    }
    for (what, value) in [
        ("state", &params.state),
        ("assignee", &params.assignee),
        ("project", &params.project),
    ] {
        if let Some(value) = value {
            check_ref(what, value)?;
        }
    }
    let priority = match params.priority.as_deref() {
        None => Priority::None,
        Some(name) => Priority::parse(name).ok_or_else(|| {
            Failure::BadParams("priority is none, urgent, high, medium or low".to_owned())
        })?,
    };
    let labels = params.labels.unwrap_or_default();
    if labels.len() > LABELS_MOST {
        return Err(Failure::BadParams(format!(
            "labels are at most {LABELS_MOST} ids"
        )));
    }
    let mut label_ids: Vec<String> = Vec::new();
    for label in labels {
        check_ref("a label", &label)?;
        if !label_ids.contains(&label) {
            label_ids.push(label);
        }
    }
    Ok(NewIssue {
        id: params.id.to_ascii_lowercase(),
        team_id: params.team,
        title: title.to_owned(),
        // A description of nothing but white space is none.
        description: params
            .description
            .filter(|text| !text.trim().is_empty())
            .map(|text| text.trim_end().to_owned()),
        state_id: params.state,
        priority,
        assignee_id: params.assignee,
        label_ids,
        project_id: params.project,
    })
}

/// What a call that wrote tells of a failure: a key Linear does not take, and a network that failed,
/// are said as they are for every call; Linear not making the issue is `create-failed`.
fn told(error: Error) -> Failure {
    match error {
        Error::Refused | Error::Unreachable(_) => Failure::Linear(error),
        Error::Forbidden => Failure::CreateFailed(
            "Linear did not make it: this key cannot create issues".to_owned(),
        ),
        Error::Unsure => Failure::CreateFailed(
            "Linear did not answer whole, so it may have made the issue: press Create again to find out"
                .to_owned(),
        ),
        Error::Failed(why) => Failure::CreateFailed(format!(
            "Linear did not make it: {}",
            why.strip_prefix("Linear said: ").unwrap_or(&why)
        )),
    }
}

fn create(session: &Session, params: Make) -> Result<CreatedMessage, Failure> {
    let input = input_of(params)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let Created { id, identifier } = session.create(&input).map_err(told)?;
    Ok(CreatedMessage { id, identifier })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "new_options", new_options);
    call(options, session, "create", create)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRAFT: &str = "11111111-2222-4333-8444-555555555555";

    fn make() -> Make {
        Make {
            id: DRAFT.to_owned(),
            team: "team-acm".to_owned(),
            title: "Fix the login".to_owned(),
            description: None,
            state: None,
            priority: None,
            assignee: None,
            labels: None,
            project: None,
        }
    }

    #[test]
    fn what_is_empty_is_not_in_the_input() {
        let input = input_of(Make {
            title: "  Fix the login \n".to_owned(),
            description: Some("   \n".to_owned()),
            labels: Some(Vec::new()),
            ..make()
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            serde_json::json!({ "id": DRAFT, "teamId": "team-acm", "title": "Fix the login" })
        );
    }

    #[test]
    fn what_is_given_is_in_the_input_and_the_priority_is_a_number() {
        let input = input_of(Make {
            id: DRAFT.to_ascii_uppercase(),
            description: Some("The words\n\n".to_owned()),
            state: Some("s-todo".to_owned()),
            priority: Some("urgent".to_owned()),
            assignee: Some("u-ada".to_owned()),
            labels: Some(vec![
                "l-api".to_owned(),
                "l-bug".to_owned(),
                "l-api".to_owned(),
            ]),
            project: Some("p-1".to_owned()),
            ..make()
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(&input).unwrap(),
            serde_json::json!({
                "id": DRAFT, "teamId": "team-acm", "title": "Fix the login",
                "description": "The words", "stateId": "s-todo", "priority": 1,
                "assigneeId": "u-ada", "labelIds": ["l-api", "l-bug"], "projectId": "p-1"
            })
        );
    }

    #[test]
    fn what_is_not_an_issue_is_refused_by_name() {
        let refused = |params: Make| {
            assert!(
                matches!(input_of(params), Err(Failure::BadParams(_))),
                "refused"
            );
        };
        refused(Make {
            title: "  ".to_owned(),
            ..make()
        });
        refused(Make {
            title: "x".repeat(256),
            ..make()
        });
        refused(Make {
            id: "issue-412".to_owned(),
            ..make()
        });
        refused(Make {
            team: "has a space".to_owned(),
            ..make()
        });
        refused(Make {
            priority: Some("loud".to_owned()),
            ..make()
        });
        refused(Make {
            assignee: Some(String::new()),
            ..make()
        });
        refused(Make {
            description: Some("x".repeat(50_001)),
            ..make()
        });
        refused(Make {
            labels: Some((0..51).map(|n| format!("l-{n}")).collect()),
            ..make()
        });
        refused(Make {
            labels: Some(vec!["a b".to_owned()]),
            ..make()
        });
        assert!(
            input_of(Make {
                title: "x".repeat(255),
                ..make()
            })
            .is_ok()
        );
    }

    #[test]
    fn the_team_is_named_by_id_or_by_key_else_it_is_the_first() {
        let team = |id: &str, key: &str| Team {
            id: id.to_owned(),
            key: key.to_owned(),
            name: key.to_owned(),
        };
        let teams = [team("t1", "ACM"), team("t2", "WEB")];
        assert_eq!(team_of(&teams, Some("t2")).unwrap().key, "WEB");
        assert_eq!(team_of(&teams, Some("web")).unwrap().key, "WEB");
        assert_eq!(team_of(&teams, Some("nope")).unwrap().key, "ACM");
        assert_eq!(team_of(&teams, None).unwrap().key, "ACM");
        assert!(team_of(&[], None).is_none());
    }

    #[test]
    fn a_failure_of_linear_is_told_in_words_that_say_who_did_not() {
        let words = |failure: Failure| match failure {
            Failure::CreateFailed(words) => words,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            words(told(Error::Forbidden)),
            "Linear did not make it: this key cannot create issues"
        );
        assert!(
            words(told(Error::Unsure)).contains("may have made the issue"),
            "an answer that settled nothing is not said to be a failure"
        );
        assert_eq!(
            words(told(Error::Failed(
                "Linear said: title is too long".to_owned()
            ))),
            "Linear did not make it: title is too long"
        );
        assert!(matches!(
            told(Error::Refused),
            Failure::Linear(Error::Refused)
        ));
        assert!(matches!(
            told(Error::Unreachable("down".to_owned())),
            Failure::Linear(Error::Unreachable(_))
        ));
    }
}
