//! The commands that only read: `list`, `show` and `teams`.

use std::fmt::Write as _;

use super::agents::{self, Working};
use super::json::{self, Issues, Shown};
use super::pictures::{self, Marked, Place};
use super::resolve::{Lookups, WORKSPACE, cycle, pick};
use super::{Access, Stop, asked_wrong, json as print_json};
use crate::command_line::{List, ListScope};
use crate::linear::types::{Neighbor, StateKind};
use crate::linear::{self, Closed, Filters, Issue, ME, NONE, Scope, lines, plain};
use crate::model::time::short_date;
use crate::text::summary::section;

/// What the person asked a list to be narrowed by, as Linear's names for them.
fn filters(lookups: &mut Lookups, list: &List) -> Result<Filters, Stop> {
    let mut status: Vec<String> = Vec::new();
    if !list.status.is_empty() {
        let states: Vec<(String, StateKind)> = lookups
            .filter_options()?
            .states
            .iter()
            .map(|state| (state.name.clone(), state.kind))
            .collect();
        for asked in &list.status {
            pick(WORKSPACE, "status", asked, &states)?;
            // The name as Linear spells it: the match Linear makes is the exact one.
            let lower = asked.to_lowercase();
            let name = states
                .iter()
                .find(|(name, _)| name == asked)
                .or_else(|| states.iter().find(|(name, _)| name.to_lowercase() == lower))
                .map_or_else(|| asked.clone(), |(name, _)| name.clone());
            status.push(name);
        }
    }
    let mut assignee: Vec<String> = Vec::new();
    for asked in &list.assignee {
        if asked.eq_ignore_ascii_case(ME) {
            assignee.push(ME.to_owned());
        } else if asked.eq_ignore_ascii_case(NONE) {
            assignee.push(NONE.to_owned());
        } else {
            let people: Vec<(String, String)> = lookups
                .filter_options()?
                .people
                .iter()
                .map(|person| (person.name.clone(), person.id.clone()))
                .collect();
            assignee.push(pick(WORKSPACE, "person", asked, &people)?);
        }
    }
    let mut labels: Vec<String> = Vec::new();
    if !list.label.is_empty() {
        let names: Vec<(String, String)> = lookups
            .filter_options()?
            .labels
            .iter()
            .map(|label| (label.name.clone(), label.name.clone()))
            .collect();
        for asked in &list.label {
            labels.push(pick(WORKSPACE, "label", asked, &names)?);
        }
    }
    let mut project: Vec<String> = Vec::new();
    for asked in &list.project {
        if asked.eq_ignore_ascii_case(NONE) {
            project.push(NONE.to_owned());
        } else {
            let projects: Vec<(String, String)> = lookups
                .names()?
                .iter()
                .filter_map(|entry| match &entry.scope {
                    Scope::Project(id) => Some((entry.title.clone(), id.clone())),
                    _ => None,
                })
                .collect();
            project.push(pick(WORKSPACE, "project", asked, &projects)?);
        }
    }
    Filters::new(
        status,
        list.priority.iter().map(|p| p.name().to_owned()).collect(),
        assignee,
        labels,
        project,
    )
    .map_err(|why| Stop::no(why.0))
}

/// The list, and what it is called.
fn scope_of(lookups: &mut Lookups, list: &List) -> Result<(Scope, String), Stop> {
    Ok(match &list.scope {
        ListScope::Mine => (Scope::Mine, "My issues".to_owned()),
        ListScope::All => (Scope::All, "All issues".to_owned()),
        ListScope::Search(words) => (
            Scope::Search(words.clone()),
            format!("Search {:?}", plain(words)),
        ),
        ListScope::View(asked) => {
            let views: Vec<(String, Scope)> = lookups
                .names()?
                .iter()
                .filter(|entry| matches!(entry.scope, Scope::View(_)))
                .map(|entry| (entry.title.clone(), entry.scope.clone()))
                .collect();
            let scope = pick(WORKSPACE, "view", asked, &views)?;
            let title = views
                .iter()
                .find(|(_, found)| *found == scope)
                .map_or_else(|| asked.clone(), |(title, _)| title.clone());
            (scope, title)
        }
        ListScope::Team(asked) => {
            let team = lookups.team(asked)?;
            match list.cycle {
                None => (Scope::Team(team.id.clone()), format!("{} issues", team.key)),
                Some(ask) => {
                    let options = lookups.options(&team)?;
                    let found = cycle(&team.key, ask, &options)?;
                    let label = linear::types::cycle_label(found.number, found.name.as_deref());
                    (
                        Scope::Cycle(found.id.clone()),
                        format!("{} {label}", team.key),
                    )
                }
            }
        }
    })
}

pub fn list(access: &Access, list: &List) -> Result<String, Stop> {
    let mut lookups = Lookups::new(access);
    let searching = matches!(list.scope, ListScope::Search(_));
    let filters = filters(&mut lookups, list)?;
    let (scope, title) = scope_of(&mut lookups, list)?;
    let fetched = linear::fetch(
        access.transport,
        access.key,
        &scope,
        &filters,
        Closed::Asked,
    )
    .map_err(|error| asked_wrong(access, &error))?;
    let agents = agents::by_issue(access.panes.as_ref());
    if list.json {
        return print_json(&Issues::of(&title, &fetched.listing, &agents));
    }
    let empty = if !filters.is_empty() {
        "No issue matches."
    } else if matches!(list.scope, ListScope::Mine) {
        "Nothing is assigned to you."
    } else if searching {
        "Nothing was found."
    } else {
        "There are no open issues."
    };
    let mut out = String::new();
    section(
        &mut out,
        &plain(&title),
        &fetched.listing,
        empty,
        &|issue| agents.get(&issue.identifier).map(Working::words),
    );
    // A list holds the open issues unless states are asked for; a search holds every state.
    if filters.status.is_empty() && !searching {
        out.push_str(
            "  Done and canceled issues are not listed: --status Done (or Canceled) lists them.\n",
        );
    }
    Ok(out)
}

/// A related issue as one line: identifier, state and title.
fn neighbor_line(out: &mut String, other: &Neighbor) {
    let _ = writeln!(
        out,
        "  {}  {}  {}",
        plain(&other.identifier),
        plain(&other.state.name),
        plain(&other.title)
    );
}

/// The words of a field that may be empty.
fn or_dash(text: Option<String>) -> String {
    text.filter(|text| !text.is_empty())
        .unwrap_or_else(|| "-".to_owned())
}

/// The list that ends the text of an issue with files: the number, where it stands, and where it is
/// (the file where `--files` had it, why not where it could not be had, else its address).
fn file_list(out: &mut String, marked: &Marked) {
    out.push_str("\nFiles\n");
    for one in &marked.seen {
        let place = match &one.place {
            Place::Description => "description".to_owned(),
            Place::Comment { author, .. } => format!("comment by {author}"),
        };
        let at = match (&one.path, one.refused) {
            (Some(path), _) => plain(&path.display().to_string()),
            (None, Some(why)) => format!("{}  {}", why.words(), plain(&one.src)),
            (None, None) => plain(&one.src),
        };
        let _ = writeln!(out, "  {}  {place}  {at}", one.n);
    }
}

/// An issue as text: its fields (with the agent at work on it, where there is one), its description,
/// the issues around it and its comments, and the pictures and files they hold (`marked`: put where
/// they stand as `[image N: name]` or `[file N: name]`, and listed at the end).
fn shown(issue: &Issue, marked: &Marked, saved: bool, agent: Option<&Working>) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}  {}\n{}\n",
        plain(&issue.identifier),
        plain(&issue.title),
        plain(&issue.url)
    );
    let mut field = |name: &str, value: String| {
        let _ = writeln!(out, "{name:<10}{value}");
    };
    field("Status", plain(&issue.state.name));
    field(
        "Priority",
        crate::text::summary::priority_words(issue.priority).to_owned(),
    );
    field(
        "Assignee",
        or_dash(issue.assignee.as_ref().map(|who| plain(&who.display_name))),
    );
    field(
        "Labels",
        or_dash(Some(
            issue
                .labels
                .iter()
                .map(|label| plain(&label.name))
                .collect::<Vec<_>>()
                .join(", "),
        )),
    );
    field(
        "Team",
        or_dash(issue.team.as_ref().map(|team| plain(&team.key))),
    );
    field(
        "Project",
        or_dash(issue.project.as_ref().map(|project| plain(&project.name))),
    );
    field(
        "Cycle",
        or_dash(issue.cycle.as_ref().map(|cycle| plain(&cycle.label()))),
    );
    field(
        "Estimate",
        or_dash(issue.estimate.map(|points| points.to_string())),
    );
    field("Due", or_dash(issue.due_date.as_deref().map(plain)));
    field("Branch", or_dash(issue.branch_name.as_deref().map(plain)));
    field("Updated", short_date(&issue.updated_at));
    if let Some(agent) = agent {
        field(
            "Agent",
            format!(
                "{}: linearpen agent {} goes there",
                agent.words(),
                plain(&issue.identifier)
            ),
        );
    }
    if let Some(description) = marked
        .description
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        out.push_str("\nDescription\n");
        for line in lines(description.trim_end()).lines() {
            let _ = writeln!(out, "  {line}");
        }
    }
    if let Some(parent) = &issue.parent {
        out.push_str("\nPart of\n");
        neighbor_line(&mut out, parent);
    }
    if !issue.children.is_empty() {
        out.push_str("\nSub-issues\n");
        for child in &issue.children {
            neighbor_line(&mut out, child);
        }
    }
    let ties = issue.ties();
    let mut last = None;
    for (tie, other) in ties {
        if last != Some(tie) {
            let _ = writeln!(out, "\n{}", tie.label());
            last = Some(tie);
        }
        neighbor_line(&mut out, other);
    }
    if let Some(comments) = &issue.comments {
        let _ = writeln!(out, "\nComments ({})", comments.nodes.len());
        for (comment, body) in comments.nodes.iter().zip(&marked.comments) {
            let who = comment
                .user
                .as_ref()
                .map_or_else(|| "Someone".to_owned(), |who| plain(&who.display_name));
            let edited = if comment.edited_at.is_some() {
                " (edited)"
            } else {
                ""
            };
            let _ = writeln!(out, "  {who}, {}{edited}", short_date(&comment.created_at));
            for line in lines(body.trim_end()).lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
        if comments.page_info.has_next_page {
            let _ = writeln!(
                out,
                "  Only the first {} comments are shown; there are more in Linear.",
                comments.nodes.len()
            );
        }
    }
    if !marked.seen.is_empty() {
        file_list(&mut out, marked);
        if !saved {
            let count = marked.seen.len();
            let _ = writeln!(
                out,
                "\n{count} file{}: linearpen show {} --files DIR saves them",
                if count == 1 { "" } else { "s" },
                plain(&issue.identifier)
            );
        }
    }
    out
}

/// An issue, printed. With `files` (a folder) the files of it that are on Linear are saved there
/// and told by their paths.
pub fn show(
    access: &Access,
    identifier: &str,
    files: Option<&str>,
    json: bool,
) -> Result<String, Stop> {
    let issue = fetched(access, identifier)?;
    let mut marked = pictures::mark(&issue);
    if let Some(dir) = files {
        pictures::save(access, std::path::Path::new(dir), &mut marked.seen)?;
    }
    let agents = agents::by_issue(access.panes.as_ref());
    if json {
        return print_json(&Shown::of(&issue, &marked.seen, &agents));
    }
    Ok(shown(
        &issue,
        &marked,
        files.is_some(),
        agents.get(&issue.identifier),
    ))
}

/// The issue `identifier` (or Linear's id of it), as Linear has it.
pub fn fetched(access: &Access, identifier: &str) -> Result<Issue, Stop> {
    linear::fetch_issue(access.transport, access.key, identifier).map_err(|error| {
        if linear::no_such_issue(&error) {
            Stop::no(format!("Linear has no issue {identifier}"))
        } else {
            asked_wrong(access, &error)
        }
    })
}

pub fn teams(access: &Access, team: Option<&str>, json: bool) -> Result<String, Stop> {
    let mut lookups = Lookups::new(access);
    let Some(asked) = team else {
        let teams = linear::teams(access.transport, access.key)
            .map_err(|error| asked_wrong(access, &error))?;
        if json {
            return print_json(
                &teams
                    .iter()
                    .map(|team| json::TeamRow {
                        key: team.key.clone(),
                        name: team.name.clone(),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        let widest = teams
            .iter()
            .map(|team| team.key.chars().count())
            .max()
            .unwrap_or(0);
        let mut out = String::new();
        for team in &teams {
            let _ = writeln!(out, "{:<widest$}  {}", team.key, team.name);
        }
        return Ok(out);
    };
    let team = lookups.team(asked)?;
    let options = lookups.options(&team)?;
    let scale: Vec<(String, u32)> = options
        .estimates
        .iter()
        .map(|size| (size.label.clone(), size.value))
        .collect();
    if json {
        return print_json(&json::TeamNames {
            key: team.key.clone(),
            name: team.name.clone(),
            statuses: options
                .all_states
                .iter()
                .map(|state| json::State {
                    name: state.name.clone(),
                    kind: state.kind.name(),
                })
                .collect(),
            labels: options
                .labels
                .iter()
                .map(|label| label.name.clone())
                .collect(),
            people: options
                .members
                .iter()
                .map(|member| json::Person {
                    name: member.name.clone(),
                    me: member.me,
                })
                .collect(),
            projects: options
                .projects
                .iter()
                .map(|project| project.name.clone())
                .collect(),
            cycles: options
                .cycles
                .iter()
                .map(|cycle| json::CycleRow {
                    number: cycle.number,
                    label: linear::types::cycle_label(cycle.number, cycle.name.as_deref()),
                    running: cycle.active,
                })
                .collect(),
            estimates: scale
                .iter()
                .map(|(label, points)| json::Size {
                    label: label.clone(),
                    points: *points,
                })
                .collect(),
        });
    }
    let mut out = format!("{}  {}\n", team.key, team.name);
    let mut part = |title: &str, items: Vec<String>, none: &str| {
        let _ = writeln!(out, "\n{title}");
        if items.is_empty() {
            let _ = writeln!(out, "  {none}");
        }
        for item in items {
            let _ = writeln!(out, "  {item}");
        }
    };
    part(
        "Statuses",
        options
            .all_states
            .iter()
            .map(|state| state.name.clone())
            .collect(),
        "none",
    );
    part(
        "Labels",
        options
            .labels
            .iter()
            .map(|label| label.name.clone())
            .collect(),
        "none",
    );
    part(
        "People",
        options
            .members
            .iter()
            .map(|member| {
                if member.me {
                    format!("{} (you: me)", member.name)
                } else {
                    member.name.clone()
                }
            })
            .collect(),
        "none",
    );
    part(
        "Projects",
        options
            .projects
            .iter()
            .map(|project| project.name.clone())
            .collect(),
        "none",
    );
    part(
        "Cycles (by number, or current)",
        options
            .cycles
            .iter()
            .map(|found| {
                let label = linear::types::cycle_label(found.number, found.name.as_deref());
                if found.active {
                    format!("{}  {label}  (running)", found.number)
                } else {
                    format!("{}  {label}", found.number)
                }
            })
            .collect(),
        if options.cycles_enabled {
            "none running or to come"
        } else {
            "the team does not use cycles"
        },
    );
    part(
        "Estimates",
        scale
            .iter()
            .map(|(label, points)| {
                if *label == points.to_string() {
                    label.clone()
                } else {
                    format!("{label} ({points})")
                }
            })
            .collect(),
        "the team does not estimate",
    );
    Ok(out)
}
