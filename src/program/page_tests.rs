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
        let name = ["Verify", "MyIssues", "AllIssues"]
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
            Mode::Down | Mode::Held => Err(LinearError::Unreachable("it is not there".to_owned())),
            Mode::Refused => reply(401, REFUSED),
            Mode::NothingAssigned if name == "MyIssues" => reply(200, MY_ISSUES_EMPTY),
            _ => match name {
                "Verify" => reply(200, VERIFY),
                "MyIssues" => reply(200, MY_ISSUES),
                "AllIssues" => reply(200, ALL_ISSUES),
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
    // Down in the field is the list's.
    page.type_text("a").expect("a letter");
    page.press("down").expect("down");
    page.press("enter").expect("enter leaves the field");
    page.press("escape").expect("escape clears what is typed");
    page.wait_for(WITHIN, |page| page.says("ACM-412"))
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
