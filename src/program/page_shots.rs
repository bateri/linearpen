//! The pictures of docs/, made from the page as it is: the page and the program's handlers in one
//! process (as `page_tests` runs them), over the fixtures of test/stand-in/, laid out and drawn in
//! the fonts of this machine (macOS: SF Mono, or Menlo, for the terminal's face, as bateri's default
//! is; SF for the system's) by pencere's host with no screen. Not a terminal's drawing, no shaping and
//! no shadows, but the same boxes, words and colours where the kit puts them. Nothing in them is from
//! the machine they are made on: the issues are made up, and Linear is not asked.
//!
//! They need the system's fonts, so they are not part of the gate: `make shots` makes them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_drive::shot::{Fonts, Palette};

use crate::page_tests::{Canned, Mode, PANE, Page, open, rig, session_over};

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
    open(&session, size, Some(fonts), "ACM-412")
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

    // Issues selected, to be taken to an agent.
    let mut page = lists(BIG, &fonts)?;
    page.press("x")?;
    page.press("shift+down")?;
    page.press("shift+down")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("3 selected"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "select.png");

    // Issues handed to an agent: the form (on a machine that has bateri and three agents), and the
    // list after the start, where each issue says who works on it.
    let linear = Canned::new(Mode::Fine);
    let machine = rig(&linear, true, &["claude", "codex", "cursor-agent"]);
    let mut page = open(&machine.session, BIG, Some(&fonts), "ACM-412")?;
    page.press("x")?;
    page.press("shift+down")?;
    page.press("shift+down")?;
    page.press("w")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Open in"))?;
    page.type_text(
        "Start with the rate limiter. The two others touch the same middleware, so do them in the same branch and say what you left out.",
    )?;
    page.click("work-folder/item/worktree")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("ada/acm-412"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "work.png");
    page.press("mod+enter")?;
    page.wait_for(Duration::from_secs(20), |page| {
        !page.says("Open in") && page.says("Claude")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "started.png");
    machine.hands.end();
    let machine = rig(&linear, true, &["claude", "codex", "cursor-agent"]);
    let mut page = open(&machine.session, PANE, Some(&fonts), "ACM-412")?;
    page.press("x")?;
    page.press("shift+down")?;
    page.press("shift+down")?;
    page.press("w")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Open in"))?;
    page.type_text("Start with the rate limiter.")?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "narrow-work.png");
    machine.hands.end();

    // The card of keys, and the palette.
    let mut page = lists(BIG, &fonts)?;
    page.press("?")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Keys"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "keys.png");
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
