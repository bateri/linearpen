//! Names into what Linear takes: a team by its key or name, a status, a label, a person, a project, a
//! cycle or an estimate by the name it is shown with. The match is the whole name, whatever the
//! case. A name that is none of them, or is more than one, is not guessed: the command stops with the
//! names there are.

use super::{Access, Stop, asked_wrong};
use crate::command_line::CycleAsk;
use crate::linear::types::cycle_label;
use crate::linear::{self, CycleChoice, Entry, FilterOptions, Options, Team};

/// What a name that is none of the choices is said to be, where it is the workspace's.
pub const WORKSPACE: &str = "the workspace";

/// The most names told of what is not found; the rest is counted.
const TOLD_MOST: usize = 40;

/// What the commands ask Linear for to find a name, each once as it is first needed.
pub struct Lookups<'a> {
    access: &'a Access<'a>,
    teams: Option<Vec<Team>>,
    filter_options: Option<FilterOptions>,
    names: Option<Vec<Entry>>,
}

impl<'a> Lookups<'a> {
    pub fn new(access: &'a Access<'a>) -> Self {
        Self {
            access,
            teams: None,
            filter_options: None,
            names: None,
        }
    }

    fn teams(&mut self) -> Result<&[Team], Stop> {
        if self.teams.is_none() {
            let access = self.access;
            let teams = linear::teams(access.transport, access.key)
                .map_err(|error| asked_wrong(access, &error))?;
            self.teams = Some(teams);
        }
        Ok(self.teams.as_deref().unwrap_or_default())
    }

    /// The team the key or the name says.
    pub fn team(&mut self, asked: &str) -> Result<Team, Stop> {
        let teams = self.teams()?;
        let lower = asked.to_lowercase();
        let by_key: Vec<&Team> = teams
            .iter()
            .filter(|team| team.key.to_lowercase() == lower)
            .collect();
        let found: Vec<&Team> = if by_key.is_empty() {
            teams
                .iter()
                .filter(|team| team.name.to_lowercase() == lower)
                .collect()
        } else {
            by_key
        };
        match found.as_slice() {
            [one] => Ok((*one).clone()),
            [] => {
                let names: Vec<String> = teams
                    .iter()
                    .map(|team| format!("{} ({})", team.key, team.name))
                    .collect();
                Err(none_found(WORKSPACE, "team", asked, &names))
            }
            several => Err(Stop::no(format!(
                "{asked:?} is more than one team: {}",
                several
                    .iter()
                    .map(|team| format!("{} ({})", team.key, team.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }

    /// What the issues of `team` can be given.
    pub fn options(&self, team: &Team) -> Result<Options, Stop> {
        linear::options(self.access.transport, self.access.key, &team.id)
            .map_err(|error| asked_wrong(self.access, &error))
    }

    /// The states, the people and the labels of the whole workspace, which the lists are narrowed by.
    pub fn filter_options(&mut self) -> Result<&FilterOptions, Stop> {
        if self.filter_options.is_none() {
            let access = self.access;
            let options = linear::filter_options(access.transport, access.key)
                .map_err(|error| asked_wrong(access, &error))?;
            self.filter_options = Some(options);
        }
        self.filter_options
            .as_ref()
            .ok_or_else(|| Stop::no("Linear did not say what the filters offer"))
    }

    /// The lists there are by name: views and projects among them.
    pub fn names(&mut self) -> Result<&[Entry], Stop> {
        if self.names.is_none() {
            let access = self.access;
            let names = linear::names(access.transport, access.key)
                .map_err(|error| asked_wrong(access, &error))?;
            self.names = Some(names);
        }
        Ok(self.names.as_deref().unwrap_or_default())
    }
}

/// The words for a name that is none of `names`.
pub fn none_found(owner: &str, noun: &str, asked: &str, names: &[String]) -> Stop {
    if names.is_empty() {
        return Stop::no(format!("{owner} has no {noun} {asked:?}; it has none"));
    }
    let mut told = names
        .iter()
        .take(TOLD_MOST)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > TOLD_MOST {
        told.push_str(&format!(", and {} more", names.len() - TOLD_MOST));
        if owner != WORKSPACE {
            told.push_str(&format!(" (linearpen teams {owner} lists them all)"));
        }
    }
    Stop::no(format!("{owner} has no {noun} {asked:?}; it has {told}"))
}

/// The choice named `asked`: the one whose name is the same whatever the case; where several are, the
/// one that is the same in every letter, if there is one. `owner` is whose choices they are (`ACM`).
pub fn pick<T: Clone>(
    owner: &str,
    noun: &str,
    asked: &str,
    choices: &[(String, T)],
) -> Result<T, Stop> {
    let lower = asked.to_lowercase();
    let same: Vec<&(String, T)> = choices
        .iter()
        .filter(|(name, _)| name.to_lowercase() == lower)
        .collect();
    match same.as_slice() {
        [(_, value)] => Ok(value.clone()),
        [] => {
            let names: Vec<String> = choices.iter().map(|(name, _)| name.clone()).collect();
            Err(none_found(owner, noun, asked, &names))
        }
        several => {
            let exact: Vec<&&(String, T)> =
                several.iter().filter(|(name, _)| name == asked).collect();
            if let [(_, value)] = exact.as_slice() {
                return Ok(value.clone());
            }
            Err(Stop::no(format!(
                "{owner} has more than one {noun} {asked:?}: {}",
                several
                    .iter()
                    .map(|(name, _)| format!("{name:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
    }
}

/// The cycle of the team `owner` that `ask` says: the one running, or by number. The team's cycles are
/// the one running and the ones that come next.
pub fn cycle<'a>(
    owner: &str,
    ask: CycleAsk,
    options: &'a Options,
) -> Result<&'a CycleChoice, Stop> {
    if !options.cycles_enabled {
        return Err(Stop::no(format!("{owner} does not use cycles")));
    }
    let found = options.cycles.iter().find(|cycle| match ask {
        CycleAsk::Current => cycle.active,
        CycleAsk::Number(number) => cycle.number == number,
    });
    found.ok_or_else(|| {
        let has: Vec<String> = options
            .cycles
            .iter()
            .map(|cycle| {
                let label = cycle_label(cycle.number, cycle.name.as_deref());
                if cycle.active {
                    format!("{label} ({}, running)", cycle.number)
                } else {
                    format!("{label} ({})", cycle.number)
                }
            })
            .collect();
        let wanted = match ask {
            CycleAsk::Current => "no cycle running now".to_owned(),
            CycleAsk::Number(number) => format!("no cycle {number}"),
        };
        if has.is_empty() {
            Stop::no(format!("{owner} has no cycles running or to come"))
        } else {
            Stop::no(format!(
                "{owner} has {wanted}; it has {} (the one running and the ones to come)",
                has.join(", ")
            ))
        }
    })
}

/// The estimate `asked` says in the scale of the team `owner`: a label of the scale or its number of
/// points.
pub fn estimate(owner: &str, asked: &str, scale: &[(String, u32)]) -> Result<u32, Stop> {
    if scale.is_empty() {
        return Err(Stop::no(format!("{owner} does not estimate its issues")));
    }
    let lower = asked.to_lowercase();
    if let Some((_, points)) = scale
        .iter()
        .find(|(label, _)| label.to_lowercase() == lower)
    {
        return Ok(*points);
    }
    if let Ok(points) = asked.parse::<u32>()
        && scale.iter().any(|(_, value)| *value == points)
    {
        return Ok(points);
    }
    let sizes: Vec<String> = scale
        .iter()
        .map(|(label, points)| {
            if *label == points.to_string() {
                label.clone()
            } else {
                format!("{label} ({points})")
            }
        })
        .collect();
    Err(Stop::no(format!(
        "{owner} has no estimate {asked:?}; it estimates in {}",
        sizes.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choices(of: &[(&str, u32)]) -> Vec<(String, u32)> {
        of.iter()
            .map(|(name, id)| ((*name).to_owned(), *id))
            .collect()
    }

    #[test]
    fn a_name_is_the_whole_name_whatever_the_case() {
        let states = choices(&[("In Progress", 1), ("In Review", 2), ("Done", 3)]);
        assert_eq!(pick("ACM", "status", "in progress", &states).unwrap(), 1);
        assert_eq!(pick("ACM", "status", "DONE", &states).unwrap(), 3);
        // A part of a name is not the name.
        let stop = pick("ACM", "status", "In", &states).unwrap_err();
        assert_eq!(
            stop.words,
            "ACM has no status \"In\"; it has In Progress, In Review, Done"
        );
        assert_eq!(stop.status, 1);
    }

    #[test]
    fn a_name_that_is_more_than_one_is_not_guessed_unless_one_is_written_so() {
        let labels = choices(&[("bug", 1), ("Bug", 2), ("api", 3)]);
        assert_eq!(pick("ACM", "label", "bug", &labels).unwrap(), 1);
        assert_eq!(pick("ACM", "label", "Bug", &labels).unwrap(), 2);
        let stop = pick("ACM", "label", "BUG", &labels).unwrap_err();
        assert_eq!(
            stop.words,
            "ACM has more than one label \"BUG\": \"bug\", \"Bug\""
        );
    }

    #[test]
    fn a_long_list_of_names_is_counted_and_a_team_says_where_the_rest_is() {
        let many: Vec<(String, u32)> = (0..45).map(|n| (format!("label {n}"), n)).collect();
        let stop = pick("ACM", "label", "x", &many).unwrap_err();
        assert!(
            stop.words
                .contains("label 39, and 5 more (linearpen teams ACM lists them all)"),
            "{}",
            stop.words
        );
        let stop = pick(WORKSPACE, "label", "x", &many).unwrap_err();
        assert!(stop.words.ends_with("and 5 more"), "{}", stop.words);
        let none: Vec<(String, u32)> = Vec::new();
        assert_eq!(
            pick("ACM", "project", "x", &none).unwrap_err().words,
            "ACM has no project \"x\"; it has none"
        );
    }

    #[test]
    fn an_estimate_is_a_label_of_the_scale_or_its_points() {
        let shirts: Vec<(String, u32)> = [("XS", 1), ("S", 2), ("M", 3), ("L", 5)]
            .iter()
            .map(|(label, points)| ((*label).to_owned(), *points))
            .collect();
        assert_eq!(estimate("ACM", "m", &shirts).unwrap(), 3);
        assert_eq!(estimate("ACM", "5", &shirts).unwrap(), 5);
        assert_eq!(
            estimate("ACM", "XXL", &shirts).unwrap_err().words,
            "ACM has no estimate \"XXL\"; it estimates in XS (1), S (2), M (3), L (5)"
        );
        let points: Vec<(String, u32)> = [1, 2, 3].iter().map(|n| (n.to_string(), *n)).collect();
        assert_eq!(estimate("ACM", "2", &points).unwrap(), 2);
        assert!(
            estimate("ACM", "4", &points)
                .unwrap_err()
                .words
                .ends_with("estimates in 1, 2, 3")
        );
        assert_eq!(
            estimate("WEB", "3", &[]).unwrap_err().words,
            "WEB does not estimate its issues"
        );
    }
}
