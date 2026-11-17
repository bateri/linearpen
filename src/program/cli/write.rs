//! The commands that write: `create`, `update` and `comment`. Each sends its one write once, after
//! every name is found out, and never again. Where the answer does not come it says so with status 3.

use std::io::Read;

use super::json::{self, Added, Changed, Made};
use super::resolve::{Lookups, cycle, estimate, pick};
use super::{Access, Stop, asked_wrong, json as print_json, written_wrong};
use crate::command_line::{Comment, Create, Fields, Setting, Update, Words};
use crate::linear::limits::{BODY_MOST, DESCRIPTION_MOST, LABELS_MOST};
use crate::linear::{
    self, Change, Changes, Created, Error, Issue, NewIssue, Options, Priority, Team, left_open,
};
use crate::state::draft;
use crate::text::summary::stated_lines;

/// The words from stdin, or as they were given. A command that takes stdin reads it before it asks
/// Linear anything; what is not text, or is longer than `most` characters, or is only white space,
/// is not taken.
fn words_of(flag: &str, words: &Words, most: usize, stdin: &mut dyn Read) -> Result<String, Stop> {
    let text = match words {
        Words::Given(text) => text.clone(),
        Words::Stdin => {
            let mut bytes = Vec::new();
            // Four bytes to a character is as many as a text of `most` characters has, and one more
            // to tell that there is more.
            let limit = u64::try_from(most.saturating_mul(4).saturating_add(1)).unwrap_or(u64::MAX);
            stdin
                .take(limit)
                .read_to_end(&mut bytes)
                .map_err(|why| Stop::usage(format!("stdin could not be read: {why}")))?;
            String::from_utf8(bytes)
                .map_err(|_| Stop::usage(format!("the words of --{flag} on stdin are not text")))?
        }
    };
    let text = text.trim_end().to_owned();
    if text.trim().is_empty() || text.chars().count() > most {
        return Err(Stop::usage(format!(
            "--{flag} is from one to {most} characters"
        )));
    }
    Ok(text)
}

/// The id of the state, the person and the rest that `fields` name in the choices of `team`; what
/// `create` and `update` both resolve, as the changes of an issue.
struct Resolved {
    state: Option<String>,
    priority: Option<Priority>,
    assignee: Option<Option<String>>,
    labels: Vec<String>,
    project: Option<Option<String>>,
    cycle: Option<Option<String>>,
    estimate: Option<Option<u32>>,
    due: Option<Option<String>>,
}

fn resolve(team: &Team, options: &Options, fields: &Fields) -> Result<Resolved, Stop> {
    let owner = team.key.as_str();
    let state = fields
        .status
        .as_ref()
        .map(|asked| {
            let states: Vec<(String, String)> = options
                .all_states
                .iter()
                .map(|state| (state.name.clone(), state.id.clone()))
                .collect();
            pick(owner, "status", asked, &states)
        })
        .transpose()?;
    let assignee = fields
        .assignee
        .as_ref()
        .map(|asked| match asked {
            Setting::Clear => Ok(None),
            Setting::To(name) if name.eq_ignore_ascii_case(linear::ME) => options
                .members
                .iter()
                .find(|member| member.me)
                .map(|member| Some(member.id.clone()))
                .ok_or_else(|| Stop::no(format!("you are not a member of {owner}"))),
            Setting::To(name) => {
                let people: Vec<(String, String)> = options
                    .members
                    .iter()
                    .map(|member| (member.name.clone(), member.id.clone()))
                    .collect();
                pick(owner, "person", name, &people).map(Some)
            }
        })
        .transpose()?;
    let mut labels: Vec<String> = Vec::new();
    for asked in &fields.label {
        labels.push(label(owner, options, asked)?);
    }
    let project = fields
        .project
        .as_ref()
        .map(|asked| match asked {
            Setting::Clear => Ok(None),
            Setting::To(name) => {
                let projects: Vec<(String, String)> = options
                    .projects
                    .iter()
                    .map(|project| (project.name.clone(), project.id.clone()))
                    .collect();
                pick(owner, "project", name, &projects).map(Some)
            }
        })
        .transpose()?;
    let cycle_id = fields
        .cycle
        .as_ref()
        .map(|asked| match asked {
            Setting::Clear => Ok(None),
            Setting::To(ask) => cycle(owner, *ask, options).map(|found| Some(found.id.clone())),
        })
        .transpose()?;
    let points = fields
        .estimate
        .as_ref()
        .map(|asked| match asked {
            Setting::Clear => Ok(None),
            Setting::To(text) => {
                let scale: Vec<(String, u32)> = options
                    .estimates
                    .iter()
                    .map(|size| (size.label.clone(), size.value))
                    .collect();
                estimate(owner, text, &scale).map(Some)
            }
        })
        .transpose()?;
    let due = fields.due.as_ref().map(|asked| match asked {
        Setting::Clear => None,
        Setting::To(day) => Some(day.clone()),
    });
    Ok(Resolved {
        state,
        priority: fields.priority,
        assignee,
        labels,
        project,
        cycle: cycle_id,
        estimate: points,
        due,
    })
}

/// The id of the label of the team (or of the workspace) that is called `asked`.
fn label(owner: &str, options: &Options, asked: &str) -> Result<String, Stop> {
    let labels: Vec<(String, String)> = options
        .labels
        .iter()
        .map(|label| (label.name.clone(), label.id.clone()))
        .collect();
    pick(owner, "label", asked, &labels)
}

/// Makes the issue where Linear has said nothing of whether it was made: asks for it by the id of
/// the draft, which Linear took as the issue's own. That asking is a read, not a second write.
fn settle(access: &Access, id: &str, failure: &Error) -> Result<Created, Stop> {
    match linear::fetch_issue(access.transport, access.key, id) {
        Ok(issue) if issue.id.eq_ignore_ascii_case(id) => Ok(Created {
            id: issue.id,
            identifier: issue.identifier,
            url: issue.url,
        }),
        _ => Err(Stop {
            words: format!(
                "{}; it is not known whether Linear made the issue; look in Linear before running it again (the id of this draft is {id})",
                super::tidy(&failure.to_string())
            ),
            status: 3,
        }),
    }
}

pub fn create(access: &Access, create: &Create, stdin: &mut dyn Read) -> Result<String, Stop> {
    let description = create
        .description
        .as_ref()
        .map(|words| words_of("description", words, DESCRIPTION_MOST, stdin))
        .transpose()?;
    let mut lookups = Lookups::new(access);
    let team = lookups.team(&create.team)?;
    let options = lookups.options(&team)?;
    let resolved = resolve(&team, &options, &create.fields)?;
    let mut label_ids: Vec<String> = Vec::new();
    for id in resolved.labels {
        if !label_ids.contains(&id) {
            label_ids.push(id);
        }
    }
    if label_ids.len() > LABELS_MOST {
        return Err(Stop::usage(format!(
            "an issue is given at most {LABELS_MOST} labels"
        )));
    }
    let id = draft::new_id().map_err(Stop::no)?;
    let input = NewIssue {
        id: id.clone(),
        team_id: team.id.clone(),
        title: create.title.clone(),
        description,
        state_id: resolved.state,
        priority: resolved.priority.unwrap_or(Priority::None),
        assignee_id: resolved.assignee.flatten(),
        label_ids,
        project_id: resolved.project.flatten(),
        cycle_id: resolved.cycle.flatten(),
        estimate: resolved.estimate.flatten(),
        due_date: resolved.due.flatten(),
    };
    let created = match linear::create(access.transport, access.key, &input) {
        Ok(created) => created,
        Err(error) => {
            let open = match &error {
                Error::Unreachable(_) | Error::Unsure => true,
                Error::Failed(why) => left_open(why),
                Error::Refused | Error::Forbidden => false,
            };
            if !open {
                return Err(written_wrong(access, &error));
            }
            settle(access, &id, &error)?
        }
    };
    if create.json {
        return print_json(&Made {
            id: created.id,
            identifier: created.identifier,
            url: created.url,
        });
    }
    let identifier = super::plain(&created.identifier);
    Ok(if created.url.is_empty() {
        format!("{identifier}\n")
    } else {
        format!("{identifier}  {}\n", super::plain(&created.url))
    })
}

/// The issues named, as Linear has them, each once and of one team.
fn issues_of(access: &Access, keys: &[String]) -> Result<Vec<Issue>, Stop> {
    let mut found: Vec<Issue> = Vec::new();
    for key in keys {
        let issue =
            linear::fetch_issue(access.transport, access.key, key).map_err(
                |error| match &error {
                    Error::Failed(why)
                        if why.to_lowercase().contains("not found")
                            || why == "Linear has no such issue" =>
                    {
                        Stop::no(format!("Linear has no issue {key}"))
                    }
                    other => asked_wrong(access, other),
                },
            )?;
        if !found.iter().any(|kept| kept.id == issue.id) {
            found.push(issue);
        }
    }
    let team_of = |issue: &Issue| {
        issue.team.clone().ok_or_else(|| {
            Stop::no(format!(
                "Linear did not say which team {} is of",
                issue.identifier
            ))
        })
    };
    let first = team_of(&found[0])?;
    for issue in &found[1..] {
        let team = team_of(issue)?;
        if team.id != first.id {
            return Err(Stop::no(format!(
                "issues of one team are changed together: {} is of {}, {} is of {}",
                super::plain(&found[0].identifier),
                super::plain(&first.key),
                super::plain(&issue.identifier),
                super::plain(&team.key)
            )));
        }
    }
    Ok(found)
}

pub fn update(access: &Access, update: &Update) -> Result<String, Stop> {
    let issues = issues_of(access, &update.issues)?;
    let Some(team) = issues[0].team.clone() else {
        return Err(Stop::no("Linear did not say which team the issues are of"));
    };
    let team = Team {
        id: team.id,
        key: super::plain(&team.key),
        name: String::new(),
    };
    let lookups = Lookups::new(access);
    let options = lookups.options(&team)?;
    let resolved = resolve(&team, &options, &update.fields)?;
    let mut changes: Vec<Change> = Vec::new();
    if let Some(id) = resolved.state {
        changes.push(Change::State(id));
    }
    if let Some(priority) = resolved.priority {
        changes.push(Change::Priority(priority));
    }
    if let Some(who) = resolved.assignee {
        changes.push(Change::Assignee(who));
    }
    changes.extend(resolved.labels.into_iter().map(Change::AddLabel));
    for asked in &update.unlabel {
        changes.push(Change::RemoveLabel(label(&team.key, &options, asked)?));
    }
    if let Some(project) = resolved.project {
        changes.push(Change::Project(project));
    }
    if let Some(cycle_id) = resolved.cycle {
        changes.push(Change::Cycle(cycle_id));
    }
    if let Some(points) = resolved.estimate {
        changes.push(Change::Estimate(points));
    }
    if let Some(day) = resolved.due {
        changes.push(Change::DueDate(day));
    }
    if let Some(title) = &update.title {
        changes.push(Change::Title(title.clone()));
    }
    let changes = Changes::new(changes).map_err(|conflict| Stop::no(conflict.0))?;
    let ids: Vec<String> = issues.iter().map(|issue| issue.id.clone()).collect();
    let mut now = linear::change_all(access.transport, access.key, &ids, &changes)
        .map_err(|error| written_wrong(access, &error))?;
    // An issue the answer leaves out is asked for, so that what is printed is what Linear holds.
    let mut note = String::new();
    for issue in &issues {
        if !now.iter().any(|got| got.id.eq_ignore_ascii_case(&issue.id)) {
            match linear::fetch_issue(access.transport, access.key, &issue.id) {
                Ok(got) => now.push(got),
                Err(_) => note.push_str(&format!(
                    "{}  changed; Linear did not say how it is now\n",
                    super::plain(&issue.identifier)
                )),
            }
        }
    }
    if update.json {
        return print_json(&Changed {
            issues: now.iter().map(json::IssueRow::of).collect(),
        });
    }
    Ok(format!("{}{note}", stated_lines(&now)))
}

pub fn comment(access: &Access, comment: &Comment, stdin: &mut dyn Read) -> Result<String, Stop> {
    let body = words_of("body", &comment.body, BODY_MOST, stdin)?;
    let id = draft::new_id().map_err(Stop::no)?;
    let posted = linear::comment(access.transport, access.key, &id, &comment.issue, &body)
        .map_err(|error| written_wrong(access, &error))?;
    if comment.json {
        return print_json(&Added {
            issue: comment.issue.clone(),
            comment: json::Said::of(&posted),
        });
    }
    Ok(format!("{}: comment added\n", comment.issue))
}
