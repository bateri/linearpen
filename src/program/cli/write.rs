//! The commands that write: `create`, `update` and `comment` (and their edits: `update --description`
//! writes a description whole, `comment --edit` the words of a comment of the person's). Each sends its
//! one write once, after every name is found out, and never again. Where the answer does not come it
//! says so with status 3.

use std::io::Read;

use super::json::{self, Added, Changed, Made};
use super::pictures::{self, with_words};
use super::resolve::{Lookups, cycle, estimate, pick};
use super::{Access, Stop, asked_wrong, json as print_json, written_wrong};
use crate::command_line::{Comment, Create, Edited, Fields, Setting, Update, Words};
use crate::linear::edit::{self, Target};
use crate::linear::images;
use crate::linear::limits::{BODY_MOST, DESCRIPTION_MOST, EDIT_MOST, LABELS_MOST};
use crate::linear::types::TeamRef;
use crate::linear::{
    self, Change, Changes, Created, Error, Issue, NewIssue, Options, Priority, Team,
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
            // More bytes than `most` characters can be is more characters than `most`, whatever
            // they are, and the last of them may be cut: it is too long, not other than text.
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) >= limit {
                return Err(Stop::usage(format!(
                    "--{flag} is from one to {most} characters"
                )));
            }
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

/// The issue where Linear has said nothing of whether it was made, as the page settles it
/// (`linear::made`): there, it was made; not there, it was not, and the failure is told with status 1;
/// the asking comes to nothing too, it is not known, status 3.
fn settle(access: &Access, id: &str, failure: &Error) -> Result<Created, Stop> {
    match linear::made(access.transport, access.key, id) {
        Ok(Some(created)) => Ok(created),
        Ok(None) => Err(Stop::no(format!(
            "{}; Linear does not have the issue, so it was not made",
            super::tidy(&failure.to_string())
        ))),
        Err(_) => Err(Stop {
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
    // The files are read and checked before Linear is asked anything, and what the description is with
    // their lines is held to its bound before any is sent.
    let files = pictures::read_files(&create.attach)?;
    let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
    if images::with_lines_most(description.as_deref().unwrap_or(""), &names) > DESCRIPTION_MOST {
        return Err(Stop::usage(format!(
            "the description with the lines of its files is over {DESCRIPTION_MOST} characters"
        )));
    }
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
    // The files go to Linear's storage once every name is found out, and their lines are put after
    // the words of the description.
    let lines = pictures::put(access, &files)?;
    let description = if lines.is_empty() {
        description
    } else {
        Some(images::with_lines(
            description.as_deref().unwrap_or(""),
            &lines,
        ))
    };
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
            if !super::unknown(&error) {
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
    let with = with_words(files.len());
    let with = if with.is_empty() {
        String::new()
    } else {
        format!("  {with}")
    };
    Ok(if created.url.is_empty() {
        format!("{identifier}{with}\n")
    } else {
        format!("{identifier}  {}{with}\n", super::plain(&created.url))
    })
}

/// The issue named `key` (its identifier, or Linear's id of it), as Linear has it.
fn issue_of(access: &Access, key: &str) -> Result<Issue, Stop> {
    linear::fetch_issue(access.transport, access.key, key).map_err(|error| {
        if linear::no_such_issue(&error) {
            Stop::no(format!("Linear has no issue {}", super::plain(key)))
        } else {
            asked_wrong(access, &error)
        }
    })
}

/// The issues named, as Linear has them, each once, and the one team they are of.
fn issues_of(access: &Access, keys: &[String]) -> Result<(Vec<Issue>, TeamRef), Stop> {
    let mut found: Vec<Issue> = Vec::new();
    for key in keys {
        let issue = issue_of(access, key)?;
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
    Ok((found, first))
}

/// Changes the issues; what is printed on stdout, and a note for stderr where Linear did not say how
/// an issue it changed is now.
pub fn update(
    access: &Access,
    update: &Update,
    stdin: &mut dyn Read,
) -> Result<(String, String), Stop> {
    if let Some(words) = &update.description {
        return describe(access, update, words, stdin).map(|out| (out, String::new()));
    }
    if !update.attach.is_empty() {
        return add_files(access, update).map(|out| (out, String::new()));
    }
    let (issues, team) = issues_of(access, &update.issues)?;
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
    let mut added: Vec<String> = Vec::new();
    for id in resolved.labels {
        if !added.contains(&id) {
            added.push(id);
        }
    }
    if added.len() > LABELS_MOST {
        return Err(Stop::usage(format!(
            "an issue is given at most {LABELS_MOST} labels"
        )));
    }
    changes.extend(added.into_iter().map(Change::AddLabel));
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
                    "linearpen: {} was changed; Linear did not say how it is now\n",
                    super::plain(&issue.identifier)
                )),
            }
        }
    }
    let out = if update.json {
        print_json(&Changed {
            issues: now.iter().map(json::IssueRow::of).collect(),
        })?
    } else {
        stated_lines(&now)
    };
    Ok((out, note))
}

/// Adds the files of `update` at the end of the description of its one issue. The files are read
/// and checked first (status 2), the issue is asked for so that one Linear does not have is told before
/// anything is put in storage, the files go to storage (status 1 if one cannot, nothing written),
/// and then the description is read whole from Linear again and sent with the lines after it, once:
/// `linear::add_files`, which the pane's `describe` is.
fn add_files(access: &Access, update: &Update) -> Result<String, Stop> {
    let files = pictures::read_files(&update.attach)?;
    let issue = issue_of(access, &update.issues[0])?;
    let lines = pictures::put(access, &files)?;
    let now = match linear::add_files(access.transport, access.key, &issue.id, &lines) {
        Ok(now) => now,
        Err(linear::Step::Read(error)) => {
            let why = asked_wrong(access, &error).words;
            return Err(Stop::no(format!(
                "the description of {} could not be read whole, so nothing was written ({why})",
                super::plain(&issue.identifier)
            )));
        }
        Err(linear::Step::TooLong) => {
            return Err(Stop::no(format!(
                "the description of {} would be too long with the files, so nothing was written",
                super::plain(&issue.identifier)
            )));
        }
        Err(linear::Step::Write(error)) => return Err(written_wrong(access, &error)),
    };
    if update.json {
        return print_json(&Changed {
            issues: vec![json::IssueRow::of(&now)],
        });
    }
    let line = stated_lines(std::slice::from_ref(&now));
    Ok(format!(
        "{}  {}\n",
        line.trim_end(),
        with_words(files.len())
    ))
}

/// What an edit that did not write says: what could not be read (status 1, nothing written), and a
/// write that failed as any write does.
fn edit_wrong(access: &Access, identifier: &str, step: edit::Step) -> Stop {
    match step {
        edit::Step::Read(linear::Error::Failed(why)) if why == edit::NOT_YOURS => {
            Stop::no("that comment is not yours: Linear lets only the one who wrote it edit it")
        }
        edit::Step::Read(linear::Error::Failed(why)) if why == edit::NOT_HERE => {
            Stop::no(format!("that comment is not on {identifier}"))
        }
        edit::Step::Read(error) => {
            let why = asked_wrong(access, &error).words;
            Stop::no(format!(
                "{identifier} could not be read again, so nothing was written ({why})"
            ))
        }
        // (The command line edits over whatever is there: it is not told.)
        edit::Step::Changed(_) => Stop::no("it was changed in Linear meanwhile"),
        edit::Step::Write(error) => written_wrong(access, &error),
    }
}

/// Writes the description of the one issue of `update`, whole, with the files at its end: the files
/// are read and checked first (status 2), the issue is asked for so that one Linear does not have is
/// told before anything is put in storage, the files go to storage, and the words are sent once in the
/// place of the description Linear holds (`linear::edit::overwrite`; nothing is sent where it holds them
/// already).
fn describe(
    access: &Access,
    update: &Update,
    words: &Words,
    stdin: &mut dyn Read,
) -> Result<String, Stop> {
    let text = words_of("description", words, EDIT_MOST, stdin)?;
    let files = pictures::read_files(&update.attach)?;
    let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
    if images::with_lines_most(&text, &names) > EDIT_MOST {
        return Err(Stop::usage(format!(
            "the description with the lines of its files is over {EDIT_MOST} characters"
        )));
    }
    let issue = issue_of(access, &update.issues[0])?;
    let identifier = super::plain(&issue.identifier);
    let lines = pictures::put(access, &files)?;
    let text = if lines.is_empty() {
        text
    } else {
        images::description_with(&text, &lines)
    };
    edit::overwrite(
        access.transport,
        access.key,
        &issue.id,
        &Target::Description,
        &text,
    )
    .map_err(|step| edit_wrong(access, &identifier, step))?;
    let now = linear::fetch_issue(access.transport, access.key, &issue.id).unwrap_or(issue);
    if update.json {
        return print_json(&Changed {
            issues: vec![json::IssueRow::of(&now)],
        });
    }
    let line = stated_lines(std::slice::from_ref(&now));
    let with = with_words(files.len());
    Ok(if with.is_empty() {
        format!("{}  description written\n", line.trim_end())
    } else {
        format!("{}  description written {with}\n", line.trim_end())
    })
}

/// The comment of the person's that `edited` names on `issue`: the last they wrote (of the first fifty,
/// which are what Linear tells with the issue), or the one by its id.
fn edited_id(issue: &Issue, edited: &Edited) -> Result<String, Stop> {
    match edited {
        Edited::Id(id) => Ok(id.clone()),
        Edited::Last => {
            let identifier = super::plain(&issue.identifier);
            let comments = issue.comments.as_ref();
            let mut mine: Vec<&linear::types::Comment> = comments
                .map(|comments| comments.nodes.iter().filter(|c| c.mine()).collect())
                .unwrap_or_default();
            mine.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let more = comments.is_some_and(|comments| comments.page_info.has_next_page);
            match mine.last() {
                Some(last) if !more => Ok(last.id.clone()),
                _ if more => Err(Stop::no(format!(
                    "{identifier} has more comments than Linear tells at once: give the id of yours (linearpen show {identifier} --json)"
                ))),
                _ => Err(Stop::no(format!(
                    "you have written no comment on {identifier}"
                ))),
            }
        }
    }
}

/// Puts the words of `comment` in the place of a comment of the person's: the issue is asked for (an
/// issue Linear does not have, or no comment of theirs, is told before anything is written), and the
/// words are sent once (`linear::edit::overwrite`, which writes only a comment Linear says is theirs).
fn edit_comment(
    access: &Access,
    comment: &Comment,
    edited: &Edited,
    stdin: &mut dyn Read,
) -> Result<String, Stop> {
    let words = comment
        .body
        .as_ref()
        .ok_or_else(|| Stop::usage("--edit needs --body"))?;
    let text = words_of("body", words, EDIT_MOST, stdin)?;
    let issue = issue_of(access, &comment.issue)?;
    let identifier = super::plain(&issue.identifier);
    let id = edited_id(&issue, edited)?;
    let target = Target::Comment(id.clone());
    edit::overwrite(access.transport, access.key, &issue.id, &target, &text)
        .map_err(|step| edit_wrong(access, &identifier, step))?;
    if comment.json {
        let (now, _) = edit::comment_of(access.transport, access.key, &issue.id, &id)
            .map_err(|error| asked_wrong(access, &error))?;
        return print_json(&Added {
            issue: identifier,
            comment: json::Said::of(&now),
        });
    }
    Ok(format!("{identifier}: comment edited\n"))
}

pub fn comment(access: &Access, comment: &Comment, stdin: &mut dyn Read) -> Result<String, Stop> {
    if let Some(edited) = &comment.edit {
        return edit_comment(access, comment, edited, stdin);
    }
    let body = comment
        .body
        .as_ref()
        .map(|words| words_of("body", words, BODY_MOST, stdin))
        .transpose()?;
    // The files are read and checked before Linear is asked anything, and what the comment is with their
    // lines is held to its bound before any is sent.
    let files = pictures::read_files(&comment.attach)?;
    let names: Vec<&str> = files.iter().map(|file| file.name.as_str()).collect();
    if images::with_lines_most(body.as_deref().unwrap_or(""), &names) > BODY_MOST {
        return Err(Stop::usage(format!(
            "the comment with the lines of its files is over {BODY_MOST} characters"
        )));
    }
    // The issue is asked for first, so that an issue Linear does not have is told before anything is
    // written, and the comment goes to the issue's own id, as the page sends it.
    let issue = issue_of(access, &comment.issue)?;
    let lines = pictures::put(access, &files)?;
    let text = images::with_lines(body.as_deref().unwrap_or(""), &lines);
    let id = draft::new_id().map_err(Stop::no)?;
    let posted = linear::comment(access.transport, access.key, &id, &issue.id, &text)
        .map_err(|error| written_wrong(access, &error))?;
    let identifier = super::plain(&issue.identifier);
    if comment.json {
        return print_json(&Added {
            issue: identifier,
            comment: json::Said::of(&posted),
        });
    }
    let with = with_words(files.len());
    Ok(if with.is_empty() {
        format!("{identifier}: comment added\n")
    } else {
        format!("{identifier}: comment added {with}\n")
    })
}
