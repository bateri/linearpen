//! The pictures of docs/, made from the page as it is: the page and the program's handlers in one
//! process (as `page_tests` runs them), over the fixtures of test/stand-in/, laid out and drawn in
//! the fonts of this machine (macOS: SF Mono for the terminal's face, as bateri's default is; SF for
//! the system's) by pencere's host with no screen. Not a terminal's drawing, no shaping and
//! no shadows, but the same boxes, words and colours where the kit puts them. Nothing in them is from
//! the machine they are made on: the issues are made up, and Linear is not asked.
//!
//! They draw in the fonts of the machine they are made on, so they are not part of the gate: `make
//! shots` makes them.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_drive::shot::{Fonts, Palette};

use crate::page_pictures::{
    REPLY, SHOT, given, on_the_page, open_filed, open_pictured, png_header, read_to,
    read_to_the_end,
};
use crate::page_tests::{
    Canned, Gh, HURRIED, Mode, PANE, Page, close_picker, cursor, ls_of_p41, open, open_patiently,
    pressable, rig, rig_polling, session_over, session_with_gh, turn, with_my_comment,
};

/// A page as large as a terminal pane on a laptop.
const BIG: (f32, f32) = (1280.0, 800.0);

/// The fonts of this machine: the pictures' own layer (`Fonts::installed`), which draws as the live
/// terminal does: SF Mono and SF on a Mac (as bateri's), with the bold and italic of their variable
/// faces, and for a character they lack the first face of the machine that has it. Where the
/// machine has none, the faces the program carries.
fn fonts() -> Fonts {
    Fonts::installed()
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

/// The cycle of the first team, from the card of lists.
fn go_to_cycle(page: &mut Page) -> Result<(), Error> {
    page.press("g")?;
    // (The card's first lines, which an 80 by 24 pane has room for too.)
    page.wait_for(Duration::from_secs(20), |page| page.says("Assigned to you"))?;
    page.advance(0.3)?;
    page.type_text("Current cycle")?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("ACM-417") && !page.says("ACM-405")
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

    // The inbox: the issues of the notifications, unread first, each with who and what; beside an issue
    // open; in an 80 by 24 pane; in the colours of a light terminal.
    let inbox = |size: (f32, f32), fonts: &Fonts| -> Result<Page, Error> {
        let mut page = lists(size, fonts)?;
        page.press("g")?;
        page.wait_for(Duration::from_secs(20), |page| {
            page.says("Your notifications")
        })?;
        page.advance(0.3)?;
        page.type_text("inbox")?;
        page.press("enter")?;
        page.wait_for(Duration::from_secs(20), |page| page.says("Unread"))?;
        page.advance(0.4)?;
        Ok(page)
    };
    let mut page = inbox(BIG, &fonts)?;
    save(&page, &fonts, Palette::Dark, "inbox.png");
    save(&page, &fonts, Palette::Light, "light-inbox.png");
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "inbox-issue.png");
    let page = inbox(PANE, &fonts)?;
    save(&page, &fonts, Palette::Dark, "narrow-inbox.png");

    // A pull request of the issue open, read with `gh`: open with a failed check and changes asked; merged;
    // while GitHub is asked; where `gh` is not signed in; in the colours of a light terminal; in an 80
    // by 24 pane.
    let pulled =
        |size: (f32, f32), fonts: &Fonts, mode: &'static str, held: bool| -> Result<Page, Error> {
            let linear = Canned::new(Mode::Fine);
            let gh = Gh::new(mode);
            let session = session_with_gh(&linear, &gh);
            let mut page = open_patiently(&session, size, Some(fonts), "ACM-412", Some(HURRIED))?;
            page.press("enter")?;
            page.wait_for(Duration::from_secs(20), |page| {
                page.bounds("pull:1").is_some()
            })?;
            page.advance(0.4)?;
            *gh.held.lock().unwrap() = held;
            let _ = page.click("pull:1");
            let words = match (mode, held) {
                (_, true) => "Asking GitHub",
                ("signed-out", _) => "gh auth login",
                ("merged", _) => "Approved",
                _ => "Changes requested",
            };
            page.wait_for(Duration::from_secs(20), |page| page.says(words))?;
            page.advance(0.4)?;
            if held {
                *gh.held.lock().unwrap() = false;
            }
            Ok(page)
        };
    let page = pulled(BIG, &fonts, "here", false)?;
    save(&page, &fonts, Palette::Dark, "pull.png");
    save(&page, &fonts, Palette::Light, "light-pull.png");
    let page = pulled(BIG, &fonts, "merged", false)?;
    save(&page, &fonts, Palette::Dark, "pull-merged.png");
    save(&page, &fonts, Palette::Light, "light-pull-merged.png");
    let page = pulled(BIG, &fonts, "here", true)?;
    save(&page, &fonts, Palette::Dark, "pull-loading.png");
    let page = pulled(BIG, &fonts, "signed-out", false)?;
    save(&page, &fonts, Palette::Dark, "pull-signed-out.png");
    let page = pulled(PANE, &fonts, "here", false)?;
    save(&page, &fonts, Palette::Dark, "narrow-pull.png");

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

    // A comment being written: the field has the keyboard (its ring), Comment can be pressed, and the
    // line under the page says how the field is left.
    let mut page = lists(BIG, &fonts)?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("600 requests a minute") && pressable(page, "panel-value:state")
    })?;
    page.press("shift+m")?;
    page.advance(0.4)?;
    page.type_text("Agreed. The fallback can answer from the last good count for a minute.")?;
    page.wait_for(Duration::from_secs(20), |page| {
        pressable(page, "comment-send")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "comment.png");

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
    // (The labels are checked in a card of the page's, which stays open until Esc.)
    page.click("new-value:labels")?;
    turn(&mut page, "bug");
    turn(&mut page, "api");
    close_picker(&mut page);
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
    // (The labels are checked in a card of the page's, which stays open until Esc.)
    page.click("new-value:labels")?;
    turn(&mut page, "bug");
    turn(&mut page, "api");
    close_picker(&mut page);
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
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("The workspace") && page.says("Bug bash, November")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "lists.png");
    // What is typed finds a list by its name, and the first favorite is where the key 3 goes.
    page.type_text("bug")?;
    page.wait_for(Duration::from_secs(20), |page| {
        !page.says("Assigned to you")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "lists-typed.png");
    page.press("escape")?;
    page.advance(0.6)?;
    page.press("3")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("ACM-398") && !page.says("ACM-412")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "favorite.png");

    // The cycle of a team, from the card, in a wide and in an 80 by 24 pane; the card in the pane too.
    let mut page = lists(BIG, &fonts)?;
    go_to_cycle(&mut page)?;
    save(&page, &fonts, Palette::Dark, "cycle.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("g")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("The workspace"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "narrow-lists.png");
    page.press("escape")?;
    page.advance(0.4)?;
    go_to_cycle(&mut page)?;
    save(&page, &fonts, Palette::Dark, "narrow-cycle.png");

    // The filters: the row `f` opens, the keyboard on its first chip; and what is left of it with the
    // keyboard back on the list, All issues narrowed to what Deniz has with the bug label. The values of
    // a chip are checked in a card of the page's, put away before each picture.
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, BIG, Some(&fonts), "ACM-412")?;
    page.press("2")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("ACM-417"))?;
    page.advance(0.4)?;
    page.press("f")?;
    page.wait_for(Duration::from_secs(20), |page| {
        pressable(page, "filters:status")
    })?;
    page.wait_for(Duration::from_secs(20), |_| {
        linear.times("FilterOptions") >= 1
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "filter-bar.png");
    page.click("filters:assignee")?;
    turn(&mut page, "Deniz");
    close_picker(&mut page);
    page.wait_for(Duration::from_secs(20), |page| page.says("ACM-431"))?;
    page.click("filters:labels")?;
    turn(&mut page, "bug");
    close_picker(&mut page);
    page.wait_for(Duration::from_secs(20), |_| {
        linear.variables_of("AllIssues").len() >= 3
    })?;
    page.advance(0.4)?;
    page.press("escape")?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "filtered.png");
    // The same in an 80 by 24 pane, where the chips are in a row that wraps.
    let linear = Canned::new(Mode::Fine);
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, PANE, Some(&fonts), "ACM-412")?;
    page.press("2")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("ACM-417"))?;
    page.press("f")?;
    page.wait_for(Duration::from_secs(20), |page| {
        pressable(page, "filters:status")
    })?;
    page.wait_for(Duration::from_secs(20), |_| {
        linear.times("FilterOptions") >= 1
    })?;
    page.advance(0.4)?;
    page.click("filters:assignee")?;
    turn(&mut page, "Deniz");
    close_picker(&mut page);
    page.wait_for(Duration::from_secs(20), |page| page.says("ACM-431"))?;
    page.click("filters:labels")?;
    turn(&mut page, "bug");
    close_picker(&mut page);
    page.wait_for(Duration::from_secs(20), |_| {
        linear.variables_of("AllIssues").len() >= 3
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-filter-bar.png");
    page.press("escape")?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "narrow-filtered.png");

    // The search: while the words are typed the line of keys says what Command Enter does; the words
    // that nothing held matches offer it as a button; and the result is a list of its own.
    let mut page = lists(BIG, &fonts)?;
    page.press("/")?;
    page.type_text("bug")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("4 of 14"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "search-hint.png");
    page.press("escape")?;
    page.press("escape")?;
    page.press("/")?;
    page.type_text("outage")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Nothing in My issues matches")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "search-none.png");
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("WEB-190"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "search.png");
    let mut page = lists(PANE, &fonts)?;
    page.press("/")?;
    page.type_text("outage")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Nothing in My issues matches")
    })?;
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("WEB-190"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "narrow-search.png");

    // All issues in a workspace that has more than the program takes.
    let linear = Canned::new(Mode::Fine);
    linear.set_big_all();
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, BIG, Some(&fonts), "ACM-412")?;
    page.press("2")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.words_in("lists").iter().any(|word| word == "500+")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "all-big.png");

    // A list that Linear has more of than the program takes: the button says 500+, and a line at the
    // end of the list says which ones it holds.
    let linear = Canned::new(Mode::Fine);
    linear.set_big_team();
    let session = session_over(&linear, Some("lin_api_test"), None);
    let mut page = open(&session, BIG, Some(&fonts), "ACM-412")?;
    page.press("g")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Bug bash, November")
    })?;
    page.type_text("OPS")?;
    page.press("enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.words_in("lists").iter().any(|word| word == "500+")
    })?;
    page.press("end")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("changed last"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "cut.png");

    // An issue handed to an agent: the form (on a machine that has bateri and three agents), and the
    // list after the start, where the issue says who works on it.
    let linear = Canned::new(Mode::Fine);
    let machine = rig(&linear, true, &["claude", "codex", "cursor-agent"]);
    let mut page = open(&machine.session, BIG, Some(&fonts), "ACM-412")?;
    work_on(&mut page, "ACM-415")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Unchanged"))?;
    page.click("work-folder/item/worktree")?;
    // Words of the person's for the agent, which end its prompt.
    page.click("work-prompt1")?;
    page.insert_text("Release it when the gate is green.")?;
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
    // (The pane is named for the issue it is started on, as bateri's list says it.)
    let named = |ago: f64| ls_of_p41(ago).replace("ACM-412", "ACM-415");
    machine.hands.set_ls(&named(0.0));
    let mut page = open(&machine.session, BIG, Some(&fonts), "ACM-412")?;
    work_on(&mut page, "ACM-415")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Unchanged"))?;
    page.click("work-status/item/keep")?;
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        !page.says("Open in") && page.says("Claude")
    })?;
    machine.hands.set_ls(&named(60.0));
    page.wait_for(Duration::from_secs(20), |page| {
        page.bounds("agent-idle:issue-415").is_some()
    })?;
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

/// An issue with pictures open at `size`, the first of them drawn.
fn pictured(size: (f32, f32), fonts: &Fonts) -> Result<crate::page_pictures::Pictured, Error> {
    let mut rig = open_pictured(size, Some(fonts))?;
    rig.page.press("enter")?;
    rig.page.wait_for(Duration::from_secs(20), |page| {
        page.bounds("pic:1").is_some() && page.bounds("pic:3").is_some()
    })?;
    rig.page.advance(0.6)?;
    Ok(rig)
}

#[test]
#[ignore = "shots: draws the page in the fonts of this machine into docs/; make shots"]
fn the_pictures_of_the_docs_with_images() -> Result<(), Error> {
    use pencere_view::FileSource;
    let fonts = fonts();
    let wait = Duration::from_secs(20);

    // The pictures of an issue, in place: in the colours of a dark and of a light terminal, and in an
    // 80 by 24 pane.
    let mut rig = pictured(BIG, &fonts)?;
    save(&rig.page, &fonts, Palette::Dark, "images.png");
    save(&rig.page, &fonts, Palette::Light, "light-images.png");
    // One of them large, over the pane.
    rig.page.click("pic:1")?;
    rig.page
        .wait_for(wait, |page| page.bounds("picture-close").is_some())?;
    rig.page.advance(0.6)?;
    save(&rig.page, &fonts, Palette::Dark, "image-view.png");
    let mut rig = pictured(PANE, &fonts)?;
    // (Read on, as a person does, to where the picture is.)
    rig.page.wheel("panel-scroll", 0.0, 60.0)?;
    rig.page.advance(0.6)?;
    save(&rig.page, &fonts, Palette::Dark, "narrow-images.png");

    // A picture Linear gives that is not one: why, and where it is.
    let mut rig = open_pictured(BIG, Some(&fonts))?;
    rig.linear
        .store(SHOT, b"GIF89a, as far as its first bytes go".to_vec());
    rig.linear.store(REPLY, png_header(5120, 2880));
    rig.page.press("enter")?;
    rig.page
        .wait_for(wait, |page| page.bounds("file-card:1").is_some())?;
    rig.page.advance(0.6)?;
    save(&rig.page, &fonts, Palette::Dark, "image-refused.png");

    // Two pictures pasted into a comment, and words after them.
    let mut rig = pictured(BIG, &fonts)?;
    let (one, two) = (given(200), given(90));
    rig.page.press("shift+m")?;
    rig.page.advance(0.6)?;
    read_to_the_end(&mut rig.page);
    rig.page.give_files(
        "comment-box",
        FileSource::Paste,
        &[("checkout-error.png", &one), ("retry-log.png", &two)],
    )?;
    rig.page
        .type_text("Both from the failing run last night.")?;
    rig.page.advance(0.8)?;
    save(&rig.page, &fonts, Palette::Dark, "comment-image.png");

    // The same on an issue that is being made.
    let mut rig = open_pictured(BIG, Some(&fonts))?;
    rig.page.press("c")?;
    rig.page
        .wait_for(wait, |page| page.bounds("new-value:state").is_some())?;
    rig.page.type_text("Retry the webhook with a backoff")?;
    rig.page.click("attach-file:new")?;
    rig.page
        .choose_files(&[("sketch.png", &one), ("flow.png", &two)])?;
    rig.page.advance(0.8)?;
    save(&rig.page, &fonts, Palette::Dark, "new-image.png");

    // Pictures chosen for the description of an issue that is open, before they are added.
    let mut rig = pictured(BIG, &fonts)?;
    read_to(&mut rig.page, "attach-file:describe");
    rig.page.click("attach-file:describe")?;
    rig.page.choose_files(&[("sketch.png", &one)])?;
    rig.page.advance(0.8)?;
    save(&rig.page, &fonts, Palette::Dark, "describe-image.png");

    // The files of an issue that are not pictures: cards, and the PDF read page by page over the pane.
    let mut rig = open_filed(BIG, Some(&fonts))?;
    rig.page.press("enter")?;
    rig.page.wait_for(wait, |page| {
        page.bounds("file-card:1").is_some() && page.bounds("pic:3").is_some()
    })?;
    rig.page.advance(0.6)?;
    save(&rig.page, &fonts, Palette::Dark, "file-card.png");
    rig.page.click("file-open:1")?;
    rig.page
        .wait_for(wait, |page| page.bounds("picture-close").is_some())?;
    rig.page.click("pdf-next")?;
    rig.page
        .wait_for(wait, |page| page.bounds("pdf-find-open").is_some())?;
    rig.page.advance(0.8)?;
    // Some of the words of the page selected, as a person does with the pointer.
    let from = on_the_page(&rig.page, 41.0, 236.0);
    let to = on_the_page(&rig.page, 120.0, 236.0);
    rig.page.drag_between(from, to)?;
    rig.page.advance(0.4)?;
    save(&rig.page, &fonts, Palette::Dark, "pdf-view.png");

    // The same PDF searched: the matches marked on the page, the one it is at the more.
    let mut rig = open_filed(BIG, Some(&fonts))?;
    rig.page.press("enter")?;
    rig.page
        .wait_for(wait, |page| page.bounds("file-open:1").is_some())?;
    rig.page.advance(0.6)?;
    rig.page.click("file-open:1")?;
    rig.page
        .wait_for(wait, |page| page.bounds("pdf-find-open").is_some())?;
    rig.page.press("mod+f")?;
    rig.page.type_text("page")?;
    rig.page.wait_for(wait, |page| page.says("1 of 6"))?;
    rig.page.press("enter")?;
    rig.page.wait_for(wait, |page| page.says("2 of 6"))?;
    rig.page.advance(0.8)?;
    save(&rig.page, &fonts, Palette::Dark, "pdf-find.png");

    // A zip, a PDF and a picture pasted into a comment: each with its kind and its size.
    let mut rig = pictured(BIG, &fonts)?;
    let zip = b"PK\x03\x04 the logs of the failing run".repeat(900);
    let paper = crate::pictures::double::pdf(3);
    rig.page.press("shift+m")?;
    rig.page.advance(0.6)?;
    read_to_the_end(&mut rig.page);
    rig.page.give_files(
        "comment-box",
        FileSource::Paste,
        &[
            ("run-logs.zip", &zip),
            ("spec.pdf", &paper),
            ("checkout-error.png", &one),
        ],
    )?;
    rig.page.type_text("Logs, the spec and the screenshot.")?;
    rig.page.advance(0.8)?;
    save(&rig.page, &fonts, Palette::Dark, "comment-file.png");
    Ok(())
}

/// The words of an issue edited: the description as a field with Save and Cancel under it, a comment of
/// the person's the same, and both in an 80 by 24 pane.
#[test]
#[ignore = "draws in the fonts of this machine: make shots"]
fn the_pictures_of_the_docs_with_edits() -> Result<(), Error> {
    let fonts = fonts();
    for (size, prefix) in [(BIG, ""), (PANE, "narrow-")] {
        let linear = Canned::new(Mode::Fine);
        with_my_comment(&linear);
        let session = session_over(&linear, Some("lin_api_test"), None);
        let mut page = open(&session, size, Some(&fonts), "ACM-412")?;
        page.press("enter")?;
        page.wait_for(Duration::from_secs(20), |page| {
            page.says("600 requests a minute") && pressable(page, "panel-value:state")
        })?;
        page.press("e")?;
        page.wait_for(Duration::from_secs(20), |page| pressable(page, "edit-save"))?;
        page.advance(0.4)?;
        page.type_text("\n- Keys of the free plan share one limit")?;
        page.advance(0.6)?;
        save(
            &page,
            &fonts,
            Palette::Dark,
            &format!("{prefix}edit-description.png"),
        );
        if prefix.is_empty() {
            save(&page, &fonts, Palette::Light, "light-edit-description.png");
        }
        page.press("escape")?;
        page.press("escape")?;
        page.wait_for(Duration::from_secs(20), |page| page.menu().is_some())?;
        page.pick_titled("Discard the edit")?;
        page.wait_for(Duration::from_secs(20), |page| !page.says("Save"))?;
        page.press("cmd+shift+m")?;
        page.wait_for(Duration::from_secs(20), |page| pressable(page, "edit-save"))?;
        page.advance(0.6)?;
        save(
            &page,
            &fonts,
            Palette::Dark,
            &format!("{prefix}edit-comment.png"),
        );
    }
    Ok(())
}
