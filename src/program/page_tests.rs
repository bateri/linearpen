//! The page (src/view/) and the program's own handlers in one process, as a terminal runs them: the
//! kit's interpreter under pencere's host with no screen (`pencere-drive`), a clock the test moves,
//! the calls and the topic of `api/` served as `serve` serves them, over a transport of memory that
//! answers with the fixtures of test/stand-in/. Nothing here starts a process, reaches Linear or reads
//! the person's key. What the page shows is read as words; what the person does is done to it as a
//! terminal does it.
//!
//! Each opens the interpreter (compiled once a run): `make test-page` runs them, in `make check`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pencere_drive::headless::{Error, Headless, InProcess};

use crate::linear::transport::Reply;
use crate::linear::{Error as LinearError, Key, Transport};
use crate::options;
use crate::session::Session;
use crate::state::Paths;

/// A page as large as the docs' pictures, and an 80×24 pane.
pub(crate) const WIDE: (f32, f32) = (1100.0, 720.0);
pub(crate) const PANE: (f32, f32) = (600.0, 330.0);

/// How long a test waits for what the program says on its own (a list, a call).
const WITHIN: Duration = Duration::from_secs(20);

pub(crate) type Page = Headless<InProcess>;

const MY_ISSUES: &str = include_str!("../../test/stand-in/my-issues.json");
const MY_ISSUES_EMPTY: &str = include_str!("../../test/stand-in/my-issues-empty.json");
const ALL_ISSUES: &str = include_str!("../../test/stand-in/all-issues.json");
const VERIFY: &str = include_str!("../../test/stand-in/verify.json");
const ISSUE: &str = include_str!("../../test/stand-in/issue.json");
const REFUSED: &str = include_str!("../../test/stand-in/refused.json");
const TEAMS: &str = include_str!("../../test/stand-in/teams.json");
const NEW_ISSUE_OPTIONS: &str = include_str!("../../test/stand-in/new-issue-options.json");
const CREATED_ISSUE: &str = include_str!("../../test/stand-in/created-issue.json");
const CREATE_REFUSED: &str = include_str!("../../test/stand-in/create-refused.json");
const CHANGE_REFUSED: &str = include_str!("../../test/stand-in/change-refused.json");

/// How Linear answers in a test.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// With the lists.
    Fine,
    /// With the lists, but the person has nothing assigned.
    NothingAssigned,
    /// By refusing the key.
    Refused,
    /// By not being there.
    Down,
    /// By not answering yet: what is asked waits until the mode is another.
    Held,
}

/// How Linear answers a new issue.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Create {
    /// By making it: it is in the lists from then on.
    Fine,
    /// By not making it (an error of its own).
    Refused,
    /// By saying the key may not do it (HTTP 403).
    Forbidden,
}

/// Linear, in memory: answers by the query's name with a fixture, and keeps what it was asked.
pub(crate) struct Canned {
    mode: Mutex<Mode>,
    create: Mutex<Create>,
    asked: Mutex<Vec<String>>,
    key: Mutex<Option<String>>,
    /// The input of each `IssueCreate` that was sent, and the input of the one that made an issue.
    sent: Mutex<Vec<serde_json::Value>>,
    made: Mutex<Option<serde_json::Value>>,
    /// Whether Linear has one team, and no label or project to choose.
    bare: Mutex<bool>,
    /// Whether Linear has one team.
    one_team: Mutex<bool>,
    /// How a change is answered.
    change: Mutex<Change>,
    /// The inputs of the `IssueBatchUpdate` requests that were sent (`ids` and `input`), in order.
    changes: Mutex<Vec<serde_json::Value>>,
    /// What each issue was changed to, in order: the lists and `Issue` answer with it from then on.
    edits: Mutex<std::collections::HashMap<String, Vec<serde_json::Value>>>,
    /// The team of an issue where it is not Acme's: the id and the key.
    teams_of: Mutex<std::collections::HashMap<String, (String, String)>>,
}

/// How Linear answers a change.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// By making it.
    Fine,
    /// By not making it (an error of its own).
    Refused,
}

impl Canned {
    pub(crate) fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode: Mutex::new(mode),
            create: Mutex::new(Create::Fine),
            asked: Mutex::new(Vec::new()),
            key: Mutex::new(None),
            sent: Mutex::new(Vec::new()),
            made: Mutex::new(None),
            bare: Mutex::new(false),
            one_team: Mutex::new(false),
            change: Mutex::new(Change::Fine),
            changes: Mutex::new(Vec::new()),
            edits: Mutex::new(std::collections::HashMap::new()),
            teams_of: Mutex::new(std::collections::HashMap::new()),
        })
    }

    /// How a change is answered from now on.
    pub(crate) fn set_change(&self, change: Change) {
        *self.change.lock().unwrap() = change;
    }

    /// The `IssueBatchUpdate` requests that were sent, in order: `{ "ids": [..], "input": {..} }`.
    pub(crate) fn changed(&self) -> Vec<serde_json::Value> {
        self.changes.lock().unwrap().clone()
    }

    /// From now on the issue `id` is of the team `team` (key `key`), not of Acme.
    pub(crate) fn set_team_of(&self, id: &str, team: &str, key: &str) {
        self.teams_of
            .lock()
            .unwrap()
            .insert(id.to_owned(), (team.to_owned(), key.to_owned()));
    }

    /// The node of the issue `id` as the fixtures have it, with the changes made to it since; `None`
    /// where there is no such issue.
    fn node_of(&self, id: &str) -> Option<serde_json::Value> {
        let all: serde_json::Value = serde_json::from_str(ALL_ISSUES).unwrap();
        let mut node = all["data"]["issues"]["nodes"]
            .as_array()?
            .iter()
            .find(|node| node["id"] == id)
            .cloned()
            .or_else(|| self.made_node().filter(|node| node["id"] == id))?;
        if let Some((team, key)) = self.teams_of.lock().unwrap().get(id) {
            node["team"] = serde_json::json!({ "id": team, "key": key });
        }
        for input in self.edits.lock().unwrap().get(id).into_iter().flatten() {
            edit_node(&mut node, input);
        }
        Some(node)
    }

    /// A node of a list as it stands now: `None` where it has left the list, being done or
    /// canceled, or (in the list of the person's own) given to someone else.
    fn shown(&self, node: &serde_json::Value, mine: bool) -> Option<serde_json::Value> {
        let id = node["id"].as_str()?;
        let edited = self.edits.lock().unwrap().contains_key(id)
            || self.teams_of.lock().unwrap().contains_key(id);
        if !edited {
            return Some(node.clone());
        }
        let now = self.node_of(id)?;
        let kind = now["state"]["type"].as_str().unwrap_or_default();
        if kind == "completed" || kind == "canceled" {
            return None;
        }
        if mine && now["assignee"]["isMe"] != true {
            return None;
        }
        Some(now)
    }

    /// From now on Linear has one team and no label or project to choose.
    pub(crate) fn set_bare(&self) {
        *self.bare.lock().unwrap() = true;
    }

    /// From now on Linear has one team.
    pub(crate) fn set_one_team(&self) {
        *self.one_team.lock().unwrap() = true;
    }

    fn teams_text(&self) -> String {
        if !*self.bare.lock().unwrap() && !*self.one_team.lock().unwrap() {
            return TEAMS.to_owned();
        }
        let mut value: serde_json::Value = serde_json::from_str(TEAMS).unwrap();
        value["data"]["teams"]["nodes"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        value.to_string()
    }

    fn options_text(&self) -> String {
        if !*self.bare.lock().unwrap() {
            return NEW_ISSUE_OPTIONS.to_owned();
        }
        let mut value: serde_json::Value = serde_json::from_str(NEW_ISSUE_OPTIONS).unwrap();
        value["data"]["team"]["labels"]["nodes"] = serde_json::json!([]);
        value["data"]["team"]["projects"]["nodes"] = serde_json::json!([]);
        value["data"]["issueLabels"]["nodes"] = serde_json::json!([]);
        value.to_string()
    }

    /// How a new issue is answered from now on.
    pub(crate) fn set_create(&self, create: Create) {
        *self.create.lock().unwrap() = create;
    }

    /// The inputs of the `IssueCreate` requests that were sent, in order.
    pub(crate) fn created(&self) -> Vec<serde_json::Value> {
        self.sent.lock().unwrap().clone()
    }

    /// The created issue as a node of a list, where there is one.
    fn made_node(&self) -> Option<serde_json::Value> {
        let input = self.made.lock().unwrap().clone()?;
        let mut node: serde_json::Value = serde_json::from_str(CREATED_ISSUE).ok()?;
        node["id"] = input["id"].clone();
        node["title"] = input["title"].clone();
        node["priority"] = input["priority"].as_f64().unwrap_or(0.0).into();
        node["state"] = option_of("states", input["stateId"].as_str().unwrap_or("s-todo"))
            .unwrap_or_else(|| option_of("states", "s-todo").unwrap());
        node["labels"]["nodes"] = input["labelIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| option_of("labels", id.as_str()?))
            .collect::<Vec<_>>()
            .into();
        node["assignee"] = match input["assigneeId"].as_str() {
            Some(id) => person_of(id),
            None => serde_json::Value::Null,
        };
        Some(node)
    }

    /// The made issue as `Issue` says it: with its description and no comments.
    fn made_detail(&self) -> Option<serde_json::Value> {
        let node = self.made_node()?;
        let input = self.made.lock().unwrap().clone()?;
        let mut value: serde_json::Value = serde_json::from_str(ISSUE).ok()?;
        for key in [
            "id",
            "identifier",
            "title",
            "priority",
            "state",
            "labels",
            "assignee",
            "url",
            "branchName",
        ] {
            value["data"]["issue"][key] = node[key].clone();
        }
        value["data"]["issue"]["description"] = input["description"].clone();
        value["data"]["issue"]["comments"]["nodes"] = serde_json::json!([]);
        value["data"]["issue"]["cycle"] = serde_json::Value::Null;
        value["data"]["issue"]["project"] = serde_json::Value::Null;
        Some(value)
    }

    /// The issue `id` as `Issue` says it: the fixture's description and comments, the rest as the
    /// issue stands in the lists now.
    fn detail_of(&self, id: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(ISSUE).unwrap();
        if let Some(node) = self.node_of(id) {
            for key in [
                "id",
                "identifier",
                "title",
                "priority",
                "state",
                "labels",
                "assignee",
                "team",
                "project",
                "cycle",
                "url",
                "branchName",
            ] {
                value["data"]["issue"][key] = node[key].clone();
            }
        }
        value.to_string()
    }

    /// A list of the fixtures with the created issue first, if one was made, and each issue as the
    /// changes made leave it.
    fn listing(&self, fixture: &str, mine: bool) -> String {
        let mut value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let nodes = if mine {
            &mut value["data"]["viewer"]["assignedIssues"]["nodes"]
        } else {
            &mut value["data"]["issues"]["nodes"]
        };
        let nodes = nodes.as_array_mut().unwrap();
        if let Some(node) = self.made_node() {
            nodes.insert(0, node);
        }
        let now: Vec<serde_json::Value> = nodes
            .iter()
            .filter_map(|node| self.shown(node, mine))
            .collect();
        *nodes = now;
        value.to_string()
    }

    pub(crate) fn set(&self, mode: Mode) {
        *self.mode.lock().unwrap() = mode;
    }

    /// How many times the query `name` was asked.
    pub(crate) fn times(&self, name: &str) -> usize {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|asked| *asked == name)
            .count()
    }

    /// The key the last question was asked with.
    pub(crate) fn last_key(&self) -> Option<String> {
        self.key.lock().unwrap().clone()
    }
}

/// The state, label, project or member `id` of the options the fixtures have for Acme, as a node
/// of an issue has it. (`kind` is `states`, `labels`, `projects` or `members`.)
fn option_of(kind: &str, id: &str) -> Option<serde_json::Value> {
    let options: serde_json::Value = serde_json::from_str(NEW_ISSUE_OPTIONS).unwrap();
    let found = options["data"]["team"][kind]["nodes"]
        .as_array()?
        .iter()
        .find(|node| node["id"] == id)?
        .clone();
    Some(match kind {
        "labels" => serde_json::json!({
            "id": found["id"], "name": found["name"], "color": found["color"] }),
        "projects" => serde_json::json!({ "id": found["id"], "name": found["name"] }),
        "members" => serde_json::json!({
            "id": found["id"], "displayName": found["displayName"], "isMe": found["isMe"] }),
        _ => found,
    })
}

/// A member as an issue names its assignee; the label that is of the workspace is found too.
fn person_of(id: &str) -> serde_json::Value {
    option_of("members", id).unwrap_or(serde_json::Value::Null)
}

/// What an `IssueBatchUpdate` input does to the node of an issue.
fn edit_node(node: &mut serde_json::Value, input: &serde_json::Value) {
    if let Some(title) = input.get("title") {
        node["title"] = title.clone();
    }
    if let Some(priority) = input["priority"].as_f64() {
        node["priority"] = priority.into();
    }
    if let Some(state) = input["stateId"]
        .as_str()
        .and_then(|id| option_of("states", id))
    {
        node["state"] = state;
    }
    if let Some(assignee) = input.get("assigneeId") {
        node["assignee"] = assignee.as_str().map_or(serde_json::Value::Null, person_of);
    }
    if let Some(project) = input.get("projectId") {
        node["project"] = project
            .as_str()
            .and_then(|id| option_of("projects", id))
            .unwrap_or(serde_json::Value::Null);
    }
    let mut labels: Vec<serde_json::Value> = node["labels"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for id in input["removedLabelIds"].as_array().into_iter().flatten() {
        labels.retain(|label| label["id"] != *id);
    }
    for id in input["addedLabelIds"].as_array().into_iter().flatten() {
        let already = labels.iter().any(|label| label["id"] == *id);
        if let (false, Some(label)) = (already, id.as_str().and_then(|id| option_of("labels", id)))
        {
            labels.push(label);
        }
    }
    node["labels"]["nodes"] = labels.into();
}

impl Transport for Canned {
    fn post(&self, key: &Key, body: &str) -> Result<Reply, LinearError> {
        let name = [
            "Verify",
            "MyIssues",
            "AllIssues",
            "IssueCreate",
            "IssueBatchUpdate",
            "NewIssueTeams",
            "NewIssueOptions",
            "Issue",
        ]
        .into_iter()
        .find(|name| body.contains(&format!(r#""operationName":"{name}""#)))
        .unwrap_or("Other");
        self.asked.lock().unwrap().push(name.to_owned());
        *self.key.lock().unwrap() = Some(key.expose().to_owned());
        let reply = |status, body: &str| {
            Ok(Reply {
                status,
                body: body.to_owned(),
            })
        };
        while *self.mode.lock().unwrap() == Mode::Held {
            std::thread::sleep(Duration::from_millis(20));
        }
        match *self.mode.lock().unwrap() {
            Mode::Down | Mode::Held => Err(LinearError::Unreachable(
                "the host name was not found".to_owned(),
            )),
            Mode::Refused => reply(401, REFUSED),
            Mode::NothingAssigned if name == "MyIssues" => {
                reply(200, &self.listing(MY_ISSUES_EMPTY, true))
            }
            _ => match name {
                "Verify" => reply(200, VERIFY),
                "MyIssues" => reply(200, &self.listing(MY_ISSUES, true)),
                "AllIssues" => reply(200, &self.listing(ALL_ISSUES, false)),
                "Issue" => {
                    let asked: serde_json::Value = serde_json::from_str(body).unwrap();
                    let id = asked["variables"]["id"].as_str().unwrap_or_default();
                    match self.made_detail() {
                        Some(value) if value["data"]["issue"]["id"] == id => {
                            reply(200, &value.to_string())
                        }
                        _ => reply(200, &self.detail_of(id)),
                    }
                }
                "NewIssueTeams" => reply(200, &self.teams_text()),
                "NewIssueOptions" => reply(200, &self.options_text()),
                "IssueBatchUpdate" => {
                    let asked: serde_json::Value = serde_json::from_str(body).unwrap();
                    self.changes.lock().unwrap().push(serde_json::json!({
                        "ids": asked["variables"]["ids"], "input": asked["variables"]["input"] }));
                    if *self.change.lock().unwrap() == Change::Refused {
                        return reply(400, CHANGE_REFUSED);
                    }
                    let ids = asked["variables"]["ids"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let mut nodes = Vec::new();
                    for id in ids.iter().filter_map(|id| id.as_str()) {
                        self.edits
                            .lock()
                            .unwrap()
                            .entry(id.to_owned())
                            .or_default()
                            .push(asked["variables"]["input"].clone());
                        nodes.extend(self.node_of(id));
                    }
                    reply(
                        200,
                        &serde_json::json!({ "data": { "issueBatchUpdate": {
                            "success": true, "issues": nodes } } })
                        .to_string(),
                    )
                }
                "IssueCreate" => {
                    let asked: serde_json::Value = serde_json::from_str(body).unwrap();
                    let input = asked["variables"]["input"].clone();
                    self.sent.lock().unwrap().push(input.clone());
                    match *self.create.lock().unwrap() {
                        Create::Refused => reply(400, CREATE_REFUSED),
                        Create::Forbidden => reply(403, r#"{"errors":[{"message":"Forbidden"}]}"#),
                        Create::Fine => {
                            let id = input["id"].as_str().unwrap_or_default().to_owned();
                            *self.made.lock().unwrap() = Some(input.clone());
                            reply(
                                200,
                                &serde_json::json!({ "data": { "issueCreate": { "success": true,
                                    "issue": { "id": id, "identifier": "ACM-900" } } } })
                                .to_string(),
                            )
                        }
                    }
                }
                _ => Err(LinearError::Failed(
                    "a query the test does not know".to_owned(),
                )),
            },
        }
    }
}

/// What the page's module is held to. The tests say what the page shows and does, not how fast a
/// machine is: a runner under load takes ten times as long as one at rest, so the kit's 250 ms a
/// call and 2 s to start, which are the wall clock's, are far wider here by default. With
/// `LINEARPEN_CALL_MS` set (`make check-timing`) a call is held to that many milliseconds.
pub(crate) fn limits(call_ms: Option<u64>) -> pencere_sandbox::ModuleLimits {
    let kit = pencere_sandbox::ModuleLimits::default();
    match call_ms {
        Some(ms) => pencere_sandbox::ModuleLimits {
            call: Duration::from_millis(ms),
            ..kit
        },
        None => pencere_sandbox::ModuleLimits {
            call: GENEROUS_CALL,
            start: GENEROUS_CALL * 6,
            ..kit
        },
    }
}

/// A call of the page, in a test that is not about time: forty times the kit's.
const GENEROUS_CALL: Duration = Duration::from_secs(10);

/// A folder of this run for the key a test connects, named by the process and the test.
pub(crate) fn config_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("linearpen-page-{}-{name}", std::process::id()))
}

/// A session over `transport`: with a key (the one of the environment, as a run is given one), or
/// without, and keeping a key that is connected in `config`.
pub(crate) fn session_over(
    transport: &Arc<Canned>,
    key: Option<&str>,
    config: Option<PathBuf>,
) -> Arc<Session> {
    Arc::new(Session::new(
        Arc::clone(transport) as Arc<dyn Transport>,
        Paths {
            config,
            state: None,
        },
        key,
        // The lists are asked for again only when the page says so.
        Duration::from_secs(3600),
    ))
}

/// The page on the handlers of `session`, at `size`, until it says `ready`.
pub(crate) fn open(
    session: &Arc<Session>,
    size: (f32, f32),
    fonts: Option<&pencere_drive::shot::Fonts>,
    ready: &str,
) -> Result<Page, Error> {
    let program = InProcess::new(options(session, &Arc::new(Mutex::new(None))));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64());
    let mut opening = Headless::app(env!("CARGO_MANIFEST_DIR"))
        .size(size.0, size.1)
        .wall_clock(now, 0)
        .limits(limits(
            std::env::var("LINEARPEN_CALL_MS")
                .ok()
                .and_then(|ms| ms.parse().ok()),
        ));
    if let Some(fonts) = fonts {
        opening = opening.measure(fonts.clone()).scale(2.0);
    }
    let mut page = opening.peer(program).open()?;
    page.wait_for(WITHIN, |page| page.says(ready))?;
    page.advance(0.3)?;
    Ok(page)
}

/// The page over the lists of the fixtures, at `size`.
pub(crate) fn open_lists(size: (f32, f32)) -> (Arc<Canned>, Arc<Session>, Page) {
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let page = open(&session, size, None, "ACM-412").expect("the page opens");
    (linear, session, page)
}

/// The words of the page, one string.
pub(crate) fn said(page: &Page) -> String {
    page.words().join("|")
}

/// What the line the cursor is on says to a screen reader, or an empty string.
pub(crate) fn cursor(page: &Page) -> String {
    page.outline()
        .accessible
        .iter()
        .find(|node| node.state.selected)
        .map(|node| node.words.clone())
        .unwrap_or_default()
}

/// Waits until the cursor is on a line that says `words`.
fn cursor_on(page: &mut Page, words: &str) {
    page.wait_for(WITHIN, |page| cursor(page).contains(words))
        .unwrap_or_else(|why| {
            panic!(
                "the cursor is on {words:?}: {why:?}; it is on {:?}",
                cursor(page)
            )
        });
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_issues_of_the_person_come_in_groups_by_state() {
    let (_linear, _session, page) = open_lists(WIDE);
    for words in [
        "In Progress",
        "In Review",
        "Todo",
        "Backlog",
        "ACM-412",
        "Add rate limiting to the public API",
        // The count of each list, in the control that chooses between them.
        "My issues 14",
        "All issues 19",
    ] {
        assert!(page.says(words), "{words}: {}", said(&page));
    }
    // The backlog begins folded: its name and its count, no issue.
    assert!(!page.says("ACM-352"), "{}", said(&page));
    // The cursor is on the first issue.
    assert!(cursor(&page).contains("ACM-412"), "{}", cursor(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_press_on_a_groups_head_folds_and_opens_it() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.click("row:g:backlog:Backlog")
        .expect("the head is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-352"))
        .expect("the backlog is open");
    cursor_on(&mut page, "Backlog");
    page.click("row:g:backlog:Backlog")
        .expect("the head is pressed");
    page.wait_for(WITHIN, |page| !page.says("ACM-352"))
        .expect("the backlog is folded again");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_arrow_keys_and_the_letters_move_the_cursor() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    page.press("j").expect("j");
    cursor_on(&mut page, "ACM-405");
    page.press("k").expect("k");
    cursor_on(&mut page, "ACM-398");
    page.press("home").expect("home");
    cursor_on(&mut page, "In Progress");
    page.press("end").expect("end");
    cursor_on(&mut page, "Backlog");
    page.press("up").expect("up");
    cursor_on(&mut page, "ACM-396");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn left_folds_the_cursors_group_and_right_opens_it_and_enter_turns_a_head_over() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("left").expect("left");
    page.wait_for(WITHIN, |page| !page.says("ACM-412"))
        .expect("the group is folded");
    // The cursor is where its group is now: on the head.
    cursor_on(&mut page, "In Progress");
    page.press("right").expect("right");
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
        .expect("the group is open");
    page.press("home").expect("home");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| !page.says("ACM-412"))
        .expect("enter folds the head it is on");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn one_and_two_choose_between_my_issues_and_all_of_them() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    assert!(!page.says("ACM-417"), "{}", said(&page));
    page.press("2").expect("2");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .expect("the issues of the others are there");
    // Whose it is, in the list of all of them.
    assert!(page.says("Deniz"), "{}", said(&page));
    page.press("1").expect("1");
    page.wait_for(WITHIN, |page| !page.says("ACM-417"))
        .expect("back to mine");
    // The control does the same with the pointer.
    page.click("scope/item/all")
        .expect("the control is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .expect("all issues by the pointer");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_filter_narrows_the_list_and_escape_takes_it_off() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("/").expect("slash");
    page.type_text("bug").expect("the filter is typed");
    page.wait_for(WITHIN, |page| page.says("4 of 14") && !page.says("ACM-412"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(page.says("ACM-398"), "{}", said(&page));
    // The cursor's issue went with the filter: it is on the first that is left.
    cursor_on(&mut page, "ACM-398");
    // The first escape clears the field, which keeps the keyboard.
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| page.says("ACM-412") && !page.says("4 of 14"))
        .expect("the filter is gone");
    // Down in the field is the list's, and Enter there is Enter on the list: the issue the cursor
    // is on opens, and the keyboard is the page's again.
    page.type_text("a").expect("a letter");
    page.press("down").expect("down");
    page.press("enter").expect("enter opens the issue");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("the issue opens: {why:?}: {}", said(&page)));
    page.press("escape").expect("escape closes the issue");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_none())
        .expect("the issue is closed");
    assert!(page.says(" of 14"), "the filter stays: {}", said(&page));
    page.press("escape").expect("escape clears what is typed");
    page.wait_for(WITHIN, |page| !page.says(" of 14"))
        .expect("the filter is gone again");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_filter_that_lets_nothing_through_says_so() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("/").expect("slash");
    page.type_text("zzzz").expect("the filter is typed");
    page.wait_for(WITHIN, |page| page.says("No issue matches"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn refresh_asks_linear_again_by_the_button_and_by_the_key() {
    let (linear, _session, mut page) = open_lists(WIDE);
    let before = linear.times("MyIssues");
    page.click("refresh").expect("refresh is pressed");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") > before)
        .expect("Linear was asked again");
    let between = linear.times("MyIssues");
    page.advance(0.5).expect("a moment");
    page.press("r").expect("r");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") > between)
        .expect("Linear was asked once more");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn an_empty_list_of_mine_leads_to_the_others() {
    let linear = Canned::new(Mode::NothingAssigned);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page =
        open(&session, WIDE, None, "Nothing is assigned to you").expect("the page opens");
    page.click("show-all").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .expect("all issues are shown");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn without_a_key_the_page_asks_for_one_and_keeps_what_linear_takes() {
    let dir = config_dir("connect");
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, None, Some(dir.clone()));
    let mut page = open(&session, WIDE, None, "Connect Linear").expect("the page opens");
    assert!(
        page.says("A key with Read access shows your issues."),
        "{}",
        said(&page)
    );
    assert!(
        page.says("To make new ones it needs Create issues too."),
        "{}",
        said(&page)
    );
    assert_eq!(
        linear.times("Verify"),
        0,
        "nothing is asked before there is a key"
    );
    page.type_text("lin_api_typed").expect("the key is typed");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert_eq!(linear.last_key().as_deref(), Some("lin_api_typed"));
    assert!(dir.join("key").is_file(), "the key is kept");
    std::fs::remove_dir_all(&dir).expect("the folder of this test is removed");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_key_linear_refuses_is_said_on_the_connect_screen() {
    let dir = config_dir("refused");
    let linear = Canned::new(Mode::Refused);
    let session = session_over(&linear, None, Some(dir.clone()));
    let mut page = open(&session, WIDE, None, "Connect Linear").expect("the page opens");
    page.type_text("lin_api_wrong").expect("the key is typed");
    page.click("connect").expect("connect is pressed");
    page.wait_for(WITHIN, |page| page.says("did not accept"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(page.says("Connect Linear"), "{}", said(&page));
    assert!(!dir.join("key").exists(), "a refused key is not kept");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_linear_that_is_not_there_is_an_error_with_the_way_to_try_again() {
    let linear = Canned::new(Mode::Down);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, WIDE, None, "Linear did not answer").expect("the page opens");
    assert!(page.says("could not be reached"), "{}", said(&page));
    linear.set(Mode::Fine);
    page.click("retry").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn an_80_by_24_pane_keeps_the_list_and_the_filter_inside_it() {
    let (_linear, _session, page) = open_lists(PANE);
    assert!(page.says("ACM-412"), "{}", said(&page));
    for name in ["scope", "filter/field"] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is laid out"));
        assert!(
            at.x + at.w <= PANE.0 + 0.5 && at.y + at.h <= PANE.1 + 0.5,
            "{name}: {at:?}"
        );
    }
    // No room for the button: its key is on the key line, and the filter has a row of its own.
    assert!(page.bounds("refresh").is_none(), "{}", said(&page));
    assert!(page.says("refresh"), "{}", said(&page));
    let scope = page.bounds("scope").expect("the control is laid out");
    let filter = page.bounds("filter/field").expect("the field is laid out");
    assert!(filter.y >= scope.y + scope.h, "{scope:?} {filter:?}");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_first_asking_shows_the_lines_to_come_and_what_comes_replaces_them() {
    let linear = Canned::new(Mode::Held);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, WIDE, None, "Loading").expect("the page opens");
    assert!(!page.says("ACM-412"), "{}", said(&page));
    linear.set(Mode::Fine);
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
        .expect("the list comes");
    assert!(!page.says("Loading"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn letters_typed_in_the_filter_are_its_own_and_the_buttons_answer_the_keyboard() {
    let (linear, session, mut page) = open_lists(WIDE);
    page.press("/").expect("slash");
    // `r`, `1` and `2` are commands of the list, and in the field they are letters.
    page.type_text("r12").expect("the filter is typed");
    page.wait_for(WITHIN, |page| page.says("No issue matches"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert_eq!(linear.times("MyIssues"), 1, "nothing asked Linear again");
    assert!(page.says("My issues 14"), "{}", said(&page));
    page.press("escape").expect("escape clears");
    page.press("escape").expect("escape leaves the field");
    // Tab brings the keyboard to a button (the control of the scope, the filter, then Refresh), and
    // Enter presses it.
    for _ in 0..3 {
        page.press("tab").expect("tab");
    }
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") == 2)
        .expect("Enter on the button asked Linear again");
    page.wait_for(WITHIN, |_| !session.state().refreshing)
        .expect("the answer came");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn after_the_button_of_an_empty_list_is_clicked_enter_opens_an_issue() {
    let linear = Canned::new(Mode::NothingAssigned);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page =
        open(&session, WIDE, None, "Nothing is assigned to you").expect("the page opens");
    page.click("show-all").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .expect("all issues are shown");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("Enter opens the issue: {why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn after_the_control_of_the_scope_is_clicked_the_arrows_and_enter_are_the_lists() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.click("scope/item/all")
        .expect("the control is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .expect("all issues are shown");
    // The cursor was on ACM-412 and the arrow is the list's: it goes on, and Enter opens that issue.
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("Enter opens the issue: {why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn after_refresh_is_clicked_enter_opens_the_issue_and_asks_linear_nothing() {
    let (linear, session, mut page) = open_lists(WIDE);
    page.click("refresh").expect("refresh is pressed");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") == 2)
        .expect("Linear was asked again");
    page.wait_for(WITHIN, |_| !session.state().refreshing)
        .expect("the answer came");
    page.advance(0.5).expect("a moment");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("Enter opens the issue: {why:?}: {}", said(&page)));
    assert_eq!(linear.times("MyIssues"), 2, "Linear was not asked again");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn after_the_button_that_closes_the_issue_is_clicked_enter_opens_it_again() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    open_the_issue(&mut page);
    page.click("panel-close").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_none())
        .expect("the issue is closed");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("Enter opens the issue again: {why:?}: {}", said(&page)));
}

/// Opens the issue under the cursor with Enter, and waits for what the program tells of it.
fn open_the_issue(page: &mut Page) {
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.says("API key"))
        .unwrap_or_else(|why| panic!("the description comes: {why:?}: {}", said(page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn enter_opens_the_issue_beside_the_list_and_escape_closes_it() {
    let (linear, _session, mut page) = open_lists(WIDE);
    assert!(page.bounds("panel-close").is_none(), "{}", said(&page));
    open_the_issue(&mut page);
    // What the row did not carry: the description, the comments, the cycle and the way out.
    for words in [
        "600 requests a minute",
        "COMMENTS",
        "Cycle 12",
        "Open in Linear",
        "Close",
    ] {
        assert!(page.says(words), "{words}: {}", said(&page));
    }
    // The list stays beside it.
    assert!(page.says("ACM-398"), "{}", said(&page));
    assert_eq!(linear.times("Issue"), 1);
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_none())
        .expect("the issue is closed");
    assert!(page.says("ACM-398"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_double_press_on_an_issue_opens_it_and_the_button_closes_it() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.double_click("row:i:issue-412")
        .expect("the line is pressed twice");
    page.wait_for(WITHIN, |page| page.says("600 requests a minute"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.click("panel-close").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_none())
        .expect("the issue is closed");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_issue_open_follows_the_cursor_and_is_asked_once_for_each_issue() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_the_issue(&mut page);
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    page.wait_for(WITHIN, |_| linear.times("Issue") == 2)
        .expect("the program is asked for the next issue");
    page.press("up").expect("up");
    cursor_on(&mut page, "ACM-412");
    page.advance(0.5).expect("a moment");
    // The first is kept: no second asking for it.
    assert_eq!(linear.times("Issue"), 2);
    // A group's head has no issue: the one open stays.
    page.press("home").expect("home");
    cursor_on(&mut page, "In Progress");
    assert!(page.bounds("panel-close").is_some(), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_link_of_the_issue_opens_by_the_key_and_by_the_button() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    // No issue open, no link.
    page.press("o").expect("o");
    assert!(page.links().is_empty(), "{:?}", page.links());
    open_the_issue(&mut page);
    let url = "https://linear.app/acme/issue/ACM-412/add-rate-limiting-to-the-public-api";
    page.press("o").expect("o");
    page.wait_for(WITHIN, |page| page.links().len() == 1)
        .expect("the key opens the issue in Linear");
    assert_eq!(page.links()[0], url);
    page.click("open-linear").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.links().len() == 2)
        .expect("the button opens it too");
    assert_eq!(page.links()[1], url);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn x_selects_the_line_shift_arrows_select_more_and_escape_clears() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    assert!(!page.says("selected"), "{}", said(&page));
    page.press("x").expect("x");
    page.wait_for(WITHIN, |page| page.says("1 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // The cursor stays where it was: x does not move it.
    assert!(cursor(&page).contains("ACM-412"), "{}", cursor(&page));
    page.press("x").expect("x again");
    page.wait_for(WITHIN, |page| !page.says("selected"))
        .expect("x takes it off");
    page.press("shift+down").expect("shift+down");
    page.press("shift+down").expect("shift+down");
    page.wait_for(WITHIN, |page| page.says("3 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("selected"))
        .expect("escape clears the selection");
    page.press("x").expect("x");
    page.click("clear-selection")
        .expect("the button is pressed");
    page.wait_for(WITHIN, |page| !page.says("selected"))
        .expect("the button clears it");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_box_of_a_line_selects_it_and_the_selection_keeps_to_the_list_that_is_shown() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    // The box shows where the pointer is over the line.
    page.hover("row:i:issue-398")
        .expect("the pointer is over the line");
    page.click("check:i:issue-398").expect("the box is pressed");
    page.wait_for(WITHIN, |page| page.says("1 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // A filter that lets it through keeps it; one that does not, takes it away.
    page.press("/").expect("slash");
    page.type_text("rate").expect("the filter is typed");
    page.wait_for(WITHIN, |page| page.says("1 of 14"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(!page.says("selected"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn command_a_selects_every_issue_shown_but_not_in_the_filter_field() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("mod+a").expect("mod+a");
    page.wait_for(WITHIN, |page| page.says("selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // The folded backlog's issues are not shown, so they are not chosen: twelve of the fourteen.
    assert!(page.says("12 selected"), "{}", said(&page));
    page.press("escape").expect("escape");
    page.press("/").expect("slash");
    page.type_text("bug").expect("the filter is typed");
    page.press("mod+a").expect("mod+a in the field");
    page.advance(0.3).expect("a moment");
    assert!(!page.says("selected"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_palette_finds_a_command_and_does_it_and_takes_words_as_a_filter() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("mod+k").expect("mod+k");
    page.wait_for(WITHIN, |page| page.says("Do it"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    for words in ["Show all issues", "Ask Linear again", "Select the issue"] {
        assert!(page.says(words), "{words}: {}", said(&page));
    }
    // The cursor's own keys are on the card, not here.
    assert!(!page.says("Next issue"), "{}", said(&page));
    // No issue is open: no link to follow.
    assert!(!page.says("Open in Linear"), "{}", said(&page));
    page.type_text("show all").expect("the query is typed");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.says("ACM-417") && !page.says("Do it"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // Words that are no command's are a filter.
    page.press("mod+k").expect("mod+k");
    page.wait_for(WITHIN, |page| page.says("Do it"))
        .expect("the palette opens again");
    page.type_text("quota").expect("the query is typed");
    page.wait_for(WITHIN, |page| page.says("Filter issues by"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| !page.says("Do it") && page.says(" of 19"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // Escape closes it, and does nothing else.
    page.press("mod+k").expect("mod+k");
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("Do it"))
        .expect("escape closes the palette");
    assert!(page.says(" of 19"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_card_of_keys_lists_what_the_page_can_do_and_closes() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("?").expect("?");
    page.wait_for(WITHIN, |page| page.says("Keys"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    for words in [
        "Next issue",
        "Select all",
        "Show all issues",
        "Find a command",
    ] {
        assert!(page.says(words), "{words}: {}", said(&page));
    }
    page.click("help-close").expect("the button is pressed");
    page.wait_for(WITHIN, |page| !page.says("Select all"))
        .expect("the card is closed");
    // The status line's own word for it opens it by the pointer.
    page.click("status/item/help")
        .expect("the status line's key");
    page.wait_for(WITHIN, |page| page.says("Select all"))
        .expect("the card is open again");
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("Select all"))
        .expect("escape closes it");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_an_80_by_24_pane_the_card_of_keys_cuts_no_line_and_the_keyboard_scrolls_it() {
    let (_linear, _session, mut page) = open_lists(PANE);
    page.press("?").expect("?");
    page.wait_for(WITHIN, |page| page.says("Keys"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // One column: the longest line of the card is whole.
    assert!(
        page.says("Work on the issue, or the ones selected"),
        "{}",
        said(&page)
    );
    let (move_keys, issue_keys) = (
        page.bounds("help/group/Move").expect("the first group"),
        page.bounds("help/group/Issue").expect("a later group"),
    );
    assert!(issue_keys.y > move_keys.y, "one under the other");
    assert!((issue_keys.x - move_keys.x).abs() < 0.5, "in one column");
    // The card is taller than the pane: the arrows bring the rest of it, and take it back. (The
    // card has come to rest first: it glides in.)
    page.advance(0.6).expect("a moment");
    let top = |page: &Page| page.bounds("help/group/Move").map(|at| at.y);
    let before = top(&page).expect("the first group is in view");
    page.press("down").expect("down");
    page.advance(0.6).expect("a moment");
    page.wait_for(WITHIN, |page| top(page).is_some_and(|y| y < before - 1.0))
        .unwrap_or_else(|why| panic!("down scrolls the card: {why:?}: {:?}", top(&page)));
    assert!(page.says("Keys"), "the card stays open: {}", said(&page));
    page.press("up").expect("up");
    page.advance(0.6).expect("a moment");
    page.wait_for(WITHIN, |page| {
        top(page).is_some_and(|y| (y - before).abs() < 1.0)
    })
    .unwrap_or_else(|why| panic!("up scrolls it back: {why:?}: {:?}", top(&page)));
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("Select all"))
        .expect("escape closes it");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_a_narrow_pane_the_issue_takes_the_pane_and_the_way_back_is_one_press() {
    let (_linear, _session, mut page) = open_lists(PANE);
    open_the_issue(&mut page);
    // The list gives way: the pane is the issue's.
    assert!(!page.says("ACM-398"), "{}", said(&page));
    for name in ["panel-close", "open-linear"] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is laid out"));
        assert!(at.x >= 0.0 && at.x + at.w <= PANE.0 + 0.5, "{name}: {at:?}");
    }
    // The key line stays under it, inside the pane.
    let keys = page
        .bounds("status/item/help")
        .expect("the key line is laid out");
    assert!(keys.y + keys.h <= PANE.1 + 0.5, "{keys:?}");
    page.click("panel-close").expect("the button is pressed");
    page.wait_for(WITHIN, |page| page.says("ACM-398"))
        .expect("the list is back");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn space_scrolls_the_issue_open_and_shift_space_scrolls_it_back() {
    let (_linear, _session, mut page) = open_lists(PANE);
    open_the_issue(&mut page);
    // The comments are under the foot of an 80 by 24 pane: the keyboard has to reach them.
    let top = |page: &Page| {
        page.bounds("panel-comments")
            .expect("the comments are laid out")
            .y
    };
    let before = top(&page);
    page.press("space").expect("space");
    page.advance(0.6).expect("a moment");
    page.wait_for(WITHIN, |page| top(page) < before - 1.0)
        .unwrap_or_else(|why| panic!("the issue scrolls down: {why:?}: {}", top(&page)));
    page.press("shift+space").expect("shift and space");
    page.advance(0.6).expect("a moment");
    page.wait_for(WITHIN, |page| (top(page) - before).abs() < 1.0)
        .unwrap_or_else(|why| panic!("the issue scrolls back: {why:?}: {}", top(&page)));
    // The key is said where the issue is open, and no key of a line that is not on the page.
    assert!(page.says("scroll"), "{}", said(&page));
    assert!(!page.says("select"), "{}", said(&page));
}

// --- Handing issues to an agent ------------------------------------------------------------------

/// What runs `git` and `bateri` in a test: nothing runs. It answers as they would and keeps what it
/// was asked; `bateri wait` returns when the test lets the agent end.
pub(crate) struct Hands {
    asked: Mutex<Vec<String>>,
    repo: Mutex<bool>,
    split_says: Mutex<Option<String>>,
    ended: Mutex<bool>,
}

impl Hands {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            asked: Mutex::new(Vec::new()),
            repo: Mutex::new(true),
            split_says: Mutex::new(None),
            ended: Mutex::new(false),
        })
    }

    /// What was run, a line each (`bateri wait p41` is one of them).
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    /// The agent's pane ends.
    pub(crate) fn end(&self) {
        *self.ended.lock().unwrap() = true;
    }
}

impl crate::work::runner::Runner for Hands {
    fn run(
        &self,
        program: &str,
        args: &[String],
        _: Option<&std::path::Path>,
    ) -> std::io::Result<crate::work::runner::Ran> {
        use crate::work::runner::Ran;
        self.asked
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        let said = |ok: bool, out: &str, err: &str| {
            Ok(Ran {
                ok,
                out: out.to_owned(),
                err: err.to_owned(),
            })
        };
        let first = args.first().map(String::as_str);
        match (program, first) {
            ("bateri", Some("wait")) => {
                while !*self.ended.lock().unwrap() {
                    std::thread::sleep(Duration::from_millis(20));
                }
                said(true, "", "")
            }
            ("bateri", Some("split" | "tab")) => match self.split_says.lock().unwrap().clone() {
                Some(words) => said(false, "", &words),
                None => said(true, "p41\n", ""),
            },
            ("git", Some("rev-parse"))
                if args.get(1).map(String::as_str) == Some("--show-toplevel") =>
            {
                if *self.repo.lock().unwrap() {
                    said(true, "/work/acme-app\n", "")
                } else {
                    said(false, "", "fatal: not a git repository")
                }
            }
            ("git", Some("rev-parse")) => said(false, "", ""),
            _ => said(true, "", ""),
        }
    }
}

/// A machine for the page to hand issues over on: bateri and two agents in a `PATH`, the pen
/// begun in `/work/acme-app/api`, a folder for the files, and `Hands` for the commands.
pub(crate) struct Rig {
    pub(crate) hands: Arc<Hands>,
    pub(crate) session: Arc<Session>,
    _state: tempfile::TempDir,
    _bin: tempfile::TempDir,
}

pub(crate) fn rig(linear: &Arc<Canned>, in_bateri: bool, agents: &[&str]) -> Rig {
    use std::os::unix::fs::PermissionsExt;
    let bin = tempfile::tempdir().unwrap();
    for name in std::iter::once(&"bateri").chain(agents) {
        let file = bin.path().join(name);
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let state = tempfile::tempdir().unwrap();
    let hands = Hands::new();
    let session = Session::new(
        Arc::clone(linear) as Arc<dyn Transport>,
        Paths {
            config: None,
            state: Some(state.path().to_owned()),
        },
        Some("lin_api_test"),
        Duration::from_secs(3600),
    )
    .with_work(
        crate::work::Setup {
            path: vec![bin.path().to_owned()],
            in_bateri,
            folder: Some(PathBuf::from("/work/acme-app/api")),
        },
        Arc::clone(&hands) as Arc<dyn crate::work::runner::Runner>,
    );
    Rig {
        hands,
        session: Arc::new(session),
        _state: state,
        _bin: bin,
    }
}

/// The page over a rig, with the cursor on the first issue.
fn open_rig(size: (f32, f32), in_bateri: bool, agents: &[&str]) -> (Rig, Page) {
    let linear = Canned::new(Mode::Fine);
    let rig = rig(&linear, in_bateri, agents);
    let page = open(&rig.session, size, None, "ACM-412").expect("the page opens");
    (rig, page)
}

/// Opens the form with `w` and waits until the program has said what can be chosen.
fn open_form(page: &mut Page) {
    page.press("w").expect("w");
    page.wait_for(WITHIN, |page| page.says("Open in"))
        .unwrap_or_else(|why| panic!("the form opens: {why:?}: {}", said(page)));
    // (The keyboard is in the title a moment after the form is drawn.)
    page.advance(0.3).expect("a moment");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn w_opens_the_form_and_start_hands_the_issue_to_the_agent_and_marks_it_until_the_pane_ends() {
    let (rig, mut page) = open_rig(WIDE, true, &["claude", "codex"]);
    assert!(page.says("Work on issue"), "the button: {}", said(&page));
    open_form(&mut page);
    for words in [
        "Agent",
        "Claude",
        "Codex",
        "This folder",
        "Split right",
        "New tab",
        "Start Claude",
    ] {
        assert!(page.says(words), "{words}: {}", said(&page));
    }
    // An agent that is not installed is not offered.
    assert!(!page.says("Cursor"), "{}", said(&page));
    page.type_text("Look at the middleware")
        .expect("the words are typed");
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |page| !page.says("Open in") && page.says("Claude"))
        .unwrap_or_else(|why| {
            panic!(
                "the form is done and the row is marked: {why:?}: {}",
                said(&page)
            )
        });
    let asked = rig.hands.asked();
    let split = asked
        .iter()
        .find(|line| line.starts_with("bateri split"))
        .unwrap_or_else(|| panic!("a pane was asked for: {asked:?}"));
    assert!(
        split.starts_with("bateri split --right --cwd /work/acme-app/api --label ACM-412 -- claude Work on the Linear issues in "),
        "{split}"
    );
    assert!(split.ends_with("Look at the middleware"), "{split}");
    // The issue's text is in the file, not in the line.
    assert!(!split.contains("What we want"), "{split}");
    // The row says who works on it, until the pane's program ends.
    assert!(page.says("ACM-412"), "{}", said(&page));
    rig.hands.end();
    page.wait_for(WITHIN, |page| !page.says("Claude"))
        .unwrap_or_else(|why| panic!("the mark goes: {why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn what_is_selected_is_what_the_form_is_for_and_a_box_lets_one_go() {
    let (rig, mut page) = open_rig(WIDE, true, &["claude"]);
    page.press("x").expect("x");
    page.press("shift+down").expect("shift+down");
    page.wait_for(WITHIN, |page| page.says("Work on 2 issues"))
        .unwrap_or_else(|why| panic!("the button says how many: {why:?}: {}", said(&page)));
    page.click("work").expect("the header button is pressed");
    page.wait_for(WITHIN, |page| page.says("Open in"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(page.says("Work on 2 issues"), "{}", said(&page));
    // The one box lets ACM-398 go; the selection in the list follows.
    page.click("work-check:issue-398")
        .expect("the box is pressed");
    page.wait_for(WITHIN, |page| page.says("1 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |page| !page.says("Open in"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // Started: what was selected is let go of.
    page.wait_for(WITHIN, |page| !page.says("selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(
        rig.hands
            .asked()
            .iter()
            .any(|line| line.starts_with("bateri split"))
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_keyboard_lets_an_issue_go_from_the_form_and_the_card_of_keys_is_not_said_there() {
    let (_rig, mut page) = open_rig(WIDE, true, &["claude"]);
    page.press("x").expect("x");
    page.press("shift+down").expect("shift+down");
    page.press("shift+down").expect("shift+down");
    open_form(&mut page);
    assert!(page.says("Work on 3 issues"), "{}", said(&page));
    // In the form `?` is a character of the words, so the foot does not offer the card of keys.
    assert!(page.says("next field"), "{}", said(&page));
    assert!(!page.words().contains(&"?"), "{}", said(&page));
    // From the words for the agent, Shift Tab goes up to the last of the boxes, and the space bar
    // lets that issue go.
    page.press("shift+tab").expect("shift+tab");
    page.press("space").expect("space");
    page.wait_for(WITHIN, |page| page.says("Work on 2 issues"))
        .unwrap_or_else(|why| panic!("an issue is let go: {why:?}: {}", said(&page)));
    assert!(page.says("2 selected"), "{}", said(&page));
    assert!(!page.says("ACM-405 Webhook"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn tab_goes_on_to_the_next_field_arrows_choose_and_escape_puts_the_form_away() {
    let (rig, mut page) = open_rig(WIDE, true, &["claude", "codex"]);
    open_form(&mut page);
    // From the words for the agent, Tab goes to the agents; the arrow chooses Codex.
    page.press("tab").expect("tab");
    page.press("right").expect("right");
    page.wait_for(WITHIN, |page| page.says("Start Codex"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // The pointer chooses too, and the new tab is a way to open it.
    page.click("work-place/item/tab")
        .expect("the segment is pressed");
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |_| {
        rig.hands
            .asked()
            .iter()
            .any(|line| line.starts_with("bateri tab --cwd"))
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", rig.hands.asked()));
    assert!(
        rig.hands
            .asked()
            .iter()
            .any(|line| line.contains(" -- codex ")),
        "{:?}",
        rig.hands.asked()
    );
    // Escape puts the form away (it is a form again, from the key).
    rig.hands.end();
    page.wait_for(WITHIN, |page| !page.says("Open in"))
        .expect("the form is done");
    open_form(&mut page);
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("Open in"))
        .expect("escape closes the form");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_new_worktree_is_offered_in_a_repository_and_the_reason_is_said_outside_one() {
    let (rig, mut page) = open_rig(WIDE, true, &["claude"]);
    open_form(&mut page);
    assert!(page.says("New worktree"), "{}", said(&page));
    page.click("work-folder/item/worktree")
        .expect("the segment is pressed");
    // (The note is one line, and the kit cuts what does not fit, so the start of the name is said.)
    page.wait_for(WITHIN, |page| page.says("branch ada/acm-412-add-rate"))
        .unwrap_or_else(|why| panic!("the branch is said: {why:?}: {}", said(&page)));
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |_| {
        rig.hands
            .asked()
            .iter()
            .any(|line| line.starts_with("bateri split"))
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", rig.hands.asked()));
    let asked = rig.hands.asked();
    assert!(
        asked.iter().any(|line| line
            == "git worktree add -b ada/acm-412-add-rate-limiting-to-the-public-api /work/acme-app-acm-412 HEAD"),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|line| line.starts_with("bateri split --right --cwd /work/acme-app-acm-412 ")),
        "{asked:?}"
    );
    // Outside a repository there is no such choice, and the page says why.
    let (rig, mut page) = open_rig(WIDE, true, &["claude"]);
    *rig.hands.repo.lock().unwrap() = false;
    open_form(&mut page);
    assert!(!page.says("New worktree"), "{}", said(&page));
    // (A note that wraps is told in the pieces of its lines.)
    assert!(
        page.says("No new worktree: this folder is not in a git"),
        "{}",
        said(&page)
    );
    assert!(page.says("repository"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn where_nothing_can_be_started_the_form_says_why_and_start_does_nothing() {
    // Not in bateri.
    let (rig, mut page) = open_rig(WIDE, false, &["claude"]);
    open_form(&mut page);
    assert!(page.says("not in a bateri pane"), "{}", said(&page));
    page.press("mod+enter").expect("mod+enter");
    page.click("work-start").ok();
    page.advance(0.5).expect("a moment");
    assert!(
        rig.hands
            .asked()
            .iter()
            .all(|line| !line.starts_with("bateri")),
        "{:?}",
        rig.hands.asked()
    );
    // No agent.
    let (_rig, mut page) = open_rig(WIDE, true, &[]);
    open_form(&mut page);
    assert!(page.says("No agent is installed"), "{}", said(&page));
    assert!(page.says("None installed"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn where_bateri_makes_no_pane_the_form_stays_and_says_so() {
    let (rig, mut page) = open_rig(WIDE, true, &["claude"]);
    *rig.hands.split_says.lock().unwrap() =
        Some("bateri: the bateri app is not connected".to_owned());
    open_form(&mut page);
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |page| page.says("not connected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(
        page.says("Open in"),
        "the form is still open: {}",
        said(&page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_an_80_by_24_pane_the_form_takes_the_pane_and_keeps_start_inside_it() {
    let (_rig, mut page) = open_rig(PANE, true, &["claude", "codex"]);
    // The button is in the head, which has room for it.
    let button = page.bounds("work").expect("the button is laid out");
    assert!(button.x + button.w <= PANE.0 + 0.5, "{button:?}");
    open_form(&mut page);
    assert!(!page.says("ACM-398"), "the list gives way: {}", said(&page));
    for name in ["work-close", "work-start"] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is laid out"));
        assert!(
            at.x >= 0.0 && at.x + at.w <= PANE.0 + 0.5 && at.y + at.h <= PANE.1 + 0.5,
            "{name}: {at:?}"
        );
    }
}

// --- A new issue ---------------------------------------------------------------------------------

/// Opens the form with `c` and waits until the program has said what can be chosen.
fn open_new(page: &mut Page) {
    page.press("c").expect("c");
    page.wait_for(WITHIN, |page| page.bounds("new-value:state").is_some())
        .unwrap_or_else(|why| panic!("the form opens: {why:?}: {}", said(page)));
}

/// The titles of the entries of the menu that is open.
fn menu_titles(page: &Page) -> Vec<String> {
    page.menu()
        .expect("a menu is open")
        .entries
        .iter()
        .filter_map(|entry| match entry {
            pencere_view::MenuEntry::Item { title, .. } => Some(title.clone()),
            _ => None,
        })
        .collect()
}

/// Presses the value `kind` and picks the entry titled `title` of its menu.
fn pick_value(page: &mut Page, kind: &str, title: &str) {
    page.click(&format!("new-value:{kind}"))
        .unwrap_or_else(|why| panic!("{kind} is pressed: {why:?}"));
    page.pick_titled(title)
        .unwrap_or_else(|why| panic!("{title:?} is picked: {why:?}: {:?}", menu_titles(page)));
}

/// Waits until `linear` has been asked for `count` issues.
fn made(linear: &Arc<Canned>, page: &mut Page, count: usize) {
    page.wait_for(WITHIN, |_| linear.created().len() >= count)
        .unwrap_or_else(|why| {
            panic!(
                "Linear is asked: {why:?}: {:?}: {}",
                linear.created(),
                said(page)
            )
        });
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn c_opens_the_form_on_the_title_and_create_waits_for_one() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    // The form is beside the list, and the keyboard is on the title: letters are its own.
    assert!(page.says("Create issue"), "{}", said(&page));
    page.type_text("cr1w").expect("the title is typed");
    assert_eq!(page.field("new-title/field"), Some("cr1w"));
    // (Nothing of the list's was done: the form is still the form.)
    assert!(page.says("Create issue"), "{}", said(&page));
    // With no title there is nothing to make: the key and the button do nothing.
    for _ in 0..4 {
        page.press("backspace").expect("backspace");
    }
    page.press("mod+enter").expect("mod+enter");
    page.click("new-create").ok();
    page.advance(0.5).expect("a moment");
    assert!(linear.created().is_empty(), "{:?}", linear.created());
    assert_eq!(linear.times("IssueCreate"), 0);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn enter_in_the_title_goes_to_the_description_and_tab_goes_through_the_values() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    page.type_text("Retry the webhook").expect("the title");
    page.press("enter").expect("enter");
    assert!(
        linear.created().is_empty(),
        "Enter in the title does not make the issue"
    );
    // The description has the keyboard: the page's keys are letters there.
    page.type_text("wcr12 it fails").expect("the description");
    assert_eq!(
        page.editor_text("new-about1").as_deref(),
        Some("wcr12 it fails")
    );
    // Tab goes through the values, in the order they stand, each one opening its menu on Enter.
    for (kind, first) in [
        ("team", "WEB web"),
        ("state", "In Progress"),
        ("priority", "No priority"),
        ("assignee", "Me"),
        ("labels", "api"),
        ("project", "No project"),
    ] {
        page.press("tab").expect("tab");
        page.press("enter").expect("enter");
        let titles = menu_titles(&page);
        assert!(
            titles.iter().any(|title| title.ends_with(first)),
            "{kind}: {titles:?}"
        );
        page.press("escape").expect("escape closes the menu");
    }
    // Then Create (it can be pressed: there is a title), then Close, then the title again.
    page.press("tab").expect("tab");
    page.press("tab").expect("tab");
    page.press("tab").expect("tab");
    page.type_text("!").expect("the title again");
    assert_eq!(
        page.field("new-title/field"),
        Some("Retry the webhook!"),
        "{}",
        said(&page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_value_opens_its_menu_and_the_pick_is_written_to_the_control() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    page.click("new-value:priority").expect("priority");
    assert_eq!(
        menu_titles(&page),
        ["\u{2713} No priority", "Urgent", "High", "Medium", "Low"]
    );
    page.pick_titled("High").expect("High");
    page.wait_for(WITHIN, |page| {
        page.words_in("new-value:priority")
            .contains(&"High".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:priority")));
    // Status offers the open ones, in the order of their kinds, and the team's own is marked.
    page.click("new-value:state").expect("state");
    assert_eq!(
        menu_titles(&page),
        ["Backlog", "\u{2713} Todo", "In Progress", "In Review"]
    );
    page.pick_titled("In Progress").expect("In Progress");
    page.wait_for(WITHIN, |page| {
        page.words_in("new-value:state")
            .contains(&"In Progress".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:state")));
    // The person is the assignee to begin with, the others who are active follow; one can be no one.
    page.click("new-value:assignee").expect("assignee");
    assert_eq!(menu_titles(&page), ["\u{2713} Me", "Deniz", "No one"]);
    page.pick_titled("No one").expect("No one");
    page.wait_for(WITHIN, |page| {
        page.words_in("new-value:assignee")
            .contains(&"Assignee".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:assignee")));
    // A project that is done is not offered; the issue may have none.
    page.click("new-value:project").expect("project");
    assert_eq!(menu_titles(&page), ["\u{2713} No project", "Billing"]);
    page.pick_titled("Billing").expect("Billing");
    page.wait_for(WITHIN, |page| {
        page.words_in("new-value:project")
            .contains(&"Billing".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:project")));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn labels_are_picked_one_at_a_time_and_a_second_pick_takes_one_off() {
    let (_linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    // (A group of labels is not offered; those of the workspace are, with the team's.)
    page.click("new-value:labels").expect("labels");
    assert_eq!(menu_titles(&page), ["api", "bug", "Customer"]);
    page.pick_titled("bug").expect("bug");
    pick_value(&mut page, "labels", "api");
    page.wait_for(WITHIN, |page| {
        let words = page.words_in("new-value:labels");
        words.contains(&"bug".to_owned()) && words.contains(&"api".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:labels")));
    // They are said in the order they were chosen.
    let words = page.words_in("new-value:labels");
    assert!(
        words.iter().position(|word| word == "bug") < words.iter().position(|word| word == "api"),
        "{words:?}"
    );
    page.click("new-value:labels").expect("labels");
    assert_eq!(
        menu_titles(&page),
        ["\u{2713} api", "\u{2713} bug", "Customer"]
    );
    page.pick_titled("\u{2713} bug").expect("bug off");
    page.wait_for(WITHIN, |page| {
        let words = page.words_in("new-value:labels");
        !words.contains(&"bug".to_owned()) && words.contains(&"api".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {:?}", page.words_in("new-value:labels")));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_value_with_nothing_to_choose_is_not_drawn() {
    let linear = Canned::new(Mode::Fine);
    linear.set_bare();
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, WIDE, None, "ACM-412").expect("the page opens");
    open_new(&mut page);
    for name in ["new-value:team", "new-value:labels", "new-value:project"] {
        assert!(page.bounds(name).is_none(), "{name}: {}", said(&page));
    }
    for name in [
        "new-value:state",
        "new-value:priority",
        "new-value:assignee",
    ] {
        assert!(page.bounds(name).is_some(), "{name}: {}", said(&page));
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn command_enter_makes_the_issue_once_and_it_is_under_the_cursor_with_the_panel_open() {
    let (linear, _session, mut page) = open_lists(WIDE);
    // A filter is on: the issue made must not be hidden by it.
    page.press("/").expect("slash");
    page.type_text("rate").expect("the filter");
    page.wait_for(WITHIN, |page| page.says(" of 14"))
        .expect("the filter is on");
    page.press("escape").expect("escape");
    page.press("escape").expect("escape");
    open_new(&mut page);
    page.type_text("Retry the webhook").expect("the title");
    page.press("enter").expect("enter");
    page.type_text("It fails twice.").expect("the description");
    pick_value(&mut page, "priority", "High");
    pick_value(&mut page, "labels", "bug");
    pick_value(&mut page, "project", "Billing");
    // The group of Backlog is folded in the list: the issue goes there, and the group opens.
    pick_value(&mut page, "state", "Backlog");
    page.press("mod+enter").expect("mod+enter");
    // (A second press while it is on its way makes nothing.)
    page.press("mod+enter").ok();
    made(&linear, &mut page, 1);
    cursor_on(&mut page, "ACM-900");
    page.wait_for(WITHIN, |page| page.bounds("panel-close").is_some())
        .unwrap_or_else(|why| panic!("the issue opens: {why:?}: {}", said(&page)));
    assert!(page.bounds("new-create").is_none(), "{}", said(&page));
    assert!(page.says("Retry the webhook"), "{}", said(&page));
    assert!(!page.says(" of 14"), "the filter is off: {}", said(&page));
    let sent = linear.created();
    assert_eq!(sent.len(), 1, "{sent:?}");
    let input = &sent[0];
    assert_eq!(input["id"].as_str().map(str::len), Some(36), "{input}");
    assert_eq!(input["teamId"], "team-acm");
    assert_eq!(input["title"], "Retry the webhook");
    assert_eq!(input["description"], "It fails twice.");
    assert_eq!(input["stateId"], "s-back");
    assert_eq!(input["priority"], 2);
    assert_eq!(input["assigneeId"], "u-ada");
    assert_eq!(input["labelIds"], serde_json::json!(["l-bug"]));
    assert_eq!(input["projectId"], "p-1");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn an_issue_for_someone_else_is_shown_in_all_issues_and_the_next_draft_begins_empty() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    page.type_text("For Deniz").expect("the title");
    pick_value(&mut page, "assignee", "Deniz");
    page.click("new-create").expect("Create is pressed");
    made(&linear, &mut page, 1);
    cursor_on(&mut page, "ACM-900");
    assert!(page.says("All issues"), "{}", said(&page));
    assert_eq!(linear.created()[0]["assigneeId"], "u-deniz");
    // A new draft: the project and the team of this one, and nothing else.
    page.press("escape").ok();
    open_new(&mut page);
    page.advance(0.3).expect("a moment");
    assert_eq!(page.field("new-title/field").unwrap_or(""), "");
    assert_eq!(page.editor_text("new-about2").as_deref(), Some(""));
    page.type_text("Another").expect("the title");
    page.press("mod+enter").expect("mod+enter");
    made(&linear, &mut page, 2);
    let sent = linear.created();
    assert_ne!(sent[0]["id"], sent[1]["id"], "each draft has its own id");
    assert!(sent[1].get("description").is_none(), "{sent:?}");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn escape_keeps_the_draft_and_c_brings_it_back() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    page.type_text("Keep this").expect("the title");
    page.press("enter").expect("enter");
    page.type_text("and this").expect("the description");
    pick_value(&mut page, "priority", "Low");
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| page.bounds("new-create").is_none())
        .unwrap_or_else(|why| panic!("the form is put away: {why:?}: {}", said(&page)));
    assert!(page.says("ACM-412"), "{}", said(&page));
    open_new(&mut page);
    page.advance(0.6).expect("a moment");
    page.press("mod+enter").expect("mod+enter");
    made(&linear, &mut page, 1);
    let input = &linear.created()[0];
    assert_eq!(input["title"], "Keep this", "{input}");
    assert_eq!(input["description"], "and this", "{input}");
    assert_eq!(input["priority"], 4, "{input}");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_refusal_is_said_in_the_form_with_what_was_written_and_create_again_sends_the_same_draft() {
    let (linear, _session, mut page) = open_lists(WIDE);
    linear.set_create(Create::Refused);
    open_new(&mut page);
    page.type_text("A title").expect("the title");
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |page| page.says("title must be shorter"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(
        page.says("Linear did not make it"),
        "the words are Linear's: {}",
        said(&page)
    );
    assert!(page.bounds("new-create").is_some(), "{}", said(&page));
    assert_eq!(page.field("new-title/field"), Some("A title"));
    // The person tries again; the draft is the same one, and this time Linear makes it.
    linear.set_create(Create::Fine);
    page.press("mod+enter").expect("mod+enter");
    made(&linear, &mut page, 2);
    cursor_on(&mut page, "ACM-900");
    let sent = linear.created();
    assert_eq!(sent[0]["id"], sent[1]["id"], "{sent:?}");
    // A key that may not create says so in words of its own.
    linear.set_create(Create::Forbidden);
    open_new(&mut page);
    page.type_text("Second").expect("the title");
    page.press("mod+enter").expect("mod+enter");
    page.wait_for(WITHIN, |page| page.says("this key cannot create issues"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_an_80_by_24_pane_the_form_takes_the_pane_and_keeps_create_inside_it() {
    let (_linear, _session, mut page) = open_lists(PANE);
    open_new(&mut page);
    page.type_text("A title").expect("the title");
    assert!(!page.says("ACM-398"), "the list gives way: {}", said(&page));
    for name in ["new-title/field", "new-close", "new-create"] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is laid out"));
        assert!(
            at.x >= 0.0 && at.x + at.w <= PANE.0 + 0.5 && at.y + at.h <= PANE.1 + 0.5,
            "{name}: {at:?}"
        );
    }
    // Esc puts it away and the list is back.
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn an_issue_made_while_all_issues_is_shown_leaves_the_list_as_it_is() {
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("2").expect("2");
    page.wait_for(WITHIN, |page| page.says("ACM-417"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    open_new(&mut page);
    page.type_text("For me, from all").expect("the title");
    page.press("mod+enter").expect("mod+enter");
    made(&linear, &mut page, 1);
    cursor_on(&mut page, "ACM-900");
    // (It is for the person, and the list that is shown is still All issues.)
    assert!(page.says("ACM-417"), "{}", said(&page));
}

// --- Changing an issue ---------------------------------------------------------------------------

/// Whether the control `name` is laid out and the person can press it now.
pub(crate) fn pressable(page: &Page, name: &str) -> bool {
    let id = pencere_view::Id::named(name);
    page.outline()
        .accessible
        .iter()
        .any(|node| node.id == Some(id) && !node.state.disabled)
}

/// Opens the issue under the cursor and waits until its controls can be pressed (the choices of its
/// team have come).
fn open_the_controls(page: &mut Page) {
    open_the_issue(page);
    page.wait_for(WITHIN, |page| pressable(page, "panel-value:state"))
        .unwrap_or_else(|why| panic!("the controls can be pressed: {why:?}: {}", said(page)));
}

/// Waits until Linear has been sent `count` changes.
fn changed(linear: &Arc<Canned>, page: &mut Page, count: usize) {
    page.wait_for(WITHIN, |_| linear.changed().len() >= count)
        .unwrap_or_else(|why| {
            panic!(
                "Linear is asked: {why:?}: {:?}: {}",
                linear.changed(),
                said(page)
            )
        });
}

/// Presses `key` until a menu opens (a command waits for the choices of the team, and for a change
/// that is on its way), and gives the menu's entries: the title and whether it can be picked.
fn menu_after(page: &mut Page, key: &str) -> Vec<(String, bool)> {
    for _ in 0..100 {
        if page.menu().is_none() {
            page.press(key).expect("the key");
        }
        if page.menu().is_some() {
            break;
        }
        page.advance(0.1).expect("a moment");
    }
    page.menu()
        .unwrap_or_else(|| panic!("{key} opens a menu: {}", said(page)))
        .entries
        .iter()
        .filter_map(|entry| match entry {
            pencere_view::MenuEntry::Item { title, enabled, .. } => Some((title.clone(), *enabled)),
            _ => None,
        })
        .collect()
}

fn titles(entries: &[(String, bool)]) -> Vec<&str> {
    entries.iter().map(|(title, _)| title.as_str()).collect()
}

/// Whether the page has the host's field `name` now (one it has taken away is not).
fn has_field(page: &Page, name: &str) -> bool {
    let id = pencere_view::Id::named(name);
    page.outline().fields.iter().any(|field| field.id == id)
}

/// Whether a group's head says `words` to a screen reader (`In Review, 3`).
fn group_says(page: &Page, words: &str) -> bool {
    page.outline()
        .accessible
        .iter()
        .any(|node| node.words == words)
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_status_control_opens_its_menu_the_pick_goes_to_linear_and_the_issue_changes_group() {
    let (linear, _session, mut page) = open_lists(WIDE);
    assert!(group_says(&page, "In Progress, 4"), "{}", said(&page));
    open_the_controls(&mut page);
    page.click("panel-value:state").expect("status");
    // Every state of the team, not only the open ones: the one it is in is marked.
    assert_eq!(
        menu_titles(&page),
        [
            "Backlog",
            "Todo",
            "\u{2713} In Progress",
            "In Review",
            "Done",
            "Canceled"
        ]
    );
    page.pick_titled("In Review").expect("In Review");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "stateId": "s-review" } })
    );
    // The control says the new value, the issue is in its new group, and the cursor is on it.
    page.wait_for(WITHIN, |page| {
        page.words_in("panel-value:state")
            .contains(&"In Review".to_owned())
            && group_says(page, "In Review, 3")
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(group_says(&page, "In Progress, 3"), "{}", said(&page));
    cursor_on(&mut page, "ACM-412");
    // (Linear was not asked for the lists again.)
    assert_eq!(linear.times("MyIssues"), 1);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_change_to_a_folded_group_opens_it_and_the_cursor_follows_the_issue() {
    let (linear, _session, mut page) = open_lists(WIDE);
    assert!(group_says(&page, "Backlog, 2"), "{}", said(&page));
    let entries = menu_after(&mut page, "s");
    assert_eq!(entries.len(), 6, "{entries:?}");
    page.pick_titled("Backlog").expect("Backlog");
    changed(&linear, &mut page, 1);
    // Backlog is folded in the list, and it is opened to show where the issue went.
    page.wait_for(WITHIN, |page| group_says(page, "Backlog, 3"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.wait_for(WITHIN, |page| page.bounds("row:i:issue-412").is_some())
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    cursor_on(&mut page, "ACM-412");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn done_takes_the_issue_out_of_the_list_the_panel_stays_on_it_and_follows_the_cursor_after_a_move()
{
    let (linear, _session, mut page) = open_lists(WIDE);
    // A line that is not the first, so that its neighbour is not where the cursor began.
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    open_the_controls(&mut page);
    page.click("panel-value:state").expect("status");
    page.pick_titled("Done").expect("Done");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-398"], "input": { "stateId": "s-done" } })
    );
    // The line is gone, the cursor is on the next, and the issue is still open and says Done.
    page.wait_for(WITHIN, |page| {
        page.bounds("row:i:issue-398").is_none()
            && page
                .words_in("panel-value:state")
                .contains(&"Done".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    cursor_on(&mut page, "ACM-405");
    assert!(page.says("Checkout test fails"), "{}", said(&page));
    assert!(group_says(&page, "In Progress, 3"), "{}", said(&page));
    page.advance(0.5).expect("a moment");
    assert!(
        page.says("Checkout test fails"),
        "the panel stays where the person left it: {}",
        said(&page)
    );
    // The change is undone from the panel that stayed: the issue is back in the list.
    page.click("panel-value:state").expect("status");
    page.pick_titled("In Progress").expect("In Progress");
    changed(&linear, &mut page, 2);
    assert_eq!(
        linear.changed()[1],
        serde_json::json!({ "ids": ["issue-398"], "input": { "stateId": "s-prog" } })
    );
    page.wait_for(WITHIN, |page| page.bounds("row:i:issue-398").is_some())
        .unwrap_or_else(|why| panic!("the line is back: {why:?}: {}", said(&page)));
    assert!(group_says(&page, "In Progress, 4"), "{}", said(&page));
    assert!(page.says("Checkout test fails"), "{}", said(&page));
    // Once the cursor is moved the issue open follows it again.
    page.press("down").expect("down");
    page.wait_for(WITHIN, |page| {
        page.words_in("panel-title-edit")
            .contains(&"Write the 3.0 migration guide".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn s_changes_the_issue_under_the_cursor_beside_its_line() {
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    let entries = menu_after(&mut page, "s");
    assert_eq!(
        titles(&entries),
        [
            "Backlog",
            "Todo",
            "\u{2713} In Progress",
            "In Review",
            "Done",
            "Canceled"
        ]
    );
    // The menu is by the line the cursor is on.
    let line = page.bounds("row:i:issue-398").expect("the line");
    let menu = page.menu().expect("the menu").at;
    assert!(
        (menu.y - line.y).abs() <= line.h * 2.0,
        "the menu {menu:?} is by the line {line:?}"
    );
    page.pick_titled("Todo").expect("Todo");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-398"], "input": { "stateId": "s-todo" } })
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn s_with_three_issues_selected_changes_all_three_and_the_menu_says_how_many() {
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("shift+down").expect("shift+down");
    page.press("shift+down").expect("shift+down");
    page.wait_for(WITHIN, |page| page.says("3 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    let entries = menu_after(&mut page, "s");
    // The first line is a title and cannot be picked.
    assert_eq!(entries[0], ("Status of 3 issues".to_owned(), false));
    assert_eq!(entries[1], ("Backlog".to_owned(), true));
    page.pick_titled("Done").expect("Done");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({
            "ids": ["issue-412", "issue-398", "issue-405"],
            "input": { "stateId": "s-done" }
        })
    );
    // They are gone from the list, and the cursor is on the next that is there.
    page.wait_for(WITHIN, |page| group_says(page, "In Progress, 1"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    cursor_on(&mut page, "ACM-377");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn issues_of_two_teams_say_why_status_cannot_change_and_priority_can() {
    let linear = Canned::new(Mode::Fine);
    linear.set_team_of("issue-398", "team-web", "WEB");
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, WIDE, None, "ACM-412").expect("the page opens");
    page.press("shift+down").expect("shift+down");
    page.wait_for(WITHIN, |page| page.says("2 selected"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    let entries = menu_after(&mut page, "s");
    assert_eq!(
        entries,
        [
            ("Status of 2 issues".to_owned(), false),
            ("The issues are of more than one team".to_owned(), false)
        ]
    );
    // (Nothing can be picked: Escape closes the menu.)
    page.press("escape").expect("escape");
    let entries = menu_after(&mut page, "p");
    assert_eq!(entries[0], ("Priority of 2 issues".to_owned(), false));
    assert_eq!(entries.len(), 6, "{entries:?}");
    page.pick_titled("Low").expect("Low");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412", "issue-398"], "input": { "priority": 4 } })
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn l_twice_adds_a_label_and_takes_it_off() {
    let (linear, _session, mut page) = open_lists(WIDE);
    let entries = menu_after(&mut page, "l");
    assert_eq!(titles(&entries), ["\u{2713} api", "bug", "Customer"]);
    page.pick_titled("bug").expect("bug");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "addedLabelIds": ["l-bug"] } })
    );
    page.wait_for(WITHIN, |page| {
        page.words_in("row:i:issue-412").contains(&"bug".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    let entries = menu_after(&mut page, "l");
    assert_eq!(
        titles(&entries),
        ["\u{2713} api", "\u{2713} bug", "Customer"]
    );
    page.pick_titled("\u{2713} bug").expect("bug off");
    changed(&linear, &mut page, 2);
    assert_eq!(
        linear.changed()[1],
        serde_json::json!({ "ids": ["issue-412"], "input": { "removedLabelIds": ["l-bug"] } })
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn assignee_and_project_are_set_and_unset_by_their_menus() {
    let (linear, _session, mut page) = open_lists(WIDE);
    let entries = menu_after(&mut page, "a");
    assert_eq!(titles(&entries), ["\u{2713} Me", "Deniz", "No one"]);
    page.pick_titled("No one").expect("No one");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "assigneeId": null } })
    );
    // The issue is no longer the person's: it leaves My issues, and the cursor goes on.
    page.wait_for(WITHIN, |page| group_says(page, "In Progress, 3"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    cursor_on(&mut page, "ACM-398");
    // The panel's own control sets a project.
    open_the_controls(&mut page);
    page.click("panel-value:project").expect("project");
    assert_eq!(menu_titles(&page), ["\u{2713} No project", "Billing"]);
    page.pick_titled("Billing").expect("Billing");
    changed(&linear, &mut page, 2);
    assert_eq!(
        linear.changed()[1],
        serde_json::json!({ "ids": ["issue-398"], "input": { "projectId": "p-1" } })
    );
    page.wait_for(WITHIN, |page| {
        page.words_in("panel-value:project")
            .contains(&"Billing".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn e_makes_the_title_a_field_enter_saves_escape_leaves_it_and_an_empty_title_is_not_sent() {
    let (linear, _session, mut page) = open_lists(WIDE);
    let title = "Add rate limiting to the public API";
    page.press("e").expect("e");
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| panic!("the title is a field: {why:?}: {}", said(&page)));
    // The words are selected, so that what is typed replaces them; and the keys are the field's.
    assert_eq!(
        page.field_selection("panel-title/field"),
        Some(0..title.len())
    );
    page.type_text("s p a l e").expect("letters");
    assert_eq!(page.field("panel-title/field"), Some("s p a l e"));
    assert!(page.menu().is_none(), "no command ran");
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-412");
    // Escape leaves the old title, and sends nothing.
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !has_field(page, "panel-title/field"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(page.says(title), "{}", said(&page));
    assert!(linear.changed().is_empty(), "{:?}", linear.changed());
    assert!(
        page.bounds("panel-close").is_some(),
        "Escape was the field's, not the panel's"
    );
    // An empty title is not sent either.
    page.press("e").expect("e");
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.press("backspace").expect("backspace");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| !has_field(page, "panel-title/field"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(linear.changed().is_empty(), "{:?}", linear.changed());
    // Enter saves what was written, once.
    page.press("e").expect("e");
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.type_text("  Limit the public API  ")
        .expect("the title");
    page.press("enter").expect("enter");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "title": "Limit the public API" } })
    );
    page.wait_for(WITHIN, |page| page.says("Limit the public API"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(!has_field(&page, "panel-title/field"));
    assert_eq!(linear.changed().len(), 1);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_refusal_is_said_in_the_panel_or_in_a_strip_and_the_value_stays() {
    let (linear, _session, mut page) = open_lists(WIDE);
    linear.set_change(Change::Refused);
    open_the_controls(&mut page);
    page.click("panel-value:state").expect("status");
    page.pick_titled("Done").expect("Done");
    changed(&linear, &mut page, 1);
    page.wait_for(WITHIN, |page| page.says("Linear did not change it"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // Nothing was changed: the line is where it was, and the control says what it said.
    assert!(page.bounds("row:i:issue-412").is_some(), "{}", said(&page));
    assert!(group_says(&page, "In Progress, 4"), "{}", said(&page));
    assert!(
        page.words_in("panel-value:state")
            .contains(&"In Progress".to_owned())
    );
    assert!(
        !page.says("Dismiss"),
        "the strip is for what the panel is not on"
    );
    // With the panel put away the words are in a strip over the list.
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("API key"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    let entries = menu_after(&mut page, "s");
    assert_eq!(entries.len(), 6);
    page.pick_titled("Done").expect("Done");
    changed(&linear, &mut page, 2);
    page.wait_for(WITHIN, |page| page.says("Linear did not change ACM-412"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(group_says(&page, "In Progress, 4"), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_letters_of_the_keys_are_letters_in_the_filter_and_in_the_forms() {
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("/").expect("slash");
    page.type_text("spale").expect("the filter");
    page.wait_for(WITHIN, |page| page.says("No issue matches"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    assert!(page.menu().is_none());
    page.press("escape").expect("escape");
    page.press("escape").expect("escape");
    open_new(&mut page);
    page.type_text("spale").expect("the title");
    assert_eq!(page.field("new-title/field"), Some("spale"));
    assert!(page.menu().is_none());
    assert!(linear.changed().is_empty());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_an_80_by_24_pane_the_controls_are_inside_the_pane() {
    let (_linear, _session, mut page) = open_lists(PANE);
    open_the_controls(&mut page);
    for name in [
        "panel-value:state",
        "panel-value:priority",
        "panel-value:assignee",
        "panel-value:labels",
        "panel-value:project",
        "panel-close",
    ] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is laid out"));
        assert!(
            at.x >= 0.0 && at.x + at.w <= PANE.0 + 0.5 && at.y + at.h <= PANE.1 + 0.5,
            "{name}: {at:?}"
        );
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_press_on_the_title_makes_it_a_field_and_a_press_elsewhere_leaves_it_as_it_was() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_the_controls(&mut page);
    let title = "Add rate limiting to the public API";
    page.click("panel-title-edit").expect("the title");
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| panic!("the title is a field: {why:?}: {}", said(&page)));
    page.type_text("zz").expect("the words");
    // Only Enter writes: a press on another line takes the keyboard away, and what was typed goes.
    page.click("row:i:issue-398").expect("another line");
    page.wait_for(WITHIN, |page| !has_field(page, "panel-title/field"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    cursor_on(&mut page, "ACM-398");
    page.advance(0.5).expect("a moment");
    assert!(linear.changed().is_empty(), "{:?}", linear.changed());
    assert!(page.says(title), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn tab_takes_the_keyboard_out_of_the_title_and_nothing_is_written() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_the_controls(&mut page);
    let title = "Add rate limiting to the public API";
    page.press("e").expect("e");
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| panic!("the title is a field: {why:?}: {}", said(&page)));
    page.type_text("zz").expect("the words");
    page.press("tab").expect("tab");
    page.wait_for(WITHIN, |page| !has_field(page, "panel-title/field"))
        .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    page.advance(0.5).expect("a moment");
    assert!(linear.changed().is_empty(), "{:?}", linear.changed());
    assert!(page.says(title), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn e_edits_the_one_issue_selected_though_the_cursor_is_on_another() {
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("x").expect("x");
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    page.press("e").expect("e");
    // The title that is the field's is the selected issue's, and the panel is on that issue.
    let title = "Add rate limiting to the public API";
    page.wait_for(WITHIN, |page| {
        page.field("panel-title/field") == Some(title)
    })
    .unwrap_or_else(|why| {
        panic!(
            "the selected issue's title is a field: {why:?}: {}",
            said(&page)
        )
    });
    page.advance(0.5).expect("a moment");
    assert_eq!(
        page.field("panel-title/field"),
        Some(title),
        "{}",
        said(&page)
    );
    page.type_text("Limit it").expect("the title");
    page.press("enter").expect("enter");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "title": "Limit it" } })
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_project_has_a_key_and_what_it_picks_is_written() {
    let (linear, _session, mut page) = open_lists(WIDE);
    let entries = menu_after(&mut page, "shift+p");
    assert_eq!(titles(&entries), ["\u{2713} No project", "Billing"]);
    page.pick_titled("Billing").expect("Billing");
    changed(&linear, &mut page, 1);
    assert_eq!(
        linear.changed()[0],
        serde_json::json!({ "ids": ["issue-412"], "input": { "projectId": "p-1" } })
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn after_a_value_is_picked_from_the_panel_enter_is_the_lists_and_not_the_controls() {
    let (linear, _session, mut page) = open_lists(WIDE);
    open_the_controls(&mut page);
    page.click("panel-value:priority").expect("priority");
    page.pick_titled("Low").expect("Low");
    changed(&linear, &mut page, 1);
    page.wait_for(WITHIN, |page| {
        page.words_in("panel-value:priority")
            .contains(&"Low".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
    // The control was pressed with the pointer: Enter does not open its menu again.
    page.press("enter").expect("enter");
    page.advance(0.5).expect("a moment");
    assert!(page.menu().is_none(), "{}", said(&page));
    assert_eq!(linear.changed().len(), 1);
    // The arrows and Enter are the list's: the cursor goes on and Enter keeps that issue open.
    page.press("down").expect("down");
    cursor_on(&mut page, "ACM-398");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| {
        page.words_in("panel-title-edit")
            .contains(&"Checkout test fails one run in ten".to_owned())
    })
    .unwrap_or_else(|why| panic!("{why:?}: {}", said(&page)));
}
