//! What an agent reads of the page, whole, in the states that matter, against goldens, and the reading audit
//! of each: how the page is read is held to what is written down.

use std::path::{Path, PathBuf};

use super::page_tests::{
    Canned, Mode, PANE, Page, WIDE, WITHIN, comment_editor, config_dir, open, open_lists, open_new,
    open_the_issue, pressable, rig, session_over, typing_in,
};

/// Sets what the page reads now against the golden `name` (`src/program/reading/<name>.txt`), and holds the
/// page to the reading audit: a control with no name, an item said twice, a modal's page read behind it.
/// `PENCERE_BLESS=1` writes the goldens (read the diff: it is how an agent reads the page); `READING_OUT`
/// names a folder each reading is copied to, for comparing two pencere.
fn check(name: &str, page: &mut Page) {
    let reading = page.reading().text();
    if let Some(dir) = std::env::var_os("READING_OUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("a folder for the readings");
        std::fs::write(dir.join(format!("{name}.txt")), &reading).expect("a reading");
    }
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/program/reading")
        .join(format!("{name}.txt"));
    if std::env::var_os("PENCERE_BLESS").is_some() {
        std::fs::create_dir_all(file.parent().expect("a folder"))
            .expect("a folder for the goldens");
        std::fs::write(&file, &reading).expect("a golden");
    } else {
        let golden = std::fs::read_to_string(&file)
            .unwrap_or_else(|_| panic!("{name}: no golden at {}: PENCERE_BLESS=1", file.display()));
        assert_eq!(
            golden, reading,
            "{name}: the page is read otherwise than the golden says: PENCERE_BLESS=1 writes it"
        );
    }
    let flaws: String = pencere_drive::audit::report(&page.audit())
        .lines()
        .filter(|line| !waived(line, &reading))
        .map(|line| format!("{line}\n"))
        .collect();
    assert!(
        flaws.is_empty(),
        "{name}: the reading audit finds:\n{flaws}"
    );
}

/// Whether the audit's `finding` (`line N: - no id: button "New issue"`) is one that is not this page's to mend:
/// a button that cannot be pressed now (the kit's `disabled`) answers no press, so it has no id for an agent to
/// name; it is read, as disabled, with what it does in its description. Only a line of the reading that says
/// `[disabled` is let off, and only for these two buttons: one that can be pressed and has no id is a flaw.
fn waived(finding: &str, reading: &str) -> bool {
    let Some(number) = finding
        .strip_prefix("line ")
        .and_then(|rest| rest.split(':').next())
        .and_then(|number| number.parse::<usize>().ok())
    else {
        return false;
    };
    let said = reading.lines().nth(number.saturating_sub(1)).unwrap_or("");
    [
        "no id: button \"New issue\"",
        "no id: button \"Create issue\"",
    ]
    .iter()
    .any(|flaw| finding.contains(flaw))
        && said.contains("[disabled")
}

/// Waits until the page says `words`.
fn says(page: &mut Page, words: &str) {
    page.wait_for(WITHIN, |page| page.says(words))
        .unwrap_or_else(|why| panic!("the page says {words:?}: {why:?}"));
    page.advance(0.3).expect("a moment");
}

#[test]
#[ignore = "page: compiles the kit's interpreter and drives the page; make test-page"]
fn the_page_is_read_in_its_states() {
    // The list of the person's issues, the cursor on the first.
    let (_linear, _session, mut page) = open_lists(WIDE);
    check("main", &mut page);

    // Two lines selected: the bar of the selection takes the line of keys.
    page.press("x").expect("x");
    page.press("shift+down").expect("shift and down");
    says(&mut page, "2 selected");
    check("selected", &mut page);

    // The filter typed, with what it lets through.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("/").expect("slash");
    page.type_text("bug").expect("the filter is typed");
    says(&mut page, "4 of 14");
    check("filter", &mut page);

    // The inbox: the issues of the notifications, each line saying who, what, and whether it is unread.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("g").expect("g");
    says(&mut page, "Assigned to you");
    page.advance(0.3).expect("a moment");
    page.type_text("inbox").expect("the words");
    page.press("enter").expect("enter");
    says(&mut page, "Unread");
    page.advance(0.3).expect("a moment");
    check("inbox", &mut page);

    // The menu of a line, opened by a right press.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.right_click("row:i:issue-398").expect("a right press");
    page.wait_for(WITHIN, |page| page.menu().is_some())
        .expect("the menu");
    page.advance(0.3).expect("a moment");
    check("menu", &mut page);

    // The issue beside the list.
    let (_linear, _session, mut page) = open_lists(WIDE);
    open_the_issue(&mut page);
    page.wait_for(WITHIN, |page| pressable(page, "panel-value:state"))
        .expect("the controls can be pressed");
    page.advance(0.3).expect("a moment");
    check("issue", &mut page);

    // The words of a comment, written in the issue.
    page.press("shift+m").expect("shift and m");
    page.wait_for(WITHIN, |page| typing_in(page, &comment_editor("issue-412")))
        .expect("the keyboard is in the field");
    page.type_text("Why so? Looks right").expect("the words");
    page.advance(0.3).expect("a moment");
    check("comment", &mut page);

    // The labels of the issue, in a card over the page.
    let (_linear, _session, mut page) = open_lists(WIDE);
    for _ in 0..100 {
        page.press("l").expect("l");
        page.advance(0.1).expect("a moment");
        if page.bounds("values-picker").is_some() {
            break;
        }
    }
    assert!(page.bounds("values-picker").is_some(), "the card of labels");
    page.advance(0.3).expect("a moment");
    check("labels_card", &mut page);

    // The command palette over the list.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("mod+k").expect("mod+k");
    says(&mut page, "Do it");
    check("palette", &mut page);

    // The card of keys.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("?").expect("?");
    says(&mut page, "Keys");
    check("keys", &mut page);

    // The card of lists, with the names of the lists.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("g").expect("g");
    page.wait_for(WITHIN, |page| {
        page.says("Assigned to you") && page.says("Bug bash, November")
    })
    .expect("the card of lists with the names");
    page.advance(0.3).expect("a moment");
    check("lists_card", &mut page);

    // The row of filters, its first chip chosen, and the card of its values.
    let (linear, _session, mut page) = open_lists(WIDE);
    page.press("f").expect("f");
    page.wait_for(WITHIN, |page| pressable(page, "filters:status"))
        .expect("the row opens");
    page.wait_for(WITHIN, |_| linear.times("FilterOptions") >= 1)
        .expect("the choices of the filters are asked for");
    page.advance(0.4).expect("a moment");
    check("filters", &mut page);
    page.press("enter").expect("enter");
    page.wait_for(WITHIN, |page| page.bounds("values-picker").is_some())
        .expect("the card of the status");
    page.advance(0.3).expect("a moment");
    check("filters_card", &mut page);

    // The form of a new issue, and the card of its labels over it.
    let (_linear, _session, mut page) = open_lists(WIDE);
    open_new(&mut page);
    page.advance(0.3).expect("a moment");
    check("new", &mut page);
    page.click("new-value:labels").expect("labels");
    page.wait_for(WITHIN, |page| page.bounds("values-picker").is_some())
        .expect("the card of labels");
    page.advance(0.3).expect("a moment");
    check("new_labels_card", &mut page);

    // The form of work on the issue, over a machine with bateri and two agents.
    let linear = Canned::new(Mode::Fine);
    let machine = rig(&linear, true, &["claude", "codex"]);
    let mut page = open(&machine.session, WIDE, None, "ACM-412").expect("the page opens");
    open_the_issue(&mut page);
    page.press("w").expect("w");
    says(&mut page, "Open in");
    check("work", &mut page);

    // A note that says what was done: the id was copied.
    let (_linear, _session, mut page) = open_lists(WIDE);
    page.press("mod+shift+i").expect("the key");
    says(&mut page, "Copied ACM-412");
    check("toast", &mut page);

    // An 80x24 pane: the issue takes the pane.
    let (_linear, _session, mut page) = open_lists(PANE);
    check("pane", &mut page);
    open_the_issue(&mut page);
    page.advance(0.3).expect("a moment");
    check("pane_issue", &mut page);

    // Without a key: the screen that asks for one, and the one that says it was refused.
    let dir = config_dir("reading-connect");
    let linear = Canned::new(Mode::Refused);
    let session = session_over(&linear, None, Some(dir.clone()));
    let mut page = open(&session, WIDE, None, "Connect Linear").expect("the page opens");
    page.advance(0.5).expect("a moment");
    check("connect", &mut page);
    page.type_text("lin_api_wrong").expect("the key is typed");
    page.click("connect").expect("connect is pressed");
    says(&mut page, "did not accept");
    check("connect_refused", &mut page);
    let _ = std::fs::remove_dir_all(&dir);

    // Linear is not there.
    let linear = Canned::new(Mode::Down);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, WIDE, None, "Linear did not answer").expect("the page opens");
    page.advance(0.5).expect("a moment");
    check("down", &mut page);
}
