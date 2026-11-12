//! The pictures of docs/, made from the page as it is: the page and the program's handlers in one
//! process (as `page_tests` runs them), over the fixtures of test/stand-in/, laid out and drawn in
//! the fonts of this machine (macOS: SF Mono, or Menlo, for the terminal's face, as bateri's default
//! is; SF for the system's) by pencere's host with no screen. Not a terminal's drawing, no shaping and
//! no shadows, but the same boxes, words and colours where the kit puts them. Nothing in them is from
//! the machine they are made on: the issues are made up, and Linear is not asked.
//!
//! They need the system's fonts, so they are not part of the gate: `make shots` makes them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_drive::shot::{Fonts, Palette};

use crate::page_tests::{
    Canned, Mode, PANE, Page, cursor, ls_of_p41, open, pressable, rig, rig_polling, session_over,
};

/// A page as large as a terminal pane on a laptop.
const BIG: (f32, f32) = (1280.0, 800.0);

/// The fonts a Mac draws a terminal and its system in: bateri's terminal face is SF Mono where the
/// system has it, else Menlo.
fn fonts() -> Fonts {
    let read = |path: &str| {
        std::fs::read(path)
            .unwrap_or_else(|why| panic!("{path}: {why} (make shots needs the system's fonts)"))
    };
    let terminal = std::fs::read("/System/Library/Fonts/SFNSMono.ttf")
        .unwrap_or_else(|_| read("/System/Library/Fonts/Menlo.ttc"));
    Fonts::none()
        .terminal(&terminal)
        .and_then(|fonts| fonts.system(&read("/System/Library/Fonts/SFNS.ttf")))
        .expect("the fonts")
}

fn docs(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join(name)
}

fn save(page: &Page, fonts: &Fonts, palette: Palette, name: &str) {
    let png = page.shot_in(fonts, palette).expect("a picture");
    std::fs::write(docs(name), png).expect("the picture written");
}

/// The page over the lists of the fixtures, at `size`.
fn lists(size: (f32, f32), fonts: &Fonts) -> Result<Page, Error> {
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, size, Some(fonts), "ACM-412")?;
    // The line of keys says `s` where the choices of the team have come, which they do a moment after
    // the list (a pane too narrow for it never says it: that wait is given up).
    let _ = page.wait_for(Duration::from_secs(3), |page| page.says("status"));
    page.advance(0.3)?;
    Ok(page)
}

/// The page over the lists of a workspace with one team (as the design shows it), at `size`, and
/// the Linear it asks.
fn one_team(size: (f32, f32), fonts: &Fonts) -> Result<(Arc<Canned>, Page), Error> {
    let linear = Canned::new(Mode::Fine);
    linear.set_one_team();
    let session = session_over(&linear, Some("lin_api_test"), None);
    let page = open(&session, size, Some(fonts), "ACM-412")?;
    Ok((linear, page))
}

/// Opens `issue` beside the list (or over it, in a pane too narrow for both), and then its Work on
/// form: Work on is the detail's, and one issue's. ACM-415 is in Todo, which has not begun, so its
/// form has the Status row.
fn work_on(page: &mut Page, issue: &str) -> Result<(), Error> {
    for _ in 0..30 {
        if cursor(page).contains(issue) {
            break;
        }
        page.press("down")?;
        page.advance(0.05)?;
    }
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        pressable(page, "panel-value:state") && pressable(page, "work")
    })?;
    page.advance(0.6)?;
    page.press("w")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Open in"))?;
    page.advance(0.3)
}

/// The new issue form, opened with `c`, once the choices are there.
fn new_form(page: &mut Page) -> Result<(), Error> {
    page.press("c")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.bounds("new-value:state").is_some()
    })?;
    page.advance(0.6)
}

#[test]
#[ignore = "shots: draws the page in the fonts of this machine into docs/; make shots"]
fn the_pictures_of_the_docs() -> Result<(), Error> {
    let fonts = fonts();

    // The list: mine, in the colours of a dark and of a light terminal, and in an 80 by 24 pane.
    let page = lists(BIG, &fonts)?;
    save(&page, &fonts, Palette::Dark, "list.png");
    save(&page, &fonts, Palette::Light, "light-list.png");
    let page = lists(PANE, &fonts)?;
    save(&page, &fonts, Palette::Dark, "narrow-list.png");

    // All issues, which say whose each is.
    let mut page = lists(BIG, &fonts)?;
    page.press("2")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("ACM-417"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "all.png");

    // The filter, on a word.
    let mut page = lists(BIG, &fonts)?;
    page.press("/")?;
    page.type_text("bug")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("4 of 14"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "filter.png");

    // An issue open beside the list: in the colours of a dark and of a light terminal, and in an
    // 80 by 24 pane, where it takes the whole of it.
    let mut page = lists(BIG, &fonts)?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "issue.png");
    save(&page, &fonts, Palette::Light, "light-issue.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-issue.png");

    // An issue changed. The controls of the issue open (the menu a press on one opens is the host's, which
    // a picture made without one does not draw), its title as a field (nor is what is typed into a
    // line), the issue done, left the list and still open, three issues selected for a change, and the
    // controls in an 80 by 24 pane.
    let mut page = lists(BIG, &fonts)?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute") && pressable(page, "panel-value:state")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "edit.png");
    page.press("r")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("save"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "edit-title.png");
    page.press("escape")?;
    page.wait_for(Duration::from_secs(20), |page| !page.says("save"))?;
    page.click("panel-value:state")?;
    page.pick_titled("Done")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.bounds("row:i:issue-412").is_none()
            && page
                .words_in("panel-value:state")
                .contains(&"Done".to_owned())
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "edit-done.png");
    let mut page = lists(BIG, &fonts)?;
    page.press("x")?;
    page.press("down")?;
    page.press("x")?;
    page.press("down")?;
    page.press("x")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("3 selected"))?;
    page.press("s")?;
    if let Err(why) = page.wait_for(Duration::from_secs(20), |page| page.menu().is_some()) {
        panic!("{why:?}: {:?}", page.words());
    }
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "edit-many.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute") && pressable(page, "panel-value:state")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "narrow-edit.png");

    // A new issue: the empty form, the form written (the host draws what is typed into a line, so the
    // title is not in a picture made without one), the issue made and open beside the list, and the
    // form in an 80 by 24 pane, where it takes the pane.
    let (_linear, mut page) = one_team(BIG, &fonts)?;
    new_form(&mut page)?;
    save(&page, &fonts, Palette::Dark, "new.png");
    save(&page, &fonts, Palette::Light, "light-new.png");
    let (linear, mut page) = one_team(BIG, &fonts)?;
    new_form(&mut page)?;
    page.type_text("Return 429 when a key is over its limit")?;
    page.press("enter")?;
    page.type_text(
        "The limiter drops the request without a word. Send 429 with Retry-After, and count it in the usage numbers.",
    )?;
    page.click("new-value:priority")?;
    page.pick_titled("High")?;
    page.click("new-value:labels")?;
    page.pick_titled("bug")?;
    page.click("new-value:labels")?;
    page.pick_titled("api")?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "new-typed.png");
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.bounds("panel-close").is_some() && page.says("ACM-900")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "created.png");
    assert_eq!(linear.created().len(), 1);
    let (_linear, mut page) = one_team(PANE, &fonts)?;
    new_form(&mut page)?;
    page.type_text("Return 429 when a key is over its limit")?;
    page.press("enter")?;
    page.type_text(
        "The limiter drops the request without a word. Send 429 with Retry-After, and count it in the usage numbers.",
    )?;
    page.click("new-value:priority")?;
    page.pick_titled("High")?;
    page.click("new-value:labels")?;
    page.pick_titled("bug")?;
    page.click("new-value:labels")?;
    page.pick_titled("api")?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "narrow-new.png");

    // Issues selected: the line under the page is the bar of the selection, which changes what they
    // have in common (an agent is started from the detail of one issue, not from the selection). The
    // menu of a right press is the host's, which a picture made without one does not draw.
    let mut page = lists(BIG, &fonts)?;
    page.press("x")?;
    page.press("shift+down")?;
    page.press("shift+down")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("3 selected"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "select.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("x")?;
    page.press("shift+down")?;
    page.press("shift+down")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("3 selected"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-select.png");

    // The lists, from the button in the head.
    let mut page = lists(BIG, &fonts)?;
    page.click("lists")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("The workspace"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "lists.png");

    // An issue handed to an agent: the form (on a machine that has bateri and three agents), and the
    // list after the start, where the issue says who works on it.
    let linear = Canned::new(Mode::Fine);
    let machine = rig(&linear, true, &["claude", "codex", "cursor-agent"]);
    let mut page = open(&machine.session, BIG, Some(&fonts), "ACM-412")?;
    work_on(&mut page, "ACM-415")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Unchanged"))?;
    page.click("work-folder/item/worktree")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("The agent makes it")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "work.png");
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        !page.says("Open in") && page.says("Claude")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "started.png");
    machine.hands.end();

    // The same issue, handed over with its status left as it is, an agent in a pane that has been
    // quiet for a minute: the row says it has stopped.
    let linear = Canned::new(Mode::Fine);
    let machine = rig_polling(
        &linear,
        true,
        &["claude", "codex", "cursor-agent"],
        Duration::from_millis(20),
    );
    machine.hands.set_ls(&ls_of_p41(0.0));
    let mut page = open(&machine.session, BIG, Some(&fonts), "ACM-412")?;
    work_on(&mut page, "ACM-415")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Unchanged"))?;
    page.click("work-status/item/keep")?;
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        !page.says("Open in") && page.says("Claude")
    })?;
    machine.hands.set_ls(&ls_of_p41(60.0));
    page.wait_for(Duration::from_secs(20), |page| page.says("Claude idle"))?;
    for _ in 0..30 {
        if cursor(&page).contains("ACM-398") {
            break;
        }
        page.press("up")?;
        page.advance(0.05)?;
    }
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "started-idle.png");
    machine.hands.end();

    // (Linear is made again: the issue that was moved is in progress now, and has no Status to say.)
    let linear = Canned::new(Mode::Fine);
    let machine = rig(&linear, true, &["claude", "codex", "cursor-agent"]);
    let mut page = open(&machine.session, PANE, Some(&fonts), "ACM-412")?;
    work_on(&mut page, "ACM-415")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Unchanged"))?;
    page.click("work-folder/item/worktree")?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-work.png");
    machine.hands.end();

    // The card of keys (in a taller window than the others: it has seven groups and the keyboard
    // scrolls it where the window is shorter), and the palette.
    let mut page = lists((BIG.0, BIG.1 + 100.0), &fonts)?;
    page.press("?")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Keys"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "keys.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("?")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Keys"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-keys.png");
    let mut page = lists(BIG, &fonts)?;
    page.press("mod+k")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Do it"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "palette.png");

    // Nothing assigned to the person.
    let linear = Canned::new(Mode::NothingAssigned);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let page = open(&session, BIG, Some(&fonts), "Nothing is assigned to you")?;
    save(&page, &fonts, Palette::Dark, "empty.png");

    // No key yet: Connect.
    let dir = std::env::temp_dir().join(format!("linearpen-shots-{}", std::process::id()));
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, None, Some(dir.clone()));
    let page = open(&session, BIG, Some(&fonts), "Connect Linear")?;
    save(&page, &fonts, Palette::Dark, "connect.png");
    let _ = std::fs::remove_dir_all(&dir);

    // The first asking: lines that stand where the issues will.
    let linear = Canned::new(Mode::Held);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let page = open(&session, BIG, Some(&fonts), "Loading")?;
    save(&page, &fonts, Palette::Dark, "loading.png");
    linear.set(Mode::Fine);

    // Linear is not reached.
    let linear = Canned::new(Mode::Down);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let page = open(&session, BIG, Some(&fonts), "Linear did not answer")?;
    save(&page, &fonts, Palette::Dark, "error.png");
    Ok(())
}
