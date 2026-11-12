//! What starting an agent may do to the issues in Linear, when the person chose it: move those that
//! have not begun to their team's first state of work in progress. Only forward (an issue that has
//! begun, is done or is canceled is left as it is) and only the state: this is the plan of it, from
//! the issues as Linear says they are now. Sending it is the session's `change`.

use crate::linear::types::StateKind;
use crate::linear::{Issue, Options, StateChoice};

/// The issues of one team that move, by Linear's id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub team: String,
    /// The key its identifiers begin with (`ACM`), for the words of why the step was not made.
    pub key: String,
    pub ids: Vec<String>,
}

/// Whether an issue in a state of this kind has not begun: what a start moves.
#[must_use]
pub fn not_begun(kind: StateKind) -> bool {
    matches!(
        kind,
        StateKind::Triage | StateKind::Backlog | StateKind::Unstarted
    )
}

/// What moves, a step for each team (in the order the teams first come): the issues that have not
/// begun. An issue with no team cannot be told where to go and stays.
#[must_use]
pub fn steps(issues: &[Issue]) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    for issue in issues {
        let Some(team) = issue.team.as_ref().filter(|_| not_begun(issue.state.kind)) else {
            continue;
        };
        match steps.iter_mut().find(|step| step.team == team.id) {
            Some(step) => {
                if !step.ids.contains(&issue.id) {
                    step.ids.push(issue.id.clone());
                }
            }
            None => steps.push(Step {
                team: team.id.clone(),
                key: team.key.clone(),
                ids: vec![issue.id.clone()],
            }),
        }
    }
    steps
}

/// The first state of work in progress of a team: the open states come in the order of their kinds
/// and, within a kind, by position, so it is the first `started` one.
#[must_use]
pub fn first_started(options: &Options) -> Option<&StateChoice> {
    options
        .states
        .iter()
        .find(|state| state.kind == StateKind::Started)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::fixture_issue;

    fn issue(id: &str, team: Option<&str>, kind: StateKind) -> Issue {
        let mut issue = fixture_issue();
        issue.id = id.to_owned();
        issue.state.kind = kind;
        issue.team = team.map(|id| crate::linear::types::TeamRef {
            id: id.to_owned(),
            key: id.to_uppercase(),
        });
        issue
    }

    #[test]
    fn only_what_has_not_begun_moves() {
        let all = [
            ("a", StateKind::Triage, true),
            ("b", StateKind::Backlog, true),
            ("c", StateKind::Unstarted, true),
            ("d", StateKind::Started, false),
            ("e", StateKind::Completed, false),
            ("f", StateKind::Canceled, false),
            ("g", StateKind::Other, false),
        ];
        for (id, kind, moves) in all {
            let planned = steps(&[issue(id, Some("t"), kind)]);
            assert_eq!(!planned.is_empty(), moves, "{id}: {kind:?}");
        }
    }

    #[test]
    fn a_step_is_a_team_and_the_issues_of_it_in_order() {
        let planned = steps(&[
            issue("a", Some("t1"), StateKind::Unstarted),
            issue("b", Some("t2"), StateKind::Backlog),
            issue("done", Some("t1"), StateKind::Completed),
            issue("c", Some("t1"), StateKind::Triage),
            issue("a", Some("t1"), StateKind::Unstarted),
        ]);
        assert_eq!(
            planned,
            [
                Step {
                    team: "t1".into(),
                    key: "T1".into(),
                    ids: vec!["a".into(), "c".into()]
                },
                Step {
                    team: "t2".into(),
                    key: "T2".into(),
                    ids: vec!["b".into()]
                },
            ]
        );
    }

    #[test]
    fn an_issue_with_no_team_stays_where_it_is() {
        assert!(steps(&[issue("a", None, StateKind::Backlog)]).is_empty());
    }

    #[test]
    fn nothing_is_planned_for_no_issues() {
        assert!(steps(&[]).is_empty());
    }

    fn choice(id: &str, kind: StateKind) -> StateChoice {
        StateChoice {
            id: id.to_owned(),
            name: id.to_owned(),
            kind,
        }
    }

    fn options(states: Vec<StateChoice>) -> Options {
        Options {
            team: "t".into(),
            default_state: None,
            all_states: states.clone(),
            states,
            labels: vec![],
            projects: vec![],
            members: vec![],
            cycles_enabled: false,
            cycles: vec![],
            estimates: vec![],
        }
    }

    #[test]
    fn the_first_state_of_work_is_the_first_started_one() {
        let got = options(vec![
            choice("back", StateKind::Backlog),
            choice("todo", StateKind::Unstarted),
            choice("doing", StateKind::Started),
            choice("review", StateKind::Started),
        ]);
        assert_eq!(first_started(&got).map(|s| s.id.as_str()), Some("doing"));
        let none = options(vec![choice("todo", StateKind::Unstarted)]);
        assert!(first_started(&none).is_none());
    }
}
