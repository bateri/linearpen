//! The commands of the command line, as typed values: what each is asked to do, in the words the
//! person wrote (a name is a name, not yet an id: `cli/` finds out what it names). What can be told
//! wrong without asking Linear is refused here.

use super::scan::{Scanned, many, one, scan, switch};
use crate::linear::images::SEND_MOST;
use crate::linear::limits::{BODY_MOST, DESCRIPTION_MOST, EDIT_MOST, TITLE_MOST, is_ref};
use crate::linear::{BATCH_MOST, Change, Priority};
use crate::state::draft;

/// The commands, by name.
pub const NAMES: [&str; 7] = [
    "list", "show", "create", "update", "comment", "teams", "agent",
];

/// Words the person gave as an option's value, or asked to be read from stdin (`-`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Words {
    Given(String),
    Stdin,
}

/// A value of a field that can also be taken away (`none`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setting<T> {
    Clear,
    To(T),
}

/// The cycle asked for: the one that is running, or by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleAsk {
    Current,
    Number(u32),
}

/// The list that is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListScope {
    Mine,
    All,
    /// By key or by name.
    Team(String),
    /// By name.
    View(String),
    Search(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub scope: ListScope,
    /// Only with a team.
    pub cycle: Option<CycleAsk>,
    pub status: Vec<String>,
    pub priority: Vec<Priority>,
    /// `me`, `none` or a name.
    pub assignee: Vec<String>,
    pub label: Vec<String>,
    /// `none` or a name.
    pub project: Vec<String>,
    pub json: bool,
}

/// The values both `create` and `update` give an issue.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fields {
    pub status: Option<String>,
    pub priority: Option<Priority>,
    pub assignee: Option<Setting<String>>,
    pub label: Vec<String>,
    pub project: Option<Setting<String>>,
    pub cycle: Option<Setting<CycleAsk>>,
    /// A label of the team's scale or a number of points.
    pub estimate: Option<Setting<String>>,
    /// A day, `2026-11-20`, checked.
    pub due: Option<Setting<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Create {
    /// By key or by name.
    pub team: String,
    /// One line, as `Change::title` makes it.
    pub title: String,
    pub description: Option<Words>,
    pub fields: Fields,
    /// Paths of the files put at the end of the description, in the order given (`--attach`).
    pub attach: Vec<String>,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    /// Identifiers, upper case, each once: from one to [`BATCH_MOST`].
    pub issues: Vec<String>,
    pub title: Option<String>,
    pub fields: Fields,
    /// Names of labels the issues lose.
    pub unlabel: Vec<String>,
    /// The words the description of the one issue is made, whole: with `attach` (whose files go at
    /// their end) the whole of the update.
    pub description: Option<Words>,
    /// Paths of files added at the end of the description of the one issue, which is then the whole
    /// of the update.
    pub attach: Vec<String>,
    pub json: bool,
}

/// The comment of the person's that is edited: the last they wrote on the issue, or one by its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edited {
    Last,
    Id(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub issue: String,
    /// Absent where there are files and no words.
    pub body: Option<Words>,
    pub attach: Vec<String>,
    /// Where it is given, the words are put in the place of a comment of the person's (`--edit`): no
    /// comment is posted, and no file goes with it.
    pub edit: Option<Edited>,
    pub json: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    List(Box<List>),
    Show {
        issue: String,
        /// The folder the files of the issue are saved in (`--files`).
        files: Option<String>,
        json: bool,
    },
    Create(Box<Create>),
    Update(Box<Update>),
    Comment(Comment),
    /// Without a key: the teams; with one: what that team's issues can be given.
    Teams {
        team: Option<String>,
        json: bool,
    },
    /// The keyboard to the pane of the agent at work on the issue.
    Agent {
        issue: String,
    },
}

/// What the words after a command's name come to.
#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    Do(Command),
    Say(String),
    Refuse(String),
}

/// The identifier `text` is (`acm-412` is `ACM-412`), or nothing: letters and digits that begin with a
/// letter, a dash, and a number; or Linear's own id of an issue, which is a UUID, in lower case (it is
/// what an agent started on an issue is told to give).
fn issue_key(text: &str) -> Option<String> {
    if draft::is_uuid(text) {
        return Some(text.to_ascii_lowercase());
    }
    let (team, number) = text.split_once('-')?;
    let fine = team.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && team.chars().all(|c| c.is_ascii_alphanumeric())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && text.len() <= 32;
    fine.then(|| text.to_ascii_uppercase())
}

/// A value that is not empty.
fn named<'a>(flag: &str, value: &'a str) -> Result<&'a str, String> {
    if value.trim().is_empty() {
        Err(format!("--{flag} needs a value"))
    } else {
        Ok(value.trim())
    }
}

fn named_all(flag: &str, values: Vec<String>) -> Result<Vec<String>, String> {
    values
        .iter()
        .map(|value| named(flag, value).map(str::to_owned))
        .collect()
}

fn priority(flag: &str, value: &str) -> Result<Priority, String> {
    Priority::parse(&value.trim().to_ascii_lowercase()).ok_or_else(|| {
        format!("--{flag} is none, urgent, high, medium or low, and {value:?} is not one")
    })
}

fn is_none(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case("none")
}

/// `none`, or a name.
fn setting(flag: &str, value: &str) -> Result<Setting<String>, String> {
    if is_none(value) {
        Ok(Setting::Clear)
    } else {
        named(flag, value).map(|name| Setting::To(name.to_owned()))
    }
}

fn cycle(value: &str, may_clear: bool) -> Result<Setting<CycleAsk>, String> {
    let word = value.trim().to_ascii_lowercase();
    if word == "current" {
        return Ok(Setting::To(CycleAsk::Current));
    }
    if word == "none" && may_clear {
        return Ok(Setting::Clear);
    }
    if !word.is_empty()
        && word.chars().all(|c| c.is_ascii_digit())
        && let Ok(number) = word.parse::<u32>()
    {
        return Ok(Setting::To(CycleAsk::Number(number)));
    }
    Err(format!(
        "--cycle is current, the number of a cycle{}, and {value:?} is none of them",
        if may_clear { " or none" } else { "" }
    ))
}

fn due(value: &str) -> Result<Setting<String>, String> {
    if is_none(value) {
        return Ok(Setting::Clear);
    }
    match Change::due_date(value.trim()) {
        Some(Change::DueDate(Some(day))) => Ok(Setting::To(day)),
        _ => Err(format!(
            "--due is a day written 2026-11-20, or none, and {value:?} is not one"
        )),
    }
}

fn title(value: &str) -> Result<String, String> {
    match Change::title(value) {
        Some(Change::Title(one_line)) if one_line.chars().count() <= TITLE_MOST => Ok(one_line),
        _ => Err(format!("--title is one to {TITLE_MOST} characters")),
    }
}

/// Words given, or `-` for stdin; what is given is held to `most` characters.
fn words(flag: &str, value: &str, most: usize) -> Result<Words, String> {
    if value == "-" {
        return Ok(Words::Stdin);
    }
    if value.trim().is_empty() || value.chars().count() > most {
        return Err(format!("--{flag} is from one to {most} characters"));
    }
    Ok(Words::Given(value.to_owned()))
}

/// The fields of the options `status` and the rest, which `create` and `update` both have.
fn fields(found: &Scanned) -> Result<Fields, String> {
    Ok(Fields {
        status: found
            .value("status")
            .map(|value| named("status", value).map(str::to_owned))
            .transpose()?,
        priority: found
            .value("priority")
            .map(|value| priority("priority", value))
            .transpose()?,
        assignee: found
            .value("assignee")
            .map(|value| setting("assignee", value))
            .transpose()?,
        label: named_all("label", found.values("label"))?,
        project: found
            .value("project")
            .map(|value| setting("project", value))
            .transpose()?,
        cycle: found
            .value("cycle")
            .map(|value| cycle(value, true))
            .transpose()?,
        estimate: found
            .value("estimate")
            .map(|value| setting("estimate", value))
            .transpose()?,
        due: found.value("due").map(due).transpose()?,
    })
}

fn list(args: &[String]) -> Result<Read, String> {
    let spec = [
        switch("all"),
        one("team"),
        one("view"),
        one("search"),
        one("cycle"),
        many("status"),
        many("priority"),
        many("assignee"),
        many("label"),
        many("project"),
        switch("json"),
    ];
    let found = scan("list", args, &spec)?;
    if found.help {
        return Ok(Read::Say(super::help::LIST.to_owned()));
    }
    if let Some(extra) = found.positional.first() {
        return Err(format!(
            "list takes no word but its options, and {extra:?} is not one"
        ));
    }
    let scopes = ["all", "team", "view", "search"]
        .into_iter()
        .filter(|name| found.has(name))
        .count();
    if scopes > 1 {
        return Err(
            "--all, --team, --view and --search choose the list, and only one of them is given"
                .to_owned(),
        );
    }
    let scope = if found.has("all") {
        ListScope::All
    } else if let Some(team) = found.value("team") {
        ListScope::Team(named("team", team)?.to_owned())
    } else if let Some(view) = found.value("view") {
        ListScope::View(named("view", view)?.to_owned())
    } else if let Some(words) = found.value("search") {
        ListScope::Search(named("search", words)?.to_owned())
    } else {
        ListScope::Mine
    };
    let cycle = match found.value("cycle") {
        None => None,
        Some(value) => {
            if !matches!(scope, ListScope::Team(_)) {
                return Err("--cycle is a cycle of a team: it is given with --team".to_owned());
            }
            match cycle(value, false)? {
                Setting::To(ask) => Some(ask),
                Setting::Clear => None,
            }
        }
    };
    Ok(Read::Do(Command::List(Box::new(List {
        scope,
        cycle,
        status: named_all("status", found.values("status"))?,
        priority: found
            .values("priority")
            .iter()
            .map(|value| priority("priority", value))
            .collect::<Result<_, _>>()?,
        assignee: named_all("assignee", found.values("assignee"))?,
        label: named_all("label", found.values("label"))?,
        project: named_all("project", found.values("project"))?,
        json: found.has("json"),
    }))))
}

/// The one identifier a command is given.
fn the_issue(command: &str, found: &Scanned) -> Result<String, String> {
    match found.positional.as_slice() {
        [one] => issue_key(one)
            .ok_or_else(|| format!("{one:?} is not an issue (an issue is written like ACM-412, or is Linear's id of it)")),
        [] => Err(format!("{command} is given an issue, written like ACM-412")),
        _ => Err(format!("{command} is given one issue")),
    }
}

/// The paths of `--attach`, as many as were given, at most ten.
fn attached(found: &Scanned) -> Result<Vec<String>, String> {
    let paths = found.values("attach");
    if paths.iter().any(String::is_empty) {
        return Err("--attach needs the path of a file".to_owned());
    }
    if paths.len() > SEND_MOST {
        return Err(format!("at most {SEND_MOST} files are sent at once"));
    }
    Ok(paths)
}

fn show(args: &[String]) -> Result<Read, String> {
    let found = scan("show", args, &[switch("json"), one("files")])?;
    if found.help {
        return Ok(Read::Say(super::help::SHOW.to_owned()));
    }
    let files = match found.value("files") {
        Some("") => return Err("--files needs the folder to save the files in".to_owned()),
        other => other.map(str::to_owned),
    };
    Ok(Read::Do(Command::Show {
        issue: the_issue("show", &found)?,
        files,
        json: found.has("json"),
    }))
}

fn agent(args: &[String]) -> Result<Read, String> {
    let found = scan("agent", args, &[])?;
    if found.help {
        return Ok(Read::Say(super::help::AGENT.to_owned()));
    }
    Ok(Read::Do(Command::Agent {
        issue: the_issue("agent", &found)?,
    }))
}

fn create(args: &[String]) -> Result<Read, String> {
    let spec = [
        one("team"),
        one("title"),
        one("description"),
        one("status"),
        one("priority"),
        one("assignee"),
        many("label"),
        one("project"),
        one("cycle"),
        one("estimate"),
        one("due"),
        many("attach"),
        switch("json"),
    ];
    let found = scan("create", args, &spec)?;
    if found.help {
        return Ok(Read::Say(super::help::CREATE.to_owned()));
    }
    if let Some(extra) = found.positional.first() {
        return Err(format!(
            "create takes no word but its options, and {extra:?} is not one (the title is --title)"
        ));
    }
    let team = found
        .value("team")
        .ok_or("create needs --team, the key or the name of a team (linearpen teams lists them)")?;
    let heading = found.value("title").ok_or("create needs --title")?;
    Ok(Read::Do(Command::Create(Box::new(Create {
        team: named("team", team)?.to_owned(),
        title: title(heading)?,
        description: found
            .value("description")
            .map(|value| words("description", value, DESCRIPTION_MOST))
            .transpose()?,
        fields: fields(&found)?,
        attach: attached(&found)?,
        json: found.has("json"),
    }))))
}

fn update(args: &[String]) -> Result<Read, String> {
    let spec = [
        one("status"),
        one("priority"),
        one("assignee"),
        many("label"),
        many("unlabel"),
        one("project"),
        one("cycle"),
        one("estimate"),
        one("due"),
        one("title"),
        many("attach"),
        one("description"),
        // Not changed by any way there is: named so that it is refused in words.
        one("team"),
        switch("json"),
    ];
    let found = scan("update", args, &spec)?;
    if found.help {
        return Ok(Read::Say(super::help::UPDATE.to_owned()));
    }
    if found.has("team") {
        return Err("linearpen never moves an issue to another team".to_owned());
    }
    let description = found
        .value("description")
        .map(|value| words("description", value, EDIT_MOST))
        .transpose()?;
    let mut issues: Vec<String> = Vec::new();
    for word in &found.positional {
        let key = issue_key(word).ok_or_else(|| {
            format!("{word:?} is not an issue (an issue is written like ACM-412, or is Linear's id of it)")
        })?;
        if !issues.contains(&key) {
            issues.push(key);
        }
    }
    if issues.is_empty() {
        return Err("update is given one or more issues, written like ACM-412".to_owned());
    }
    if issues.len() > BATCH_MOST {
        return Err(format!(
            "from one to {BATCH_MOST} issues are changed at once"
        ));
    }
    let heading = found.value("title").map(title).transpose()?;
    if heading.is_some() && issues.len() > 1 {
        return Err("--title is of one issue: the issues given have one title each".to_owned());
    }
    let fields = fields(&found)?;
    let unlabel = named_all("unlabel", found.values("unlabel"))?;
    let attach = attached(&found)?;
    let nothing = heading.is_none() && unlabel.is_empty() && fields == Fields::default();
    if description.is_some() {
        if issues.len() > 1 {
            return Err(
                "--description is of one issue: each issue has words of its own".to_owned(),
            );
        }
        if !nothing {
            return Err("--description is the only option of its update, with --attach: it writes the description, and the other options change the issue, so run them one after the other".to_owned());
        }
    } else if !attach.is_empty() {
        if issues.len() > 1 {
            return Err(
                "--attach is of one issue: its files go to the end of one description".to_owned(),
            );
        }
        if !nothing {
            return Err("--attach is the only option of its update: it adds to the description, and the other options change the issue, so run them one after the other".to_owned());
        }
    } else if nothing {
        return Err("update has nothing to change: give one of --status, --priority, --assignee, --label, --unlabel, --project, --cycle, --estimate, --due, --title, --description and --attach".to_owned());
    }
    let lower = |name: &str| name.to_lowercase();
    if let Some(both) = fields
        .label
        .iter()
        .find(|label| unlabel.iter().any(|gone| lower(gone) == lower(label)))
    {
        return Err(format!("the label {both:?} is both given and taken away"));
    }
    Ok(Read::Do(Command::Update(Box::new(Update {
        issues,
        title: heading,
        fields,
        unlabel,
        description,
        attach,
        json: found.has("json"),
    }))))
}

fn comment(args: &[String]) -> Result<Read, String> {
    let found = scan(
        "comment",
        args,
        &[one("body"), many("attach"), one("edit"), switch("json")],
    )?;
    if found.help {
        return Ok(Read::Say(super::help::COMMENT.to_owned()));
    }
    let issue = the_issue("comment", &found)?;
    let attach = attached(&found)?;
    if let Some(which) = found.value("edit") {
        let edit = match which {
            "last" => Edited::Last,
            id if is_ref(id) => Edited::Id(id.to_owned()),
            _ => {
                return Err("--edit is last (the last comment you wrote on the issue) or the id of a comment of yours (show --json)".to_owned());
            }
        };
        if !attach.is_empty() {
            return Err("--edit takes no --attach: a file goes in a comment of its own".to_owned());
        }
        let Some(body) = found.value("body") else {
            return Err(
                "--edit needs --body, the words the comment is made, or - for them on stdin"
                    .to_owned(),
            );
        };
        return Ok(Read::Do(Command::Comment(Comment {
            issue,
            body: Some(words("body", body, EDIT_MOST)?),
            attach,
            edit: Some(edit),
            json: found.has("json"),
        })));
    }
    let body = match found.value("body") {
        Some(body) => Some(words("body", body, BODY_MOST)?),
        None if attach.is_empty() => {
            return Err(
                "comment needs --body, the words, or - for the words on stdin, or --attach"
                    .to_owned(),
            );
        }
        None => None,
    };
    Ok(Read::Do(Command::Comment(Comment {
        issue,
        body,
        attach,
        edit: None,
        json: found.has("json"),
    })))
}

fn teams(args: &[String]) -> Result<Read, String> {
    let found = scan("teams", args, &[switch("json")])?;
    if found.help {
        return Ok(Read::Say(super::help::TEAMS.to_owned()));
    }
    let team = match found.positional.as_slice() {
        [] => None,
        [key] => Some(
            named("team", key)
                .map_err(|_| "a team is its key".to_owned())?
                .to_owned(),
        ),
        _ => return Err("teams is given one team, or none".to_owned()),
    };
    Ok(Read::Do(Command::Teams {
        team,
        json: found.has("json"),
    }))
}

/// What the words after the command `name` come to.
#[must_use]
pub fn read(name: &str, args: &[String]) -> Read {
    let done = match name {
        "list" => list(args),
        "show" => show(args),
        "create" => create(args),
        "update" => update(args),
        "comment" => comment(args),
        "teams" => teams(args),
        "agent" => agent(args),
        _ => Err(format!("{name:?} is not a command")),
    };
    match done {
        Ok(read) => read,
        Err(why) => Read::Refuse(format!("linearpen {name}: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, args: &[&str]) -> Read {
        read(
            name,
            &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
        )
    }

    fn command(name: &str, args: &[&str]) -> Command {
        match run(name, args) {
            Read::Do(command) => command,
            other => panic!("{name} {args:?}: {other:?}"),
        }
    }

    fn refused(name: &str, args: &[&str]) -> String {
        match run(name, args) {
            Read::Refuse(why) => why,
            other => panic!("{name} {args:?}: {other:?}"),
        }
    }

    fn strings(of: &[&str]) -> Vec<String> {
        of.iter().map(|word| (*word).to_owned()).collect()
    }

    // Linear's own id of an issue is what an agent started on one is told to give.
    #[test]
    fn an_issue_is_its_identifier_or_linears_id_of_it_in_lower_case() {
        let uuid = "0C9E0F1E-6d1b-4d7a-9a56-3b1c0a5e7f10";
        let lower = "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f10";
        let Command::Update(update) = command("update", &[uuid, "ACM-1", "--status", "Done"])
        else {
            panic!()
        };
        assert_eq!(update.issues, [lower, "ACM-1"]);
        assert!(matches!(
            command("show", &[uuid]),
            Command::Show { issue, .. } if issue == lower
        ));
        assert!(matches!(
            command("comment", &[uuid, "--body", "-"]),
            Command::Comment(comment) if comment.issue == lower
        ));
        // What is neither is not an issue.
        for word in [
            "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f1",
            "0c9e0f1e-6d1b-4d7a-9a56-3b1c0a5e7f1g",
            "0c9e0f1e6d1b4d7a9a563b1c0a5e7f10",
        ] {
            assert!(refused("show", &[word]).contains("not an issue"), "{word}");
        }
    }

    #[test]
    fn list_is_my_issues_and_takes_what_narrows_it() {
        let Command::List(bare) = command("list", &[]) else {
            panic!()
        };
        assert_eq!(bare.scope, ListScope::Mine);
        assert!(bare.status.is_empty() && !bare.json);
        let Command::List(list) = command(
            "list",
            &[
                "--team",
                "ACM",
                "--cycle",
                "current",
                "--status",
                "In Progress",
                "--status",
                "Todo",
                "--priority",
                "URGENT",
                "--assignee",
                "me",
                "--assignee",
                "none",
                "--label",
                "bug",
                "--project",
                "none",
                "--json",
            ],
        ) else {
            panic!()
        };
        assert_eq!(list.scope, ListScope::Team("ACM".to_owned()));
        assert_eq!(list.cycle, Some(CycleAsk::Current));
        assert_eq!(list.status, strings(&["In Progress", "Todo"]));
        assert_eq!(list.priority, [Priority::Urgent]);
        assert_eq!(list.assignee, strings(&["me", "none"]));
        assert_eq!(list.label, strings(&["bug"]));
        assert_eq!(list.project, strings(&["none"]));
        assert!(list.json);
    }

    #[test]
    fn the_lists_are_chosen_one_at_a_time() {
        for (flag, value, scope) in [
            ("--view", "Bugs", ListScope::View("Bugs".to_owned())),
            (
                "--search",
                "rate limit",
                ListScope::Search("rate limit".to_owned()),
            ),
        ] {
            let Command::List(list) = command("list", &[flag, value]) else {
                panic!()
            };
            assert_eq!(list.scope, scope);
        }
        let Command::List(all) = command("list", &["--all"]) else {
            panic!()
        };
        assert_eq!(all.scope, ListScope::All);
        assert!(refused("list", &["--all", "--team", "ACM"]).contains("only one"));
        assert!(refused("list", &["--view", "a", "--search", "b"]).contains("only one"));
        assert!(refused("list", &["--cycle", "3"]).contains("with --team"));
        assert!(refused("list", &["--team", "ACM", "--cycle", "none"]).contains("--cycle is"));
        assert!(refused("list", &["--team", "ACM", "--cycle", "soon"]).contains("--cycle is"));
        assert!(refused("list", &["--priority", "soon"]).contains("--priority is"));
        assert!(refused("list", &["--team", "ACM", "--team", "WEB"]).contains("more than once"));
        assert!(refused("list", &["ACM-1"]).contains("no word"));
        assert!(refused("list", &["--status", " "]).contains("needs a value"));
    }

    #[test]
    fn show_takes_one_issue_in_any_case() {
        assert_eq!(
            command("show", &["acm-412", "--json"]),
            Command::Show {
                issue: "ACM-412".to_owned(),
                files: None,
                json: true
            }
        );
        assert_eq!(
            command("show", &["acm-412", "--files", "pics/a b"]),
            Command::Show {
                issue: "ACM-412".to_owned(),
                files: Some("pics/a b".to_owned()),
                json: false
            }
        );
        for bad in [
            &[][..],
            &["ACM"],
            &["412"],
            &["ACM-"],
            &["A B-1"],
            &["ACM-1", "ACM-2"],
            &["1A-2"],
        ] {
            assert!(matches!(run("show", bad), Read::Refuse(_)), "{bad:?}");
        }
    }

    #[test]
    fn create_needs_a_team_and_a_title_and_checks_the_values() {
        let Command::Create(made) = command(
            "create",
            &[
                "--team",
                "ACM",
                "--title",
                " Fix\nthe login ",
                "--description",
                "-",
                "--status",
                "Todo",
                "--priority",
                "high",
                "--assignee",
                "me",
                "--label",
                "bug",
                "--label",
                "api",
                "--project",
                "Billing",
                "--cycle",
                "12",
                "--estimate",
                "M",
                "--due",
                "2026-11-20",
            ],
        ) else {
            panic!()
        };
        assert_eq!(made.team, "ACM");
        assert_eq!(made.title, "Fix the login");
        assert_eq!(made.description, Some(Words::Stdin));
        assert_eq!(made.fields.status.as_deref(), Some("Todo"));
        assert_eq!(made.fields.priority, Some(Priority::High));
        assert_eq!(made.fields.assignee, Some(Setting::To("me".to_owned())));
        assert_eq!(made.fields.label, strings(&["bug", "api"]));
        assert_eq!(made.fields.cycle, Some(Setting::To(CycleAsk::Number(12))));
        assert_eq!(made.fields.estimate, Some(Setting::To("M".to_owned())));
        assert_eq!(made.fields.due, Some(Setting::To("2026-11-20".to_owned())));
        assert!(refused("create", &["--title", "x"]).contains("--team"));
        assert!(refused("create", &["--team", "ACM"]).contains("--title"));
        assert!(refused("create", &["--team", "ACM", "--title", " "]).contains("--title"));
        assert!(
            refused(
                "create",
                &["--team", "ACM", "--title", "x", "--description", ""]
            )
            .contains("--description")
        );
        assert!(
            refused(
                "create",
                &["--team", "ACM", "--title", "x", "--due", "soon"]
            )
            .contains("--due")
        );
        assert!(
            refused(
                "create",
                &["--team", "ACM", "--title", "x", "--unlabel", "a"]
            )
            .contains("not an option")
        );
        assert!(
            refused(
                "create",
                &[
                    "--team", "ACM", "--title", "x", "--status", "a", "--status", "b"
                ]
            )
            .contains("more than once")
        );
        let long = "x".repeat(256);
        assert!(refused("create", &["--team", "ACM", "--title", &long]).contains("--title"));
    }

    #[test]
    fn what_can_be_taken_away_is_taken_away_by_none() {
        let Command::Update(update) = command(
            "update",
            &[
                "ACM-1",
                "--assignee",
                "None",
                "--project",
                "none",
                "--cycle",
                "none",
                "--estimate",
                "none",
                "--due",
                "none",
                "--priority",
                "none",
            ],
        ) else {
            panic!()
        };
        assert_eq!(update.fields.assignee, Some(Setting::Clear));
        assert_eq!(update.fields.project, Some(Setting::Clear));
        assert_eq!(update.fields.cycle, Some(Setting::Clear));
        assert_eq!(update.fields.estimate, Some(Setting::Clear));
        assert_eq!(update.fields.due, Some(Setting::Clear));
        assert_eq!(update.fields.priority, Some(Priority::None));
    }

    #[test]
    fn update_takes_issues_and_fields_and_refuses_what_it_does_not_change() {
        let Command::Update(update) = command(
            "update",
            &[
                "acm-1",
                "ACM-2",
                "ACM-1",
                "--status",
                "Done",
                "--label",
                "a",
                "--label",
                "b",
                "--unlabel",
                "c",
            ],
        ) else {
            panic!()
        };
        assert_eq!(update.issues, strings(&["ACM-1", "ACM-2"]));
        assert_eq!(update.fields.label, strings(&["a", "b"]));
        assert_eq!(update.unlabel, strings(&["c"]));
        assert!(refused("update", &["--status", "Done"]).contains("one or more issues"));
        assert!(refused("update", &["ACM-1"]).contains("nothing to change"));
        assert!(refused("update", &["ACM-1", "--json"]).contains("nothing to change"));
        assert!(
            refused("update", &["ACM-1", "--status", "a", "--status", "b"])
                .contains("more than once")
        );
        assert!(
            refused("update", &["ACM-1", "--label", "Bug", "--unlabel", "bug"]).contains("both")
        );
        assert!(
            refused("update", &["ACM-1", "ACM-2", "--title", "x"]).contains("--title is of one")
        );
        assert!(refused("update", &["ACM-1", "--team", "WEB"]).contains("another team"));
        assert!(
            refused("update", &["ACM-1", "--description=x", "--status", "a"])
                .contains("--description is the only option")
        );
        assert!(
            refused("update", &["ACM-1", "ACM-2", "--description", "x"])
                .contains("--description is of one issue")
        );
        assert!(refused("update", &["ACM-1", "--description", " "]).contains("--description"));
        assert!(refused("update", &["nope", "--status", "a"]).contains("not an issue"));
    }

    #[test]
    fn at_most_fifty_distinct_issues_are_changed() {
        let many: Vec<String> = (1..=51).map(|n| format!("ACM-{n}")).collect();
        let mut args: Vec<&str> = many.iter().map(String::as_str).collect();
        args.extend(["--status", "Done"]);
        assert!(refused("update", &args).contains("at once"));
        // A repeated id is one.
        let mut repeated: Vec<&str> = many[..50].iter().map(String::as_str).collect();
        repeated.push("ACM-1");
        repeated.extend(["--status", "Done"]);
        assert!(matches!(run("update", &repeated), Read::Do(_)));
    }

    #[test]
    fn update_writes_a_description_whole_for_one_issue_with_its_files_and_nothing_else() {
        let Command::Update(update) = command("update", &["acm-1", "--description", "-"]) else {
            panic!()
        };
        assert_eq!(update.description, Some(Words::Stdin));
        assert_eq!(update.issues, ["ACM-1"]);
        let Command::Update(update) = command(
            "update",
            &["ACM-1", "--description=New words", "--attach", "shot.png"],
        ) else {
            panic!()
        };
        assert_eq!(
            update.description,
            Some(Words::Given("New words".to_owned()))
        );
        assert_eq!(update.attach, ["shot.png"]);
    }

    #[test]
    fn comment_edit_names_the_last_comment_or_one_by_its_id_and_takes_words_and_no_file() {
        let Command::Comment(given) =
            command("comment", &["ACM-412", "--edit", "last", "--body", "-"])
        else {
            panic!()
        };
        assert_eq!(given.edit, Some(Edited::Last));
        assert_eq!(given.body, Some(Words::Stdin));
        let Command::Comment(given) = command(
            "comment",
            &[
                "ACM-412",
                "--edit",
                "5c1c2a58-7d5e-4c0b-9b43-1c8f0f6f2d11",
                "--body=Fixed",
            ],
        ) else {
            panic!()
        };
        assert_eq!(
            given.edit,
            Some(Edited::Id(
                "5c1c2a58-7d5e-4c0b-9b43-1c8f0f6f2d11".to_owned()
            ))
        );
        assert!(refused("comment", &["ACM-412", "--edit", "last"]).contains("--body"));
        assert!(
            refused(
                "comment",
                &[
                    "ACM-412", "--edit", "last", "--body", "x", "--attach", "a.png"
                ]
            )
            .contains("--attach")
        );
        assert!(
            refused("comment", &["ACM-412", "--edit", "a b", "--body", "x"]).contains("--edit")
        );
        // A comment edited is longer than one posted may be: it was written in Linear, perhaps.
        let long = "x".repeat(10_001);
        assert!(matches!(
            run("comment", &["ACM-412", "--edit", "last", "--body", &long]),
            Read::Do(_)
        ));
    }

    #[test]
    fn comment_takes_an_issue_and_words_or_stdin() {
        assert_eq!(
            command("comment", &["acm-412", "--body", "-"]),
            Command::Comment(Comment {
                issue: "ACM-412".to_owned(),
                body: Some(Words::Stdin),
                attach: Vec::new(),
                edit: None,
                json: false
            })
        );
        let Command::Comment(given) =
            command("comment", &["ACM-412", "--body=Looks right", "--json"])
        else {
            panic!()
        };
        assert_eq!(given.body, Some(Words::Given("Looks right".to_owned())));
        assert!(given.json);
        assert!(refused("comment", &["ACM-412"]).contains("--body"));
        assert!(refused("comment", &["--body", "x"]).contains("an issue"));
        assert!(refused("comment", &["ACM-412", "--body", "  "]).contains("--body"));
        let long = "x".repeat(10_001);
        assert!(refused("comment", &["ACM-412", "--body", &long]).contains("--body"));
    }

    #[test]
    fn teams_takes_a_key_or_none() {
        assert_eq!(
            command("teams", &[]),
            Command::Teams {
                team: None,
                json: false
            }
        );
        assert_eq!(
            command("teams", &["ACM", "--json"]),
            Command::Teams {
                team: Some("ACM".to_owned()),
                json: true
            }
        );
        assert!(refused("teams", &["ACM", "WEB"]).contains("one team"));
    }

    #[test]
    fn every_command_says_its_options_for_help_wherever_it_is_asked() {
        for name in NAMES {
            match run(name, &["--help"]) {
                Read::Say(words) => assert!(words.contains(&format!("linearpen {name}")), "{name}"),
                other => panic!("{name}: {other:?}"),
            }
            assert!(matches!(run(name, &["-h"]), Read::Say(_)), "{name}");
        }
    }
}
