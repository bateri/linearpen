//! The pictures on the page: those of Linear drawn in the text and large, and those the person adds to a
//! comment, to a new issue and to a description. The page (src/view/) and the program's handlers in
//! one process (as `page_tests` runs them), with a terminal that is a double (`pictures::double`) and
//! Linear in memory: nothing here starts a process or reaches Linear.
//!
//! Each opens the kit's interpreter: `make test-page` runs them, in `make check`.

use std::sync::Arc;

use pencere_drive::headless::Error;
use pencere_drive::shot::Fonts;
use pencere_view::FileSource;

use crate::page_tests::{
    Canned, Mode, Page, WIDE, WITHIN, comment_editor, made, open_new, open_the_issue, said,
    session_over, typing_in,
};
use crate::pictures::double::{self, Double};

/// The address of the picture in the text of the issue of the fixtures.
pub(crate) const SHOT: &str = "https://uploads.linear.app/ws/9a1f/checkout-error.png";

/// A picture of the first comment, and one that is not Linear's.
pub(crate) const REPLY: &str = "https://uploads.linear.app/ws/9a1f/reply.png";
pub(crate) const ELSEWHERE: &str = "https://example.com/elsewhere.png";

/// The description the issue ACM-412 has in these tests: the fixture's, and two pictures.
pub(crate) fn description() -> String {
    format!(
        "The public API has no limit per key. On Nov 3 one customer's import script took the read \
         replicas down.\n\n![Checkout error]({SHOT})\n\n**What we want**\n\n- A limit per API key, \
         600 requests a minute\n\n![Elsewhere]({ELSEWHERE})"
    )
}

/// A PDF in the text of the issue, and a zip, and a file of the first comment.
pub(crate) const PAPER: &str = "https://uploads.linear.app/ws/9a1f/spec.pdf";
pub(crate) const ARCHIVE: &str = "https://uploads.linear.app/ws/9a1f/logs.zip";
pub(crate) const NOTES: &str = "https://uploads.linear.app/ws/9a1f/notes.txt";

/// Everything a test of pictures holds.
pub(crate) struct Pictured {
    pub session: Arc<crate::session::Session>,
    pub linear: Arc<Canned>,
    pub page: Page,
    pub terminal: Arc<Double>,
}

/// The page over Linear in memory with pictures in the issue ACM-412: one in its description, and one in
/// its first comment.
pub(crate) fn open_pictured(size: (f32, f32), fonts: Option<&Fonts>) -> Result<Pictured, Error> {
    open_with(
        size,
        fonts,
        &description(),
        &format!("Here is what the customer sees: ![reply]({REPLY})"),
    )
}

/// The page over Linear in memory with files in the issue ACM-412: a PDF of three pages and a zip, each
/// a paragraph of its own in the description, and a text in its first comment.
pub(crate) fn open_filed(size: (f32, f32), fonts: Option<&Fonts>) -> Result<Pictured, Error> {
    let rig = open_with(
        size,
        fonts,
        &format!(
            "The public API has no limit per API key.\n\n[spec.pdf]({PAPER})\n\n[logs.zip]({ARCHIVE})\n\n\
             ![Checkout error]({SHOT})"
        ),
        &format!("The notes of the call:\n\n[notes.txt]({NOTES})"),
    )?;
    rig.linear.store(PAPER, double::pdf(3));
    rig.linear
        .store(ARCHIVE, b"PK\x03\x04 a zip, as far as a name goes".to_vec());
    rig.linear.store(NOTES, b"words".to_vec());
    Ok(rig)
}

fn open_with(
    size: (f32, f32),
    fonts: Option<&Fonts>,
    description: &str,
    first_comment: &str,
) -> Result<Pictured, Error> {
    open_with_patience(size, fonts, description, first_comment, None)
}

fn open_with_patience(
    size: (f32, f32),
    fonts: Option<&Fonts>,
    description: &str,
    first_comment: &str,
    patience: Option<std::time::Duration>,
) -> Result<Pictured, Error> {
    let linear = Canned::new(Mode::Fine);
    linear.set_detail(
        "issue-412",
        serde_json::json!({
            "description": description,
            "comments": { "nodes": [
                { "id": "c-1", "body": first_comment,
                  "createdAt": "2026-11-06T09:12:00.000Z", "user": { "displayName": "Deniz" } },
                { "id": "c-2", "body": "Keep a fallback for when Redis is down.",
                  "createdAt": "2026-11-08T16:40:00.000Z", "user": { "displayName": "Mert" } },
            ], "pageInfo": { "hasNextPage": false, "endCursor": null } },
        }),
    );
    let session = session_over(&linear, Some("lin_api_test"), None);
    let terminal = Arc::new(Double::default());
    session.pictures.using(terminal.clone());
    let mut page = crate::page_tests::open_patiently(&session, size, fonts, "ACM-412", patience)?;
    // The terminal's names for the bytes the program attaches: the page draws by them.
    for bytes in [double::the_picture(), double::pdf(3)] {
        let name = page.attach(&bytes);
        terminal.know(&bytes, &name);
    }
    Ok(Pictured {
        session,
        linear,
        page,
        terminal,
    })
}

/// The picture a person gives, its bytes (the colour tells one from another).
pub(crate) fn given(tint: u8) -> Vec<u8> {
    double::png(200, 120, [tint, 90, 60])
}

fn opened() -> Pictured {
    let mut rig = open_pictured(WIDE, None).expect("the page opens");
    open_the_issue(&mut rig.page);
    // Where the pictures are drawn the issue is as long as it will be: what a test does is where it sees it.
    wait_bounds(&mut rig.page, "pic:3");
    rig.page.advance(0.4).expect("a moment");
    rig
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_of_linear_in_the_text_is_fetched_with_the_key_and_drawn_where_it_is() {
    let Pictured {
        linear,
        mut page,
        terminal,
        ..
    } = opened();
    page.wait_for(WITHIN, |page| {
        page.bounds("pic:1").is_some() && page.bounds("pic:3").is_some()
    })
    .unwrap_or_else(|why| panic!("the pictures are drawn: {why:?}: {}", said(&page)));
    // Each address of Linear's was asked for once, with the key, and given to the terminal once; the one
    // that is not Linear's was not asked for with it or without it.
    // (They come at once, four at a time, in no order.)
    let mut fetched = linear.fetched();
    fetched.sort();
    assert_eq!(
        fetched,
        vec![
            (SHOT.to_owned(), "lin_api_test".to_owned()),
            (REPLY.to_owned(), "lin_api_test".to_owned())
        ],
        "{fetched:?}"
    );
    assert_eq!(terminal.attached().len(), 2);
    // The one that is not Linear's is a link in its words: it is not fetched, and a press opens it in the
    // browser.
    assert!(page.says("Elsewhere"), "{}", said(&page));
    assert!(
        page.bounds("pic:2").is_none() && page.bounds("file-card:2").is_none(),
        "{}",
        said(&page)
    );
    assert!(page.bounds("pic-ext:2").is_some(), "{}", said(&page));
    page.click("pic-ext:2").expect("the link");
    assert_eq!(page.links(), [ELSEWHERE.to_owned()]);
    // A picture is no wider than the room, nor larger than it is.
    let at = page.bounds("pic:1").unwrap();
    assert!(at.w <= 640.0 + 2.0 && at.h <= 520.0 + 2.0, "{at:?}");
}

/// The words of the page that are exactly `word`, counted.
fn count(page: &Page, word: &str) -> usize {
    page.words().iter().filter(|said| **said == word).count()
}

/// The issue is read to its end, as a person turns the wheel until nothing moves.
pub(crate) fn read_to_the_end(page: &mut Page) {
    for _ in 0..12 {
        page.wheel("panel-scroll", 0.0, 600.0).expect("the wheel");
        page.advance(0.3).expect("a moment");
    }
}

/// The issue is read on until the element named `name` is under the pointer's reach.
pub(crate) fn read_to(page: &mut Page, name: &str) {
    for _ in 0..16 {
        if page.hover(name).is_ok() {
            return;
        }
        page.wheel("panel-scroll", 0.0, 150.0).expect("the wheel");
        page.advance(0.3).expect("a moment");
    }
    panic!("{name} cannot be reached: {}", said(page));
}

fn wait_says(page: &mut Page, words: &str) {
    page.wait_for(WITHIN, |page| page.says(words))
        .unwrap_or_else(|why| panic!("the page says {words:?}: {why:?}: {}", said(page)));
}

fn wait_bounds(page: &mut Page, name: &str) {
    page.wait_for(WITHIN, |page| page.bounds(name).is_some())
        .unwrap_or_else(|why| panic!("{name} is on the page: {why:?}: {}", said(page)));
}

fn wait_gone(page: &mut Page, name: &str) {
    page.wait_for(WITHIN, |page| page.bounds(name).is_none())
        .unwrap_or_else(|why| panic!("{name} is gone: {why:?}: {}", said(page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_linear_cannot_show_is_an_image_card_with_its_reason_and_one_verb() {
    let mut rig = open_pictured(WIDE, None).expect("the page opens");
    rig.linear
        .store(SHOT, b"this is words and not a picture".to_vec());
    open_the_issue(&mut rig.page);
    let page = &mut rig.page;
    wait_bounds(page, "file-card:1");
    // It is a card of an image, with why it is not drawn, and one verb: Save. There is no other button.
    assert!(page.bounds("file-save:1").is_some(), "{}", said(page));
    assert!(page.says("Image"), "{}", said(page));
    assert!(page.says("Not shown here"), "{}", said(page));
    assert!(page.says("Checkout error"), "{}", said(page));
    assert!(
        page.bounds("pic:1").is_none() && page.bounds("pic-link:1").is_none(),
        "{}",
        said(page)
    );
    // The other picture of the text is drawn as it was.
    wait_bounds(page, "pic:3");
    // The rest of what is done to the card is its right-click menu (and the commands): Open the link,
    // Copy the file's link, and Save.
    page.right_click("file-card:1").expect("a right press");
    page.wait_for(WITHIN, |page| page.menu().is_some())
        .unwrap_or_else(|why| panic!("the menu: {why:?}: {}", said(page)));
    let titles = crate::page_tests::open_menu_titles(page);
    for words in ["Save", "Open the link", "Copy the file's link"] {
        assert!(
            titles.iter().any(|title| title == words),
            "{words}: {titles:?}"
        );
    }
    assert!(!titles.iter().any(|title| title == "Open"), "{titles:?}");
    page.pick_titled("Open the link").expect("Open the link");
    assert_eq!(page.links(), [SHOT.to_owned()]);
    page.right_click("file-card:1").expect("a right press");
    page.wait_for(WITHIN, |page| page.menu().is_some())
        .unwrap_or_else(|why| panic!("the menu: {why:?}: {}", said(page)));
    page.pick_titled("Copy the file's link").expect("Copy");
    assert_eq!(page.pasteboard(), Some(SHOT), "{}", said(page));
    // The card the menu was opened on is the one the keys are for, until a press elsewhere.
    page.press("mod+shift+e").expect("open the link");
    page.wait_for(WITHIN, |page| page.links().len() == 2)
        .unwrap_or_else(|why| panic!("the key opens it: {why:?}: {}", said(page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn where_the_terminal_holds_no_files_the_text_is_its_words_as_it_was_before_the_pictures() {
    let mut rig = open_pictured(WIDE, None).expect("the page opens");
    rig.terminal.no_view();
    open_the_issue(&mut rig.page);
    let page = &mut rig.page;
    wait_says(page, "Checkout error");
    page.advance(0.5).expect("a moment");
    assert!(
        page.bounds("pic:1").is_none() && page.bounds("pic-link:1").is_none(),
        "{}",
        said(page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_press_on_a_picture_opens_it_over_the_pane_and_escape_puts_it_away_with_the_keyboard_on_it() {
    let mut rig = opened();
    let page = &mut rig.page;
    wait_bounds(page, "pic:1");
    assert!(
        page.says("API key") && page.says("COMMENTS"),
        "{}",
        said(page)
    );
    page.click("pic:1").expect("the picture");
    wait_bounds(page, "picture-close");
    // It has the pane: its name and the way out are at the top, and the list and the issue are not there.
    assert!(
        page.says("Checkout error") && page.says("Close") && page.says("Esc"),
        "{}",
        said(page)
    );
    assert!(
        !page.says("ACM-371") && !page.says("COMMENTS"),
        "{}",
        said(page)
    );
    wait_bounds(page, "picture-zoom");
    // Escape puts it away: the issue is as it was, and the keyboard is on the picture (Enter opens it again).
    page.press("escape").expect("escape");
    wait_gone(page, "picture-close");
    wait_says(page, "COMMENTS");
    page.press("enter").expect("enter");
    wait_bounds(page, "picture-close");
    // Close is a button as well.
    page.click("picture-close").expect("close");
    wait_gone(page, "picture-close");
    wait_says(page, "COMMENTS");
    // The picture is let go with the issue: both pictures of it, nothing else.
    page.press("escape").expect("escape");
    wait_gone(page, "panel-close");
    page.wait_for(WITHIN, |_| rig.terminal.detached().len() == 2)
        .unwrap_or_else(|why| {
            panic!(
                "the pictures are let go: {why:?}: {:?}",
                rig.terminal.detached()
            )
        });
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_that_is_large_zooms_where_the_kit_zooms() {
    let mut rig = opened();
    let page = &mut rig.page;
    wait_bounds(page, "pic:1");
    page.click("pic:1").expect("the picture");
    wait_bounds(page, "picture-zoom");
    let Ok(before) = page.zoom_of("picture-zoom") else {
        // (A kit that has no zoom: the picture stays at its fit.)
        return;
    };
    assert!((before - 1.0).abs() < 0.01, "{before}");
    page.pinch("picture-zoom", 0.6).expect("a pinch");
    let after = page.zoom_of("picture-zoom").expect("a zoom");
    assert!(after > 1.2, "{after}");
}

/// What is pasted into the comment: one picture, and the handle it was given.
fn paste_into_comment(rig: &mut Pictured, name: &str, bytes: &[u8]) -> String {
    let page = &mut rig.page;
    page.press("shift+m").expect("shift and m");
    page.wait_for(WITHIN, |page| typing_in(page, &comment_editor("issue-412")))
        .unwrap_or_else(|why| panic!("the keyboard is in the field: {why:?}: {}", said(page)));
    // (The issue is read to its end, where the field is, and that is a motion.)
    page.advance(0.6).expect("a moment");
    if page.hover("comment-box").is_err() {
        read_to_the_end(page);
    }
    let handles = page
        .give_files("comment-box", FileSource::Paste, &[(name, bytes)])
        .expect("pasted");
    rig.terminal.give(&handles[0], bytes);
    handles[0].clone()
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_pasted_into_the_comment_is_shown_small_and_goes_to_linear_before_the_comment() {
    let mut rig = opened();
    let bytes = given(200);
    paste_into_comment(&mut rig, "shot.png", &bytes);
    let page = &mut rig.page;
    wait_says(page, "shot.png");
    assert!(page.says("Remove"), "{}", said(page));
    // Comment can be pressed with the picture alone: the comment is the picture.
    page.wait_for(WITHIN, |page| {
        crate::page_tests::pressable(page, "comment-send")
    })
    .unwrap_or_else(|why| panic!("Comment can be pressed: {why:?}: {}", said(page)));
    page.press("mod+enter").expect("command and enter");
    page.wait_for(WITHIN, |_| rig.linear.commented().len() == 1)
        .unwrap_or_else(|why| panic!("the comment is sent: {why:?}: {}", said(&rig.page)));
    let sent = &rig.linear.commented()[0];
    assert_eq!(
        sent["body"],
        "![shot.png](https://uploads.linear.app/stand-in/1)"
    );
    // The picture went to the storage Linear named, with the headers it named, and no key.
    let put = rig.linear.put();
    assert_eq!(put.len(), 1);
    assert_eq!(put[0].body, bytes);
    assert!(
        put[0].url.starts_with("https://storage.stand-in/put/1"),
        "{}",
        put[0].url
    );
    assert!(
        put[0]
            .headers
            .contains(&("x-stand-in".to_owned(), "yes".to_owned())),
        "{:?}",
        put[0].headers
    );
    assert!(
        put[0]
            .headers
            .contains(&("Content-Type".to_owned(), "image/png".to_owned())),
        "{:?}",
        put[0].headers
    );
    assert!(
        put[0]
            .headers
            .iter()
            .all(|(name, value)| !name.eq_ignore_ascii_case("authorization")
                && !value.contains("lin_api")),
        "{:?}",
        put[0].headers
    );
    // The address the storage signed is nowhere on the page, and the tray is empty.
    let page = &mut rig.page;
    wait_says(page, "Deniz");
    page.wait_for(WITHIN, |page| !page.says("Remove"))
        .unwrap_or_else(|why| panic!("the tray is empty: {why:?}: {}", said(page)));
    assert!(
        !said(page).contains("SECRET") && !said(page).contains("storage.stand-in"),
        "{}",
        said(page)
    );
}

/// The files of `open_filed`, with the issue read to where the cards are drawn.
fn filed() -> Pictured {
    let mut rig = open_filed(WIDE, None).expect("the page opens");
    open_the_issue(&mut rig.page);
    wait_bounds(&mut rig.page, "file-card:1");
    wait_bounds(&mut rig.page, "pic:3");
    rig.page.advance(0.4).expect("a moment");
    rig
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_pdf_and_a_zip_of_linear_are_cards_that_say_what_they_are_and_what_can_be_done() {
    let rig = filed();
    let page = &rig.page;
    // Each is a card: its name, what it is, and its ways; the PDF alone can be read, in place.
    assert!(page.bounds("file-card:2").is_some(), "{}", said(page));
    assert!(page.bounds("file-open:1").is_some(), "{}", said(page));
    assert!(page.bounds("file-open:2").is_none(), "{}", said(page));
    assert!(
        page.says("spec.pdf") && page.says("logs.zip"),
        "{}",
        said(page)
    );
    assert!(page.says("PDF") && page.says("ZIP"), "{}", said(page));
    assert!(page.says("3 pages"), "{}", said(page));
    // One verb to a card: Open for the PDF, Save for the others (the zip, the log of the comment).
    assert_eq!(count(page, "Save"), 2, "{}", said(page));
    assert!(page.bounds("file-save:1").is_none(), "{}", said(page));
    assert!(page.bounds("file-save:2").is_some(), "{}", said(page));
    assert_eq!(count(page, "Open in Linear"), 1, "{}", said(page));
    // The picture beside them is drawn.
    assert!(page.bounds("pic:3").is_some(), "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_press_on_the_card_itself_is_its_verb() {
    let mut rig = filed();
    let page = &mut rig.page;
    // The zip's card: Save. A press on its name, not on the button.
    let card = page.bounds("file-card:2").expect("the card");
    let button = page.bounds("file-save:2").expect("the button");
    assert!(
        card.x + 40.0 < button.x,
        "the card is wider than its button: {card:?} {button:?}"
    );
    click_run(
        page,
        pencere_view::Point {
            x: card.x + 40.0,
            y: card.y + card.h / 2.0,
        },
        1,
    );
    assert!(
        matches!(
            page.panel(),
            Some(pencere_view::Panel::Save { name, .. }) if name == "logs.zip"
        ),
        "{:?}",
        page.panel()
    );
    page.choose_place(None).expect("turned away");
    // The PDF's card: Open.
    let card = page.bounds("file-card:1").expect("the card");
    click_run(
        page,
        pencere_view::Point {
            x: card.x + 40.0,
            y: card.y + card.h / 2.0,
        },
        1,
    );
    wait_bounds(page, "picture-close");
    assert!(page.says("Page 1 of 3"), "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn open_reads_a_pdf_over_the_pane_page_by_page_and_escape_puts_it_away() {
    let mut rig = filed();
    let page = &mut rig.page;
    page.click("file-open:1").expect("open");
    wait_bounds(page, "picture-close");
    assert!(page.says("Page 1 of 3"), "{}", said(page));
    assert!(page.says("spec.pdf"), "{}", said(page));
    // The first page has no page before it.
    assert!(
        !crate::page_tests::pressable(page, "pdf-previous"),
        "{}",
        said(page)
    );
    page.click("pdf-next").expect("next");
    wait_says(page, "Page 2 of 3");
    page.press("right").expect("right");
    wait_says(page, "Page 3 of 3");
    // The last page has no page after it, and the arrow stays.
    assert!(
        !crate::page_tests::pressable(page, "pdf-next"),
        "{}",
        said(page)
    );
    page.press("right").expect("right");
    page.advance(0.3).expect("a moment");
    assert!(page.says("Page 3 of 3"), "{}", said(page));
    page.press("left").expect("left");
    wait_says(page, "Page 2 of 3");
    page.press("escape").expect("escape");
    wait_gone(page, "picture-close");
    // The issue is as it was, and the PDF is let go with it only when the issue is.
    assert!(page.bounds("file-card:1").is_some(), "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn save_asks_where_and_the_file_is_written_there_from_linear() {
    let mut rig = filed();
    let page = &mut rig.page;
    page.click("file-save:2").expect("save");
    assert!(
        matches!(
            page.panel(),
            Some(pencere_view::Panel::Save { name, .. }) if name == "logs.zip"
        ),
        "{:?}",
        page.panel()
    );
    let handle = page
        .choose_place(Some("logs.zip"))
        .expect("chosen")
        .expect("a handle");
    page.wait_for(WITHIN, |_| !rig.terminal.saved().is_empty())
        .unwrap_or_else(|why| panic!("the file is saved: {why:?}: {}", said(&rig.page)));
    let saved = rig.terminal.saved();
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0].0, handle);
    assert_eq!(saved[0].1, b"PK\x03\x04 a zip, as far as a name goes");
    wait_says(&mut rig.page, "Saved logs.zip");
    // A person who turns the panel away saves nothing.
    let page = &mut rig.page;
    page.click("file-save:2").expect("save");
    page.choose_place(None).expect("turned away");
    page.advance(0.3).expect("a moment");
    assert_eq!(rig.terminal.saved().len(), 1);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_save_the_terminal_cannot_write_says_so() {
    let mut rig = filed();
    rig.terminal.save_fails();
    let page = &mut rig.page;
    page.click("file-save:2").expect("save");
    page.choose_place(Some("logs.zip")).expect("chosen");
    wait_says(page, "Could not save logs.zip");
    assert!(rig.terminal.saved().is_empty());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_file_of_any_kind_pasted_into_the_comment_goes_to_linear_as_a_link() {
    let mut rig = opened();
    let bytes = b"the words of a log".to_vec();
    paste_into_comment(&mut rig, "run.log", &bytes);
    let page = &mut rig.page;
    wait_says(page, "run.log");
    page.press("mod+enter").expect("command and enter");
    page.wait_for(WITHIN, |_| rig.linear.commented().len() == 1)
        .unwrap_or_else(|why| panic!("the comment is sent: {why:?}: {}", said(&rig.page)));
    // A file that is not a picture is a link, not a picture, in the text.
    assert_eq!(
        rig.linear.commented()[0]["body"],
        "[run.log](https://uploads.linear.app/stand-in/1)"
    );
    let put = rig.linear.put();
    assert_eq!(put.len(), 1);
    assert_eq!(put[0].body, bytes);
    assert!(
        put[0].headers.contains(&(
            "Content-Type".to_owned(),
            "application/octet-stream".to_owned()
        )),
        "{:?}",
        put[0].headers
    );
}

/// A PDF that is large: opened from its card, and read by the program (it has said how many pages and
/// that there are words).
fn reading() -> Pictured {
    let mut rig = filed();
    rig.page.click("file-open:1").expect("open");
    wait_bounds(&mut rig.page, "picture-close");
    wait_bounds(&mut rig.page, "pdf-find-open");
    // The words of the first page come as the program reads them.
    rig.page.advance(0.4).expect("a moment");
    rig
}

/// The point of the page that is large, at `x`, `y` points of the PDF's own (420 by 560).
pub(crate) fn on_the_page(page: &Page, x: f32, y: f32) -> pencere_view::Point {
    let at = page.bounds("pdf-page").expect("the page is drawn");
    pencere_view::Point {
        x: at.x + x / 420.0 * at.w,
        y: at.y + y / 560.0 * at.h,
    }
}

/// The person clicks at `at` `times` times in a run (a double click, a triple click).
fn click_run(page: &mut Page, at: pencere_view::Point, times: u32) {
    use pencere_view::{Button, Input, Mods};
    for count in 1..=times {
        page.pointer(Input::Press {
            at,
            button: Button::Left,
            mods: Mods::default(),
            count,
        })
        .expect("a press");
        page.pointer(Input::Release {
            at,
            button: Button::Left,
            mods: Mods::default(),
        })
        .expect("a release");
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_pdf_that_is_large_is_read_by_the_program_and_let_go_with_the_view() {
    let mut rig = reading();
    assert!(rig.session.document("issue-412").is_some());
    assert!(rig.page.says("Page 1 of 3"), "{}", said(&rig.page));
    rig.page.press("escape").expect("escape");
    wait_gone(&mut rig.page, "picture-close");
    assert!(rig.session.document("issue-412").is_none());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_pdf_with_no_words_has_none_to_find() {
    let mut rig = open_filed(WIDE, None).expect("the page opens");
    let blank = "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
        2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
        3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>\nendobj\n\
        4 0 obj\n<< /Length 21 >>\nstream\n1 0 0 rg 0 0 50 50 re f\nendstream\nendobj\n\
        trailer\n<< /Size 5 /Root 1 0 R >>\n%%EOF\n";
    rig.linear.store(PAPER, blank.as_bytes().to_vec());
    open_the_issue(&mut rig.page);
    wait_bounds(&mut rig.page, "file-open:1");
    rig.page.click("file-open:1").expect("open");
    wait_bounds(&mut rig.page, "picture-close");
    rig.page.advance(0.6).expect("a moment");
    assert!(
        rig.page.bounds("pdf-find-open").is_none(),
        "{}",
        said(&rig.page)
    );
    assert!(rig.session.document("issue-412").is_some());
    // Where the program cannot read it, the pages are drawn all the same.
    assert!(rig.page.says("Page 1"), "{}", said(&rig.page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn words_are_selected_with_the_pointer_and_copied_and_a_press_elsewhere_clears_them() {
    let mut rig = reading();
    let page = &mut rig.page;
    // "Page 1" is set at (40, 360) from the bottom left: its line is about 170 to 208 from the top.
    let from = on_the_page(page, 41.0, 190.0);
    let to = on_the_page(page, 400.0, 190.0);
    page.drag_between(from, to).expect("a drag");
    page.wait_for(WITHIN, |page| page.bounds("pdf-selection:1").is_some())
        .unwrap_or_else(|why| panic!("the words are marked: {why:?}: {}", said(page)));
    page.press("mod+c").expect("copy");
    assert_eq!(page.pasteboard(), Some("Page 1"), "{}", said(page));
    wait_says(page, "Copied 2 words");
    // Shift and the arrows take more of the words: a line down.
    page.press("shift+down").expect("shift and down");
    page.advance(0.3).expect("a moment");
    assert!(page.bounds("pdf-selection:2").is_some(), "{}", said(page));
    page.press("mod+c").expect("copy");
    assert_eq!(
        page.pasteboard().map(|words| words.lines().count()),
        Some(2),
        "{:?}",
        page.pasteboard()
    );
    // Esc is Close's and nothing of the selection's: a press elsewhere (above the page, on no button) lets
    // it go.
    let zoom = page.bounds("picture-zoom").expect("the page's box");
    let above = pencere_view::Point {
        x: zoom.x + 20.0,
        y: zoom.y - 4.0,
    };
    click_run(page, above, 1);
    page.advance(0.3).expect("a moment");
    assert!(page.bounds("pdf-selection:1").is_none(), "{}", said(page));
    assert!(page.bounds("picture-close").is_some(), "{}", said(page));
    // A double click takes a word, a triple click the line.
    let word = on_the_page(page, 60.0, 190.0);
    click_run(page, word, 2);
    page.press("mod+c").expect("copy");
    assert_eq!(page.pasteboard(), Some("Page"), "{}", said(page));
    // A triple click takes the line.
    click_run(page, word, 3);
    page.press("mod+c").expect("copy");
    assert_eq!(page.pasteboard(), Some("Page 1"), "{}", said(page));
    // Select all takes the words of the page.
    page.press("mod+a").expect("select all");
    page.press("mod+c").expect("copy");
    assert_eq!(
        page.pasteboard(),
        Some("Page 1\nThe text of the page"),
        "{}",
        said(page)
    );
    // The words of another page are another page's: a page turned has nothing selected.
    page.press("right").expect("right");
    wait_says(page, "Page 2 of 3");
    assert!(page.bounds("pdf-selection:1").is_none(), "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn escape_is_written_on_the_button_it_does_and_closes_the_view_with_words_selected() {
    let mut rig = reading();
    let page = &mut rig.page;
    // One button says Esc, and the bottom line none of it; the buttons say their keys and the line does not
    // repeat them (it says what no button has: copying, and selecting all).
    let words = page.words();
    assert_eq!(count(page, "Esc"), 1, "{words:?}");
    for key in ["\u{2190}", "\u{2192}", "\u{2318}S", "\u{2318}F"] {
        assert_eq!(count(page, key), 1, "{key}: {words:?}");
    }
    assert!(page.says("select all"), "{words:?}");
    assert!(!page.says("close") && !page.says("page"), "{words:?}");
    // (On the second page both of the buttons that turn it can be pressed.)
    page.press("right").expect("right");
    wait_says(page, "Page 2 of 3");
    let from = on_the_page(page, 41.0, 190.0);
    let to = on_the_page(page, 400.0, 190.0);
    page.drag_between(from, to).expect("a drag");
    page.wait_for(WITHIN, |page| page.bounds("pdf-selection:1").is_some())
        .unwrap_or_else(|why| panic!("the words are marked: {why:?}: {}", said(page)));
    assert!(page.says("copy"), "{}", said(page));
    // The search: Done has Esc and Close has none; the matches have their keys; and no button of the row
    // above moves (Close, the last of its row, only grows narrower from its left).
    let before: Vec<_> = [
        "pdf-save",
        "pdf-previous",
        "pdf-next",
        "pdf-find-open",
        "picture-close",
    ]
    .iter()
    .map(|name| {
        page.bounds(name)
            .unwrap_or_else(|| panic!("{name}: {}", said(page)))
    })
    .collect();
    page.press("mod+f").expect("find");
    wait_bounds(page, "pdf-find-bar");
    for (name, was) in [
        "pdf-save",
        "pdf-previous",
        "pdf-next",
        "pdf-find-open",
        "picture-close",
    ]
    .iter()
    .zip(&before)
    {
        let now = page.bounds(name).expect("still drawn");
        let (was_right, now_right) = (was.x + was.w, now.x + now.w);
        assert!(
            (was.y - now.y).abs() < 0.5 && (was_right - now_right).abs() < 0.5,
            "{name} moved: {was:?} {now:?}"
        );
        if *name != "picture-close" {
            assert!((was.x - now.x).abs() < 0.5, "{name} moved: {was:?} {now:?}");
        }
    }
    let words = page.words();
    assert_eq!(count(page, "Esc"), 1, "{words:?}");
    assert_eq!(count(page, "\u{21e7}\u{2318}G"), 1, "{words:?}");
    assert_eq!(count(page, "\u{2318}G"), 1, "{words:?}");
    page.press("escape").expect("escape");
    wait_gone(page, "pdf-find-bar");
    assert!(page.bounds("picture-close").is_some(), "{}", said(page));
    // With the words still selected, Esc closes the view: it does the job of the button it is written on.
    assert!(page.bounds("pdf-selection:1").is_some(), "{}", said(page));
    page.press("escape").expect("escape");
    wait_gone(page, "picture-close");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_search_marks_the_matches_and_goes_from_page_to_page() {
    let mut rig = reading();
    let page = &mut rig.page;
    page.press("mod+f").expect("find");
    wait_bounds(page, "pdf-find-bar");
    page.type_text("text of").expect("typed");
    // "The text of the page" is on every page: the first match is on the page that is shown.
    wait_says(page, "1 of 3");
    wait_bounds(page, "pdf-match:1");
    assert!(page.says("Page 1 of 3"), "{}", said(page));
    // Enter goes to the next, which is on the page after: the page is turned to it.
    page.press("enter").expect("enter");
    wait_says(page, "2 of 3");
    wait_says(page, "Page 2 of 3");
    wait_bounds(page, "pdf-match:2");
    // The keys of the commands go round the ends, with the field or without.
    page.press("mod+g").expect("next");
    wait_says(page, "3 of 3");
    page.press("mod+g").expect("next");
    wait_says(page, "1 of 3");
    wait_says(page, "Page 1 of 3");
    page.press("shift+enter").expect("previous");
    wait_says(page, "3 of 3");
    page.click("pdf-find-previous").expect("previous match");
    wait_says(page, "2 of 3");
    // Words nobody wrote say so, and the marks go. (The keyboard is on the button that was pressed: the
    // key of the search brings it back to the field.)
    page.press("mod+f").expect("find");
    page.type_text("zzz").expect("typed");
    wait_says(page, "No match");
    assert!(page.bounds("pdf-match:1").is_none(), "{}", said(page));
    // Escape puts the search away; the view is still there.
    page.press("escape").expect("escape");
    wait_gone(page, "pdf-find-bar");
    assert!(page.bounds("picture-close").is_some(), "{}", said(page));
    // The search is not kept: opened again it is empty.
    page.click("pdf-find-open").expect("find");
    wait_bounds(page, "pdf-find-bar");
    assert!(!page.says("No match"), "{}", said(page));
    page.click("pdf-find-close").expect("done");
    wait_gone(page, "pdf-find-bar");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_taken_out_is_not_sent_and_a_comment_of_words_goes_as_it_did() {
    let mut rig = opened();
    let bytes = given(120);
    paste_into_comment(&mut rig, "one.png", &bytes);
    let page = &mut rig.page;
    wait_says(page, "one.png");
    let remove = page
        .outline()
        .accessible
        .iter()
        .find(|node| node.words.contains("Remove"))
        .and_then(|node| node.id)
        .expect("a Remove");
    let _ = remove;
    page.click("file-remove:1").expect("remove");
    page.wait_for(WITHIN, |page| !page.says("one.png"))
        .unwrap_or_else(|why| panic!("the picture is out: {why:?}: {}", said(page)));
    page.type_text("Only words").expect("words");
    page.press("mod+enter").expect("command and enter");
    page.wait_for(WITHIN, |_| rig.linear.commented().len() == 1)
        .unwrap_or_else(|why| panic!("the comment is sent: {why:?}: {}", said(&rig.page)));
    assert_eq!(rig.linear.commented()[0]["body"], "Only words");
    assert!(rig.linear.put().is_empty() && rig.linear.times("FileUpload") == 0);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_file_of_any_kind_is_kept_but_one_over_a_hundred_megabytes_is_turned_away_with_words() {
    let mut rig = opened();
    // One over a hundred megabytes: said, and not kept.
    let big = vec![0_u8; 100 * 1024 * 1024 + 1];
    let _ = paste_into_comment(&mut rig, "big.mov", &big);
    let page = &mut rig.page;
    wait_says(page, "Files are up to 100 MiB");
    assert!(
        !page.says("Remove") && !page.says("big.mov"),
        "{}",
        said(page)
    );
    // Words and not a picture: kept, with the icon of its kind and its name, and Remove.
    read_to_the_end(page);
    page.give_files(
        "comment-box",
        FileSource::Paste,
        &[("notes.txt", b"words and not a picture")],
    )
    .expect("pasted");
    wait_says(page, "notes.txt");
    assert!(page.says("Remove"), "{}", said(page));
    assert_eq!(count(page, "Remove"), 1, "{}", said(page));
    // (The words go after a while.)
    page.advance(6.0).expect("the words go");
    assert!(!page.says("Files are up to 100 MiB"), "{}", said(page));
    // More than ten are kept to ten, and the person is told.
    let one = given(30);
    let files: Vec<(String, &[u8])> = (0..11)
        .map(|n| (format!("p{n}.png"), one.as_slice()))
        .collect();
    let files: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, bytes)| (name.as_str(), *bytes))
        .collect();
    page.give_files("comment-box", FileSource::Paste, &files)
        .expect("pasted");
    page.advance(0.4).expect("a moment");
    assert_eq!(count(page, "Remove"), 10, "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_drop_goes_to_the_place_it_is_on_and_nowhere_else() {
    let mut rig = opened();
    let bytes = given(90);
    let page = &mut rig.page;
    wait_bounds(page, "describe-zone");
    // On nothing that takes pictures: said, and nothing is kept.
    page.give_files("panel-close", FileSource::Drop, &[("c.png", &bytes)])
        .expect("dropped");
    wait_says(page, "Drop files on");
    assert!(
        !page.says("c.png") && !page.says("Add to description"),
        "{}",
        said(page)
    );
    // On the description: its tray, with the words that add them.
    page.give_files("describe-zone", FileSource::Drop, &[("a.png", &bytes)])
        .expect("dropped");
    wait_says(page, "Add to description");
    assert!(page.says("Cancel") && page.says("a.png"), "{}", said(page));
    // On the comment: its own.
    read_to_the_end(page);
    page.give_files("comment-box", FileSource::Drop, &[("b.png", &bytes)])
        .expect("dropped");
    wait_says(page, "b.png");
    assert_eq!(count(page, "Remove"), 2, "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn attach_file_opens_the_panel_for_the_place_it_is_in_and_the_key_is_written_once() {
    let mut rig = opened();
    let bytes = given(60);
    let page = &mut rig.page;
    wait_bounds(page, "attach-file:describe");
    read_to_the_end(page);
    wait_bounds(page, "attach-file:comment");
    // The key is on one button: the description's, while the keyboard is not in the comment.
    assert_eq!(count(page, "\u{21e7}\u{2318}U"), 1, "{}", said(page));
    page.click("attach-file:comment").expect("add image");
    assert!(page.panel().is_some(), "the Open panel is up");
    let handles = page.choose_files(&[("c.png", &bytes)]).expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "c.png");
    // It is the comment's: no description tray.
    assert!(!page.says("Add to description"), "{}", said(page));
    // The key, from the field of the comment, is the comment's too, and still written once.
    page.press("shift+m").expect("shift and m");
    page.wait_for(WITHIN, |page| typing_in(page, &comment_editor("issue-412")))
        .unwrap_or_else(|why| panic!("the keyboard is in the field: {why:?}: {}", said(page)));
    assert_eq!(count(page, "\u{21e7}\u{2318}U"), 1, "{}", said(page));
    page.press("mod+shift+u").expect("the key");
    assert!(page.panel().is_some(), "the Open panel is up");
    // The person turns it away: nothing is added.
    page.choose_files(&[]).expect("turned away");
    page.advance(0.3).expect("a moment");
    assert_eq!(count(page, "Remove"), 1, "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn pictures_added_to_the_description_go_at_its_end_after_what_linear_holds_and_cancel_lets_them_go()
{
    let mut rig = opened();
    let bytes = given(150);
    let page = &mut rig.page;
    wait_bounds(page, "attach-file:describe");
    read_to(page, "attach-file:describe");
    page.click("attach-file:describe").expect("add image");
    let handles = page.choose_files(&[("shot.png", &bytes)]).expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "Add to description");
    page.advance(0.4).expect("a moment");
    // Cancel (and Esc) lets them go; the issue stays open and nothing was written.
    page.click("describe-cancel").expect("cancel");
    page.wait_for(WITHIN, |page| !page.says("Add to description"))
        .unwrap_or_else(|why| panic!("the tray is gone: {why:?}: {}", said(page)));
    assert!(
        page.says("COMMENTS") && rig.linear.changed().is_empty() && rig.linear.put().is_empty()
    );
    read_to(page, "attach-file:describe");
    page.click("attach-file:describe").expect("add image");
    let handles = page.choose_files(&[("shot.png", &bytes)]).expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "Add to description");
    page.press("escape").expect("escape");
    page.wait_for(WITHIN, |page| !page.says("Add to description"))
        .unwrap_or_else(|why| panic!("the tray is gone: {why:?}: {}", said(page)));
    assert!(
        page.says("COMMENTS"),
        "Esc let the pictures go, not the issue: {}",
        said(page)
    );
    // Now add them.
    read_to(page, "attach-file:describe");
    page.click("attach-file:describe").expect("add image");
    let handles = page.choose_files(&[("shot.png", &bytes)]).expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "Add to description");
    page.advance(0.4).expect("a moment");
    page.click("describe-add").expect("add");
    crate::page_tests::changed(&rig.linear, &mut rig.page, 1);
    let changed = &rig.linear.changed()[0];
    assert_eq!(changed["ids"], serde_json::json!(["issue-412"]));
    assert_eq!(
        changed["input"],
        serde_json::json!({ "description": format!("{}\n\n![shot.png](https://uploads.linear.app/stand-in/1)", description()) })
    );
    assert_eq!(rig.linear.put().len(), 1);
    // The issue is asked for again: the new picture is drawn with the others, from Linear.
    let page = &mut rig.page;
    wait_bounds(page, "pic:4");
    assert!(
        rig.linear
            .fetched()
            .iter()
            .any(|(url, _)| url == "https://uploads.linear.app/stand-in/1"),
        "{:?}",
        rig.linear.fetched()
    );
    assert!(!page.says("Add to description"), "{}", said(page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_storage_that_turns_a_picture_away_writes_nothing_and_keeps_the_pictures_for_another_try() {
    let mut rig = opened();
    rig.linear.refuse_storage();
    let bytes = given(33);
    let page = &mut rig.page;
    wait_bounds(page, "attach-file:describe");
    read_to(page, "attach-file:describe");
    page.click("attach-file:describe").expect("add image");
    let handles = page.choose_files(&[("shot.png", &bytes)]).expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "Add to description");
    page.advance(0.4).expect("a moment");
    page.click("describe-add").expect("add");
    page.wait_for(WITHIN, |_| rig.linear.put().len() == 1)
        .unwrap_or_else(|why| panic!("the storage is asked: {why:?}"));
    page.advance(0.5).expect("a moment");
    assert!(
        rig.linear.changed().is_empty(),
        "no description was written"
    );
    assert!(
        page.says("Add to description") && page.says("shot.png"),
        "{}",
        said(page)
    );
    assert!(
        !said(page).contains("SECRET") && !said(page).contains("storage.stand-in"),
        "{}",
        said(page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_new_issue_carries_its_pictures_at_the_end_of_its_description() {
    let mut rig = open_pictured(WIDE, None).expect("the page opens");
    let bytes = given(77);
    let page = &mut rig.page;
    open_new(page);
    wait_bounds(page, "attach-file:new");
    page.type_text("A title with a picture").expect("a title");
    page.click("attach-file:new").expect("add image");
    let handles = page
        .choose_files(&[("sketch.png", &bytes)])
        .expect("chosen");
    rig.terminal.give(&handles[0], &bytes);
    wait_says(page, "sketch.png");
    page.click("new-create").expect("create");
    made(&rig.linear, &mut rig.page, 1);
    let sent = &rig.linear.created()[0];
    assert_eq!(sent["title"], "A title with a picture");
    assert_eq!(
        sent["description"],
        "![sketch.png](https://uploads.linear.app/stand-in/1)"
    );
    assert_eq!(rig.linear.put().len(), 1);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn in_a_narrow_pane_the_buttons_stay_in_their_rows_and_the_pictures_wrap() {
    let mut rig = open_pictured(crate::page_tests::PANE, None).expect("the page opens");
    let bytes = given(240);
    let page = &mut rig.page;
    page.press("enter").expect("enter");
    wait_bounds(page, "attach-file:describe");
    page.wheel("panel-scroll", 0.0, 120.0).expect("read on");
    page.advance(0.4).expect("a moment");
    read_to(page, "describe-zone");
    let handles = page
        .give_files(
            "describe-zone",
            FileSource::Drop,
            &[
                ("one.png", &bytes),
                ("two-with-a-long-name.png", &bytes),
                ("three.png", &bytes),
            ],
        )
        .expect("dropped");
    assert_eq!(handles.len(), 3);
    wait_says(page, "Add to description");
    for name in [
        "attach-file:describe",
        "describe-add",
        "describe-cancel",
        "file-remove:1",
        "file-remove:3",
    ] {
        let at = page
            .bounds(name)
            .unwrap_or_else(|| panic!("{name} is drawn: {}", said(page)));
        assert!(
            at.x >= 0.0 && at.x + at.w <= 600.0,
            "{name} is inside the pane: {at:?}"
        );
    }
}

/// The header of a PNG that says it is `width` by `height` (all the program reads of a picture it will not
/// draw).
pub(crate) fn png_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_picture_that_is_not_drawn_is_an_image_card_that_says_why() {
    let base = "https://uploads.linear.app/ws/9a1f/";
    let (gif, wide, webp, gone) = (
        format!("{base}a.gif"),
        format!("{base}wide.png"),
        format!("{base}b.webp"),
        format!("{base}gone.png"),
    );
    let mut rig = open_with(
        WIDE,
        None,
        &format!(
            "The public API has no limit per API key.\n\n![animated]({gif})\n\n![wide one]({wide})\n\n![still]({webp})\n\n![lost]({gone})"
        ),
        "nothing",
    )
    .expect("the page opens");
    rig.linear.store(&gif, b"GIF89a....".to_vec());
    rig.linear.store(&wide, png_header(5120, 2880));
    rig.linear.lose(&gone);
    rig.linear.store(&webp, b"RIFF\0\0\0\0WEBPVP8 ".to_vec());
    open_the_issue(&mut rig.page);
    for n in 1..=4 {
        wait_bounds(&mut rig.page, &format!("file-card:{n}"));
    }
    let page = &rig.page;
    // Each is an image (never a "File"), with its reason; no card has none.
    for words in [
        "Image GIF is not shown here",
        "Image 5120\u{d7}2880 Too large to show here",
        "Image WebP is not shown here",
        "Image Could not be fetched",
    ] {
        assert!(page.says(words), "{words}: {}", said(page));
    }
    assert!(!page.says("File"), "{}", said(page));
    // One verb to a card: Save. The rest is the menu and the commands.
    assert_eq!(count(page, "Save"), 4, "{}", said(page));
    assert!(page.bounds("pic-link:1").is_none(), "{}", said(page));
}

/// Where the two buttons at the end of the head of the issue are.
fn head_of_the_issue(page: &Page) -> (pencere_view::Rect, pencere_view::Rect) {
    (
        page.bounds("open-linear").expect("Open in Linear"),
        page.bounds("panel-close").expect("Close"),
    )
}

/// The two are where they were: nothing of the head moved (Close is as wide as it was).
fn not_moved(
    was: (pencere_view::Rect, pencere_view::Rect),
    now: (pencere_view::Rect, pencere_view::Rect),
) {
    for (was, now) in [(was.0, now.0), (was.1, now.1)] {
        assert!(
            (was.x - now.x).abs() < 0.5
                && (was.y - now.y).abs() < 0.5
                && (was.w - now.w).abs() < 0.5
                && (was.h - now.h).abs() < 0.5,
            "moved: {was:?} {now:?}"
        );
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn esc_is_written_once_where_the_comment_or_the_description_tray_has_it_and_close_stays_put() {
    // The issue is open and the keyboard is in the list: Esc closes the issue, and Close says so.
    let mut rig = opened();
    let before = head_of_the_issue(&rig.page);
    assert_eq!(count(&rig.page, "Esc"), 1, "{}", said(&rig.page));
    // The keyboard is in the comment: the line of keys says Esc (leave), Close does not.
    paste_into_comment(&mut rig, "run.log", b"the words of a log");
    let page = &mut rig.page;
    wait_says(page, "run.log");
    page.advance(0.4).expect("a moment");
    assert!(page.says("leave"), "{}", said(page));
    assert_eq!(count(page, "Esc"), 1, "{}", said(page));
    not_moved(before, head_of_the_issue(page));

    // Files wait for the description: Cancel says Esc, Close does not.
    let mut rig = open_pictured(WIDE, None).expect("the page opens");
    open_the_issue(&mut rig.page);
    let page = &mut rig.page;
    let before = head_of_the_issue(page);
    assert_eq!(count(page, "Esc"), 1, "{}", said(page));
    wait_bounds(page, "attach-file:describe");
    read_to(page, "describe-zone");
    page.give_files(
        "describe-zone",
        FileSource::Drop,
        &[("one.png", &given(240))],
    )
    .expect("dropped");
    wait_says(page, "Add to description");
    page.advance(0.4).expect("a moment");
    assert_eq!(count(page, "Esc"), 1, "{}", said(page));
    assert!(page.says("Cancel"), "{}", said(page));
    not_moved(before, head_of_the_issue(page));
    // Cancel gives the key back to Close.
    page.press("escape").expect("escape");
    wait_gone(page, "describe-add");
    page.advance(0.4).expect("a moment");
    assert_eq!(count(page, "Esc"), 1, "{}", said(page));
    not_moved(before, head_of_the_issue(page));
}

/// The page with the issue open, that waits no longer than a moment for the program's answer: a step
/// that tells it something while an answer is owed comes back `Slow`, and the answer is still owed.
fn opened_in_a_hurry() -> Pictured {
    let mut rig = open_with_patience(
        WIDE,
        None,
        &description(),
        &format!("Here is what the customer sees: ![reply]({REPLY})"),
        Some(std::time::Duration::from_millis(1500)),
    )
    .expect("the page opens");
    open_the_issue(&mut rig.page);
    wait_bounds(&mut rig.page, "pic:3");
    rig.page.advance(0.4).expect("a moment");
    rig
}

/// What a step that told the page something came to while the program owed an answer: done, or too slow
/// to wait for it (what it told the page is done either way).
fn in_flight<T>(step: Result<T, Error>) {
    match step {
        Ok(_) | Err(Error::Slow { .. }) => {}
        Err(other) => panic!("{other:?}"),
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_file_added_while_the_others_are_being_sent_stays_in_the_tray_when_they_are_posted() {
    let bytes = given(200);
    // The comment: the first file is on its way (the storage holds it), and the second is dropped on the
    // comment then.
    let mut rig = opened_in_a_hurry();
    paste_into_comment(&mut rig, "first.png", &bytes);
    let page = &mut rig.page;
    wait_says(page, "first.png");
    page.advance(0.4).expect("a moment");
    page.wait_for(WITHIN, |page| {
        crate::page_tests::pressable(page, "comment-send")
    })
    .unwrap_or_else(|why| panic!("Comment can be pressed: {why:?}: {}", said(page)));
    rig.linear.hold_storage(true);
    // (The program owes the answer: the step may be slow, and what it told the page is done.)
    in_flight(page.click("comment-send"));
    assert!(
        rig.linear.put().len() == 1 && rig.linear.commented().is_empty(),
        "the first file is on its way and the comment is not posted"
    );
    read_to_the_end(page);
    in_flight(page.give_files("comment-box", FileSource::Drop, &[("second.png", &bytes)]));
    assert!(
        rig.linear.commented().is_empty(),
        "the second was dropped while the first was on its way"
    );
    rig.linear.hold_storage(false);
    page.wait_for(WITHIN, |_| !rig.linear.commented().is_empty())
        .unwrap_or_else(|why| panic!("the comment is posted: {why:?}: {}", said(page)));
    page.advance(0.8).expect("a moment");
    let body = rig.linear.commented()[0]["body"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        body.contains("![first.png]") && !body.contains("second.png"),
        "{body}"
    );
    // Only the first went: the second waits in the tray, for the next comment.
    assert_eq!(rig.linear.put().len(), 1);
    assert!(page.says("second.png"), "{}", said(page));
    assert_eq!(count(page, "Remove"), 1, "{}", said(page));

    // The description.
    let mut rig = opened_in_a_hurry();
    let page = &mut rig.page;
    wait_bounds(page, "attach-file:describe");
    read_to(page, "describe-zone");
    let one = page
        .give_files("describe-zone", FileSource::Drop, &[("one.png", &bytes)])
        .expect("dropped");
    rig.terminal.give(&one[0], &bytes);
    wait_says(page, "Add to description");
    page.advance(0.4).expect("a moment");
    rig.linear.hold_storage(true);
    in_flight(page.click("describe-add"));
    assert!(
        rig.linear.put().len() == 1 && rig.linear.changed().is_empty(),
        "the first file is on its way and the description is not written"
    );
    in_flight(page.give_files("describe-zone", FileSource::Drop, &[("two.png", &bytes)]));
    assert!(
        rig.linear.changed().is_empty(),
        "the second was dropped while the first was on its way"
    );
    rig.linear.hold_storage(false);
    page.wait_for(WITHIN, |_| !rig.linear.changed().is_empty())
        .unwrap_or_else(|why| panic!("the description is written: {why:?}: {}", said(page)));
    page.advance(0.8).expect("a moment");
    let written = rig.linear.changed()[0]["input"]["description"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        written.contains("![one.png]") && !written.contains("two.png"),
        "{written}"
    );
    assert!(
        page.says("two.png") && page.says("Add to description"),
        "{}",
        said(page)
    );
    assert_eq!(count(page, "Remove"), 1, "{}", said(page));
}

/// Files wait for the description, and a field has the keyboard: ⌘Enter is the field's, and nothing is
/// written to the description.
fn nothing_is_written(rig: &mut Pictured, why: &str) {
    rig.page.press("mod+enter").expect("command and enter");
    rig.page.advance(0.6).expect("a moment");
    assert!(
        rig.linear.changed().is_empty()
            && rig.linear.put().is_empty()
            && rig.linear.commented().is_empty(),
        "{why}: something was written: {}",
        said(&rig.page)
    );
    assert!(
        rig.page.says("Add to description"),
        "{why}: the tray is still there: {}",
        said(&rig.page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn command_enter_in_the_comment_or_the_head_field_does_not_write_the_description_even_when_it_is_empty()
 {
    for field in ["comment", "head"] {
        let mut rig = open_pictured(WIDE, None).expect("the page opens");
        open_the_issue(&mut rig.page);
        let page = &mut rig.page;
        wait_bounds(page, "attach-file:describe");
        read_to(page, "describe-zone");
        let bytes = given(240);
        let handles = page
            .give_files("describe-zone", FileSource::Drop, &[("one.png", &bytes)])
            .expect("dropped");
        rig.terminal.give(&handles[0], &bytes);
        let page = &mut rig.page;
        wait_says(page, "Add to description");
        page.advance(0.4).expect("a moment");
        // The files are read and can be added: it is the keyboard that says they are not.
        assert!(page.bounds("describe-add").is_some(), "{}", said(page));
        if field == "comment" {
            page.press("shift+m").expect("shift and m");
            page.wait_for(WITHIN, |page| typing_in(page, &comment_editor("issue-412")))
                .unwrap_or_else(|why| panic!("the comment has the keyboard: {why:?}"));
        } else {
            page.press("/").expect("slash");
            page.advance(0.3).expect("a moment");
            assert!(
                matches!(page.focus(), pencere_drive::headless::Focus::Field(_)),
                "the head's field has the keyboard: {:?}",
                page.focus()
            );
        }
        // (Both fields are empty.)
        nothing_is_written(&mut rig, field);
    }
}

/// Three files of names of very different lengths are given to the description of the issue.
fn give_three(page: &mut Page) {
    let bytes = given(240);
    page.press("enter").expect("enter");
    wait_bounds(page, "attach-file:describe");
    page.wheel("panel-scroll", 0.0, 120.0).expect("read on");
    page.advance(0.4).expect("a moment");
    read_to(page, "describe-zone");
    page.give_files(
        "describe-zone",
        FileSource::Drop,
        &[
            ("one.png", &bytes),
            (
                "two-with-a-name-that-is-very-much-too-long-for-a-small-card.png",
                &bytes,
            ),
            ("three.log", &bytes),
        ],
    )
    .expect("dropped");
    wait_says(page, "Add to description");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_files_of_a_tray_are_all_of_one_width_and_a_narrow_pane_gives_each_a_row() {
    // Where there is room each is as wide as the others, whatever its name; the whole name is the tip.
    let mut rig = open_pictured(crate::page_tests::PANE, None).expect("the page opens");
    give_three(&mut rig.page);
    let page = &rig.page;
    let widths: Vec<f32> = (1..=3)
        .map(|n| {
            page.bounds(&format!("tray-file:{n}"))
                .unwrap_or_else(|| panic!("file {n} is drawn: {}", said(page)))
                .w
        })
        .collect();
    assert!(widths.iter().all(|w| (w - 240.0).abs() < 1.0), "{widths:?}");
    // A narrow pane gives each the whole row.
    let mut rig = open_pictured((480.0, 640.0), None).expect("the page opens");
    give_three(&mut rig.page);
    let page = &rig.page;
    let widths: Vec<f32> = (1..=3)
        .map(|n| {
            page.bounds(&format!("tray-file:{n}"))
                .unwrap_or_else(|| panic!("file {n} is drawn: {}", said(page)))
                .w
        })
        .collect();
    assert!(
        widths.iter().all(|w| (w - widths[0]).abs() < 1.0)
            && widths[0] > 300.0
            && widths[0] <= 480.0,
        "{widths:?}"
    );
}

#[allow(dead_code)]
fn unused(_: Error) {}
