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

/// Linear, in memory: answers by the query's name with a fixture, and keeps what it was asked.
pub(crate) struct Canned {
    mode: Mutex<Mode>,
    asked: Mutex<Vec<String>>,
    key: Mutex<Option<String>>,
}

impl Canned {
    pub(crate) fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode: Mutex::new(mode),
            asked: Mutex::new(Vec::new()),
            key: Mutex::new(None),
        })
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

impl Transport for Canned {
    fn post(&self, key: &Key, body: &str) -> Result<Reply, LinearError> {
        let name = ["Verify", "MyIssues", "AllIssues", "Issue"]
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
            Mode::NothingAssigned if name == "MyIssues" => reply(200, MY_ISSUES_EMPTY),
            _ => match name {
                "Verify" => reply(200, VERIFY),
                "MyIssues" => reply(200, MY_ISSUES),
                "AllIssues" => reply(200, ALL_ISSUES),
                "Issue" => reply(200, ISSUE),
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
        page.says("A key with Read access is enough."),
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
    // A button the pointer pressed has the keyboard, and Enter presses it again.
    page.click("refresh").expect("refresh is pressed");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") == 2)
        .expect("Linear was asked again");
    page.wait_for(WITHIN, |_| !session.state().refreshing)
        .expect("the answer came");
    // The page is told, and the button is pressable again.
    page.advance(0.5).expect("a moment");
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |_| linear.times("MyIssues") == 3)
        .expect("Enter on the button asked Linear once more");
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
