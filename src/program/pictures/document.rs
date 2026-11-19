//! A PDF as the program reads it: how many pages it has and how large each is, the text on a page
//! and where every letter of it stands, and a search through the text of all its pages. The terminal
//! draws the pages (the sandbox of pencere does, from the file the program gave it); this is what the
//! page cannot ask of a drawing: where the words are, to select them and to mark what was found.
//!
//! The text is what the interpreter of `hayro` says it would draw: each letter with the matrix it
//! is drawn by. A PDF has no lines and no words, only letters put at places, so the lines are made
//! here: letters on one baseline, in order of their places and not of the file's, with a space
//! where the ink leaves a gap that a word has. Text that is not set along the page (turned a
//! quarter, or upside down) is left out, and so is a letter the font says no character of. Places
//! are points from the top left corner of the page as it is shown, which is what the terminal's
//! sizes of a page are (`Rotate` applied).
//!
//! The pages are the ones `hayro` counts, which is the terminal's list of pages too (its sandbox
//! reads the file with the same reader), so that page `n` is the same page here and there. A page
//! with no `/Contents` is a page of the list, blank, and keeps its number; only a file whose page
//! tree cannot be read, so that the reader looks for its pages one by one, loses such pages from
//! the list, in both places alike.
//!
//! Plain values in and out: this file knows nothing of the page. (Adapted from textpen's
//! `project/pdf.rs`: the same reading, with a search of words as they are written and no pattern.)

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use hayro_interpret::font::{Glyph, GlyphRun, PositionedGlyph};
use hayro_interpret::hayro_cmap::BfString;
use hayro_interpret::hayro_syntax::Pdf;
use hayro_interpret::kurbo::{Affine, BezPath, Point, Rect, Shape as _};
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, TransformExt as _, interpret_page,
};
use serde::Serialize;

/// The most matches a search gives: past this it stops and says so.
pub const MATCH_LIMIT: usize = 2000;

/// The most characters of what is looked for.
pub const QUERY_MOST: usize = 200;

/// What a search looks for: the words, as they are written (no pattern), and whether capitals count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub case_sensitive: bool,
}

/// A PDF with more pages than this is not shown: the sizes of its pages are told in one message,
/// and a view that reads them all has to find room for them.
pub const PAGE_LIMIT: usize = 10_000;

/// How long a search through the pages goes on: a book is read in seconds, and a file made to
/// take longer is stopped and says so.
const TIME_LIMIT: Duration = Duration::from_secs(30);

/// How long the first look for any text at all goes on, over the pages in turn. A PDF of
/// pictures has none; past this it is taken to have some and the search finds out.
const TEXT_LOOK: Duration = Duration::from_secs(1);

/// How long the reading of one page goes on, and how many letters of it are taken: a page made to
/// have no end (a content that draws a million letters, or a loop of its own) is read as far as these
/// let it, and what was read is its text.
const PAGE_TIME: Duration = Duration::from_secs(5);
const GLYPH_MOST: usize = 200_000;

/// The interpreter says what it draws a great many times: the time is looked at once in this many.
const LOOK_EVERY: usize = 256;

/// The most characters of a line. A row of more is several lines: a message carries a line whole, and
/// one line is never more than a message has room for.
pub const LINE_MOST: usize = 4096;

/// How many pages' text are kept for the page to ask again.
const KEPT: usize = 8;

/// The widest or tallest a page is taken to be, points: the terminal's rule (pencere's sandbox),
/// so that the sizes told here are the sizes it draws.
const MOST_NATURAL: f32 = 16_384.0;

/// A baseline further than this share of the letters' size from the line's is another line.
const SAME_LINE: f32 = 0.4;

/// A gap between two letters' ink, as a share of their size, that a word's space has.
const WORD_GAP: f32 = 0.15;

/// A gap past this many sizes breaks a line in two: a column, a cell of a table.
const COLUMN_GAP: f32 = 3.0;

/// How the terminal-side height of a line's box is taken from the size of its letters: above the
/// baseline and below it, as shares of the size.
const ABOVE: f32 = 0.82;
const BELOW: f32 = 0.25;

/// Why a file is not a PDF the program can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unopened {
    /// It is not a PDF that can be read: broken, encrypted, or with no page.
    Unreadable,
    /// It has more pages than [`PAGE_LIMIT`].
    TooManyPages,
}

/// A line of text on a page. `x` and `y` are the left and the top of its box, `w` and `h` its
/// size; `xs` are the places across the line of each character's left edge, from `x`, and then the
/// right edge of the last: one more than the line has characters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub text: String,
    pub xs: Vec<f64>,
}

/// The text of a page, its lines from the top down and from the left.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageText {
    pub lines: Vec<Line>,
}

/// A match in the text of a page: its box, points from the page's top left corner.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMatch {
    /// From 1.
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What a search through a PDF found.
#[derive(Debug, Serialize)]
pub struct Found {
    pub matches: Vec<PageMatch>,
    /// How many pages were read.
    pub searched: usize,
    /// Stopped at the most matches, at the time it may take, or by the page.
    pub truncated: bool,
    pub ms: u64,
}

/// A PDF, opened.
pub struct Document {
    pdf: Pdf,
    /// The own size of each page, as the terminal draws it.
    sizes: Vec<(f64, f64)>,
    /// The text of the pages asked for last.
    kept: Mutex<Vec<(usize, Arc<PageText>)>>,
}

impl Document {
    /// The PDF in `bytes`.
    ///
    /// # Errors
    ///
    /// Why it is not one the program shows.
    pub fn open(bytes: Arc<[u8]>) -> Result<Self, Unopened> {
        let read = catch_unwind(AssertUnwindSafe(|| {
            let pdf = Pdf::new(Arc::new(bytes)).ok()?;
            let sizes: Vec<(f64, f64)> = pdf
                .pages()
                .iter()
                .map(|page| {
                    let (width, height) = page.render_dimensions();
                    own_size(width, height)
                })
                .collect();
            Some((pdf, sizes))
        }));
        let Ok(Some((pdf, sizes))) = read else {
            return Err(Unopened::Unreadable);
        };
        if sizes.is_empty() {
            return Err(Unopened::Unreadable);
        }
        if sizes.len() > PAGE_LIMIT {
            return Err(Unopened::TooManyPages);
        }
        Ok(Self {
            pdf,
            sizes,
            kept: Mutex::new(Vec::new()),
        })
    }

    /// The own size of each page, points.
    pub fn sizes(&self) -> &[(f64, f64)] {
        &self.sizes
    }

    /// The pages' sizes as runs of pages of one size: how many, the width, the height. A book is
    /// a few runs; the most a PDF of [`PAGE_LIMIT`] pages of different sizes comes to still fits a
    /// message.
    pub fn runs(&self) -> Vec<(usize, f64, f64)> {
        let mut runs: Vec<(usize, f64, f64)> = Vec::new();
        for &(width, height) in &self.sizes {
            match runs.last_mut() {
                Some(run) if (run.1, run.2) == (width, height) => run.0 += 1,
                _ => runs.push((1, width, height)),
            }
        }
        runs
    }

    /// The text of the page at `index` (from 0; none for a page the PDF has not). A page whose
    /// content cannot be read has none; one that goes on too long is read as far as it went.
    pub fn text(&self, index: usize) -> Option<Arc<PageText>> {
        if index >= self.sizes.len() {
            return None;
        }
        if let Some(kept) = self.kept_text(index) {
            return Some(kept);
        }
        let (text, _) = self.read_text(index, PAGE_TIME);
        let text = Arc::new(text);
        self.keep(index, &text);
        Some(text)
    }

    fn keep(&self, index: usize, text: &Arc<PageText>) {
        let mut kept = lock(&self.kept);
        kept.retain(|(at, _)| *at != index);
        if kept.len() >= KEPT {
            kept.remove(0);
        }
        kept.push((index, Arc::clone(text)));
    }

    fn kept_text(&self, index: usize) -> Option<Arc<PageText>> {
        lock(&self.kept)
            .iter()
            .find(|(at, _)| *at == index)
            .map(|(_, text)| Arc::clone(text))
    }

    /// The text of the page at `index` read from its content, kept nowhere, for no longer than
    /// `within` and no more than [`GLYPH_MOST`] letters: the text, and whether the page was read to its
    /// end.
    fn read_text(&self, index: usize, within: Duration) -> (PageText, bool) {
        let mut letters = Letters::until(Instant::now() + within);
        let read = catch_unwind(AssertUnwindSafe(|| {
            let page = self.pdf.pages().get(index)?;
            let (width, height) = page.render_dimensions();
            let cache = InterpreterCache::new();
            let mut context = Context::new(
                page.initial_transform(true).to_kurbo(),
                Rect::new(0.0, 0.0, f64::from(width), f64::from(height)),
                &cache,
                page.xref(),
                InterpreterSettings::default(),
            );
            interpret_page(page, &mut context, &mut letters);
            Some(())
        }));
        let (whole, cut) = match &read {
            Ok(done) => (done.is_some(), false),
            Err(payload) => (false, payload.is::<Cut>()),
        };
        if whole || cut {
            (
                PageText {
                    lines: assemble(letters.glyphs),
                },
                whole,
            )
        } else {
            (PageText::default(), false)
        }
    }

    /// Whether any page has text, looked for page by page for as long as [`TEXT_LOOK`]: a PDF
    /// whose pages are pictures has none, and one the look did not get through is taken to have.
    pub fn has_text(&self) -> bool {
        let started = Instant::now();
        for index in 0..self.sizes.len() {
            let left = TEXT_LOOK.saturating_sub(started.elapsed());
            if left.is_zero() {
                return true;
            }
            let text = match self.kept_text(index) {
                Some(kept) => kept,
                None => {
                    let (read, whole) = self.read_text(index, left.min(PAGE_TIME));
                    if !whole && read.lines.is_empty() && started.elapsed() >= TEXT_LOOK {
                        // The page was cut by the look, with nothing yet: it is taken to have some.
                        return true;
                    }
                    let read = Arc::new(read);
                    if whole {
                        self.keep(index, &read);
                    }
                    read
                }
            };
            if !text.lines.is_empty() {
                return true;
            }
        }
        false
    }

    /// The matches of `query` in the text of the pages, until `stop` says to stop, the matches are
    /// [`MATCH_LIMIT`] or the search has gone on for [`TIME_LIMIT`]. A match is within a line.
    pub fn find(&self, query: &Query, stop: &dyn Fn() -> bool) -> Found {
        let started = Instant::now();
        let needle: Vec<char> = query
            .text
            .chars()
            .map(|ch| fold(ch, query.case_sensitive))
            .collect();
        if needle.is_empty() {
            return Found {
                matches: Vec::new(),
                searched: 0,
                truncated: false,
                ms: 0,
            };
        }
        let mut matches = Vec::new();
        let mut searched = 0;
        let mut truncated = false;
        for index in 0..self.sizes.len() {
            if stop() || started.elapsed() > TIME_LIMIT {
                truncated = true;
                break;
            }
            // Read for the search and not kept: a search through a book would push out the pages
            // the person is reading.
            let text = self.kept_text(index).unwrap_or_else(|| {
                let within = PAGE_TIME.min(TIME_LIMIT.saturating_sub(started.elapsed()));
                Arc::new(self.read_text(index, within).0)
            });
            searched += 1;
            for line in &text.lines {
                if !find_in_line(
                    line,
                    &needle,
                    query.case_sensitive,
                    index + 1,
                    &mut matches,
                    MATCH_LIMIT,
                ) {
                    truncated = true;
                    break;
                }
            }
            if truncated {
                break;
            }
        }
        Found {
            matches,
            searched,
            truncated,
            ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A page's own size as the terminal draws it, to hundredths of a point: nothing under a point, and
/// not past what the sandbox of pencere takes a page to be.
fn own_size(width: f32, height: f32) -> (f64, f64) {
    let (width, height) = (width.max(1.0), height.max(1.0));
    let shrink = (MOST_NATURAL / width.max(height)).min(1.0);
    (hundredths(width * shrink), hundredths(height * shrink))
}

/// `value` to hundredths, as the number of the nearest decimal: a size of a page is told as the
/// `612.28` it is and not as the `612.280029296875` that the same number in a `f32` is.
fn hundredths(value: f32) -> f64 {
    let value = f64::from(value);
    if value.is_finite() {
        (value * 100.0).round() / 100.0
    } else {
        0.0
    }
}

/// The same for a number that is already as wide as it is told.
fn hundredths_of(value: f64) -> f64 {
    if value.is_finite() {
        (value * 100.0).round() / 100.0
    } else {
        0.0
    }
}

/// A character as it is looked for: capitals folded one character at a time (a character whose lower
/// case is more than one stays as it is), so that the places of the characters are the line's.
fn fold(ch: char, case_sensitive: bool) -> char {
    if case_sensitive {
        return ch;
    }
    let mut lower = ch.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => ch,
    }
}

/// The matches of `needle` (already folded) in `line`, pushed on `matches` as boxes on page `page`, until
/// they are `limit`; whether there is room for more.
fn find_in_line(
    line: &Line,
    needle: &[char],
    case_sensitive: bool,
    page: usize,
    matches: &mut Vec<PageMatch>,
    limit: usize,
) -> bool {
    let chars: Vec<char> = line
        .text
        .chars()
        .map(|ch| fold(ch, case_sensitive))
        .collect();
    let mut at = 0;
    while at + needle.len() <= chars.len() {
        if chars[at..at + needle.len()] != *needle {
            at += 1;
            continue;
        }
        if matches.len() >= limit {
            return false;
        }
        let left = line.xs.get(at).copied().unwrap_or(0.0);
        let right = line
            .xs
            .get(at + needle.len())
            .copied()
            .unwrap_or(line.w)
            .max(left);
        matches.push(PageMatch {
            page,
            x: hundredths_of(line.x + left),
            y: line.y,
            w: hundredths_of(right - left),
            h: line.h,
        });
        at += needle.len();
    }
    true
}

/// A letter as the interpreter draws it: the character, where its origin stands, the size of the
/// letters of its font as set, and where its ink begins and ends across.
#[derive(Debug, Clone, Copy)]
struct Glyph1 {
    ch: char,
    x: f32,
    /// The baseline.
    y: f32,
    size: f32,
    left: f32,
    right: f32,
}

/// What the interpreter is stopped with when the page has gone on too long or has too many letters: it
/// is the unwinding that ends the interpreter (`resume_unwind` makes no noise), and what was read
/// before is the page's text.
struct Cut;

/// What the interpreter says it draws, of the letters; it draws nothing else.
struct Letters {
    glyphs: Vec<Glyph1>,
    /// The ink of a glyph of a font, across, in thousandths of its size: the same glyph is drawn
    /// again and again.
    ink: HashMap<u128, (f32, f32)>,
    /// When the reading of the page is stopped, and how many things the interpreter has drawn.
    until: Instant,
    drawn: usize,
}

impl Letters {
    fn until(until: Instant) -> Self {
        Self {
            glyphs: Vec::new(),
            ink: HashMap::new(),
            until,
            drawn: 0,
        }
    }

    /// One more thing drawn: the time is looked at every `LOOK_EVERY`, and a page that is out of time
    /// is stopped.
    fn tick(&mut self) {
        self.drawn += 1;
        if self.drawn.is_multiple_of(LOOK_EVERY) && Instant::now() >= self.until {
            std::panic::resume_unwind(Box::new(Cut));
        }
    }

    /// The letters of one glyph of a run: where it is drawn, by the run's transform.
    fn push(&mut self, positioned: &PositionedGlyph<'_>, transform: Affine) {
        self.tick();
        if self.glyphs.len() >= GLYPH_MOST {
            std::panic::resume_unwind(Box::new(Cut));
        }
        let glyph: &Glyph<'_> = positioned;
        let characters = match glyph.as_unicode() {
            Some(BfString::Char(ch)) => vec![ch],
            Some(BfString::String(text)) => text.chars().collect(),
            None => return,
        };
        if characters.is_empty() {
            return;
        }
        // The outline is in thousandths of the font's size: the matrix takes one thousand along
        // the glyph's x axis to the size on the page, which is how far the letter is set.
        let matrix = transform * positioned.transform();
        let origin = matrix * Point::new(0.0, 0.0);
        let along = matrix * Point::new(1000.0, 0.0);
        let (dx, dy) = (along.x - origin.x, along.y - origin.y);
        let size = dx.hypot(dy) as f32;
        // Set along the page, to the right: other text has no place in a line.
        let along =
            dx.is_finite() && dy.is_finite() && dx > 0.0 && dy.abs() <= f64::from(size) * 0.35;
        if !along || !size.is_finite() || size <= 0.0 {
            return;
        }
        let x = origin.x as f32;
        let (ink_left, ink_right) = match glyph {
            Glyph::Outline(outline) => *self
                .ink
                .entry(outline.identifier().cache_key())
                .or_insert_with(|| {
                    let bounds = outline.outline().bounding_box();
                    if bounds.x1 > bounds.x0 {
                        ((bounds.x0 / 1000.0) as f32, (bounds.x1 / 1000.0) as f32)
                    } else {
                        (0.0, 0.0)
                    }
                }),
            Glyph::Type3(_) => (0.0, 0.5),
        };
        let left = x + ink_left * size;
        let right = x + ink_right.max(ink_left + 0.05) * size;
        // A glyph that the font reads as several characters (a ligature the font's own map spells
        // out) shares its place among them, as the ligatures of `expansion` do.
        let count = characters.len() as f32;
        for (at, ch) in characters.into_iter().enumerate() {
            let (from, to) = (at as f32 / count, (at + 1) as f32 / count);
            self.glyphs.push(Glyph1 {
                ch,
                x: x + (right - x) * from,
                y: origin.y as f32,
                size,
                left: left + (right - left) * from,
                right: left + (right - left) * to,
            });
        }
    }
}

impl Device<'_> for Letters {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'_>, _: &DrawMode) {
        self.tick();
    }

    fn push_clip_path(&mut self, _: &ClipPath) {
        self.tick();
    }

    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {
        self.tick();
    }

    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, '_>, props: DrawProps<'_>, _: &DrawMode) {
        for positioned in run.glyphs() {
            self.push(positioned, props.transform);
        }
    }

    fn draw_image(&mut self, _: Image<'_, '_>, _: ImageDrawProps<'_>) {
        self.tick();
    }

    fn pop_clip(&mut self) {
        self.tick();
    }

    fn pop_transparency_group(&mut self) {
        self.tick();
    }
}

/// The characters a letter is, for the ones that are more than one when read: the ligatures.
fn expansion(ch: char) -> Option<&'static str> {
    Some(match ch {
        '\u{FB00}' => "ff",
        '\u{FB01}' => "fi",
        '\u{FB02}' => "fl",
        '\u{FB03}' => "ffi",
        '\u{FB04}' => "ffl",
        '\u{FB05}' | '\u{FB06}' => "st",
        _ => return None,
    })
}

/// The lines of the letters of a page: the letters of one baseline together, in order across,
/// and a line where the letters stop for a column's width.
fn assemble(mut glyphs: Vec<Glyph1>) -> Vec<Line> {
    glyphs.retain(|glyph| {
        glyph.x.is_finite() && glyph.y.is_finite() && !glyph.ch.is_control() && glyph.ch != '\u{AD}'
    });
    glyphs.sort_by(|a, b| a.y.total_cmp(&b.y));
    // The baselines: letters whose baseline is within a share of the size of the first of a group.
    let mut rows: Vec<(f32, Vec<Glyph1>)> = Vec::new();
    for glyph in glyphs {
        match rows.last_mut() {
            Some((base, row)) if (glyph.y - *base).abs() <= SAME_LINE * glyph.size => {
                row.push(glyph);
            }
            _ => rows.push((glyph.y, vec![glyph])),
        }
    }
    let mut lines = Vec::new();
    for (_, mut row) in rows {
        row.sort_by(|a, b| a.x.total_cmp(&b.x));
        lines.extend(row_lines(&row));
    }
    // Top down by the baseline; the lines of one row, from the left.
    let baseline = |line: &Line| line.y + line.h * f64::from(ABOVE / (ABOVE + BELOW));
    lines.sort_by(|a, b| {
        baseline(a)
            .total_cmp(&baseline(b))
            .then(a.x.total_cmp(&b.x))
    });
    lines
}

/// The lines of the letters of one row, from the left.
fn row_lines(row: &[Glyph1]) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut current: Option<Builder> = None;
    let mut spaced = false;
    let mut last: Option<Glyph1> = None;
    for &glyph in row {
        if glyph.ch.is_whitespace() {
            spaced = true;
            continue;
        }
        if let Some(before) = last {
            // The same letter drawn again beside itself is the heavy type of a font that has none.
            if before.ch == glyph.ch && (glyph.x - before.x).abs() < before.size * 0.08 {
                continue;
            }
            let gap = glyph.left - before.right;
            let size = glyph.size.min(before.size);
            if gap > COLUMN_GAP * size {
                if let Some(done) = current.take() {
                    lines.push(done.finish());
                }
            } else if (spaced || gap > WORD_GAP * size)
                && let Some(builder) = current.as_mut()
            {
                builder.push_space(before.right.min(glyph.x));
            }
        }
        spaced = false;
        if current
            .as_ref()
            .is_some_and(|builder| builder.edges.len() >= LINE_MOST)
            && let Some(done) = current.take()
        {
            lines.push(done.finish());
        }
        current
            .get_or_insert_with(Builder::default)
            .push_letter(&glyph);
        last = Some(glyph);
    }
    if let Some(done) = current {
        lines.push(done.finish());
    }
    lines
}

/// A line being made of letters, in the page's own places.
#[derive(Default)]
struct Builder {
    text: String,
    /// The left edge of each character, in the page's places.
    edges: Vec<f32>,
    /// The right edge of the last letter.
    end: f32,
    baseline: f32,
    size: f32,
}

impl Builder {
    fn push_space(&mut self, at: f32) {
        self.text.push(' ');
        self.edges.push(at);
    }

    fn push_letter(&mut self, glyph: &Glyph1) {
        self.baseline = glyph.y;
        self.size = self.size.max(glyph.size);
        let end = glyph.right.max(glyph.x);
        match expansion(glyph.ch) {
            Some(letters) => {
                let count = letters.chars().count().max(1) as f32;
                for (at, letter) in letters.chars().enumerate() {
                    self.text.push(letter);
                    self.edges
                        .push(glyph.x + (end - glyph.x) * at as f32 / count);
                }
            }
            None => {
                self.text.push(glyph.ch);
                self.edges.push(glyph.x);
            }
        }
        self.end = end;
    }

    fn finish(self) -> Line {
        let x = self.edges.first().copied().unwrap_or(0.0);
        // Never backwards: letters set over each other (kerning) share an edge.
        let mut xs = Vec::with_capacity(self.edges.len() + 1);
        let mut across = 0.0_f32;
        for edge in &self.edges {
            across = across.max(edge - x);
            xs.push(hundredths(across));
        }
        across = across.max(self.end - x);
        xs.push(hundredths(across));
        Line {
            x: hundredths(x),
            y: hundredths(self.baseline - ABOVE * self.size),
            w: hundredths(across),
            h: hundredths((ABOVE + BELOW) * self.size),
            text: self.text,
            xs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pictures::double;

    fn pages() -> Document {
        Document::open(Arc::from(double::pdf(3))).expect("the test PDF opens")
    }

    fn query(text: &str) -> Query {
        Query {
            text: text.to_owned(),
            case_sensitive: false,
        }
    }

    /// A PDF of one page of `width` by `height` points (`extra` is more of its dictionary) with
    /// `content`, Helvetica as `F1`.
    fn one_page(width: u32, height: u32, extra: &str, content: &str) -> Document {
        let pdf = format!(
            "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] {extra} \
             /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n\
             4 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n\
             5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n\
             trailer\n<< /Size 6 /Root 1 0 R >>\n%%EOF\n",
            content.len()
        );
        Document::open(Arc::from(pdf.as_bytes())).expect("it opens")
    }

    #[test]
    fn the_pages_have_the_sizes_the_file_says() {
        let document = pages();
        assert_eq!(document.sizes(), [(420.0, 560.0); 3]);
        // The same size is one run.
        assert_eq!(document.runs(), [(3, 420.0, 560.0)]);
    }

    #[test]
    fn a_page_with_no_content_is_a_page_of_the_list_and_keeps_its_number() {
        let pdf = "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
             2 0 obj\n<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>\nendobj\n\
             3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n\
             4 0 obj\n<< /Length 21 >>\nstream\n1 0 0 rg 0 0 50 50 re f\nendstream\nendobj\n\
             5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>\n\
             endobj\ntrailer\n<< /Size 6 /Root 1 0 R >>\n%%EOF\n";
        let document = Document::open(Arc::from(pdf.as_bytes())).expect("it opens");
        assert_eq!(document.sizes(), [(100.0, 100.0), (200.0, 100.0)]);
        assert_eq!(document.runs(), [(1, 100.0, 100.0), (1, 200.0, 100.0)]);
        assert!(!document.has_text());
    }

    #[test]
    fn words_that_are_not_drawn_are_still_words_of_the_page() {
        // Text that is invisible (mode 3: the layer a scan's recognition leaves) or only a clip
        // (mode 7) is no ink, but a person selects and searches it all the same.
        let document = one_page(
            200,
            100,
            "",
            "BT /F1 12 Tf 10 80 Td (Seen) Tj ET\n\
             BT /F1 12 Tf 3 Tr 10 60 Td (Hidden) Tj ET\n\
             BT /F1 12 Tf 7 Tr 10 40 Td (Clipped) Tj ET\n",
        );
        let text = document.text(0).expect("a page");
        let said: Vec<&str> = text.lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(said, ["Seen", "Hidden", "Clipped"]);
    }

    #[test]
    fn the_text_of_a_page_is_its_lines_with_the_place_of_every_letter() {
        let document = pages();
        let text = document.text(0).expect("a page");
        let said: Vec<&str> = text.lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(said, ["Page 1", "The text of the page"]);
        let first = &text.lines[0];
        // Set at (40, 360) from the bottom left of a page 560 high: 200 points from the top, and the box
        // is around the line.
        assert!((first.x - 40.0).abs() < 0.5, "{first:?}");
        assert!(first.y < 200.0 && first.y + first.h > 200.0, "{first:?}");
        assert_eq!(first.xs.len(), first.text.chars().count() + 1);
        assert!(
            first.xs.windows(2).all(|pair| pair[0] <= pair[1]),
            "{first:?}"
        );
        assert_eq!(first.xs[0], 0.0);
        assert_eq!(first.xs.last().copied(), Some(first.w));
        // The second line is below the first.
        assert!(text.lines[1].y > first.y);
        // A space is a place of its own, between the words.
        let space = first.text.find(' ').expect("two words");
        assert!(
            first.xs[space] > 0.0 && first.xs[space + 1] > first.xs[space],
            "{first:?}"
        );
        // A page that is not there has no text.
        assert!(document.text(3).is_none());
    }

    #[test]
    fn a_word_is_found_on_the_pages_it_is_on_where_it_is() {
        let document = pages();
        let found = document.find(&query("PAGE 2"), &|| false);
        assert_eq!(found.searched, 3);
        assert!(!found.truncated);
        // Not case sensitive: "Page 2" is one, and only on the second page.
        assert_eq!(found.matches.len(), 1, "{found:?}");
        let one = &found.matches[0];
        assert_eq!(one.page, 2);
        assert!((one.x - 40.0).abs() < 1.0, "{one:?}");
        assert!(one.y < 200.0 && one.y + one.h > 200.0, "{one:?}");
        assert!(one.w > 20.0 && one.h > 5.0, "{one:?}");
        // "page" is a word of every page's second line, and of every page's first.
        let found = document.find(&query("page"), &|| false);
        let pages: Vec<usize> = found.matches.iter().map(|found| found.page).collect();
        assert_eq!(pages, [1, 1, 2, 2, 3, 3]);
        for found in &found.matches {
            assert!(found.x >= 0.0 && found.x + found.w <= 420.0, "{found:?}");
            assert!(found.y >= 0.0 && found.y + found.h <= 560.0, "{found:?}");
        }
        // Capitals count where asked.
        let exact = Query {
            case_sensitive: true,
            ..query("page")
        };
        assert_eq!(document.find(&exact, &|| false).matches.len(), 3);
        assert!(document.find(&query(""), &|| false).matches.is_empty());
        assert!(
            document
                .find(&query("nothing of the sort"), &|| false)
                .matches
                .is_empty()
        );
        assert!(document.has_text());
    }

    #[test]
    fn a_search_that_is_given_up_stops_and_says_so() {
        let document = pages();
        let found = document.find(&query("page"), &|| true);
        assert!(found.truncated);
        assert_eq!(found.searched, 0);
        assert!(found.matches.is_empty());
    }

    #[test]
    fn a_page_turned_a_quarter_has_its_words_where_they_are_shown() {
        // Words set up a page 200 by 300 and the page turned a quarter (`Rotate 90`): it is shown 300 by
        // 200 and the words read across it.
        let document = one_page(
            200,
            300,
            "/Rotate 90",
            "BT /F1 12 Tf 0 1 -1 0 50 100 Tm (Turned page) Tj ET\n",
        );
        assert_eq!(document.sizes(), [(300.0, 200.0)]);
        let text = document.text(0).expect("a page");
        assert_eq!(text.lines.len(), 1, "{text:?}");
        assert_eq!(text.lines[0].text, "Turned page");
        let line = &text.lines[0];
        assert!(line.w > 30.0 && line.x + line.w < 300.0, "{line:?}");
        // The words run up the unturned page, so they are the page's own and none of them is left out.
        let flat = one_page(
            200,
            300,
            "",
            "BT /F1 12 Tf 0 1 -1 0 50 100 Tm (Turned page) Tj ET\n",
        );
        assert!(flat.text(0).expect("a page").lines.is_empty());
    }

    #[test]
    fn a_page_of_a_great_many_letters_is_read_up_to_the_most_and_in_lines_of_a_size_a_message_holds()
     {
        // Two hundred and fifty thousand letters in one row: past the most, the rest is not read, and
        // what is read is lines of at most `LINE_MOST` characters (and a little over, for a ligature).
        let row = "a".repeat(250_000);
        let document = one_page(
            600,
            800,
            "",
            &format!("BT /F1 1 Tf 10 400 Td ({row}) Tj ET\n"),
        );
        let text = document.text(0).expect("a page");
        let letters: usize = text
            .lines
            .iter()
            .map(|line| line.text.chars().count())
            .sum();
        assert_eq!(letters, GLYPH_MOST, "{} lines", text.lines.len());
        for line in &text.lines {
            assert!(
                line.text.chars().count() <= LINE_MOST + 3,
                "{}",
                line.text.len()
            );
            assert_eq!(line.xs.len(), line.text.chars().count() + 1);
        }
        assert!(text.lines.len() >= GLYPH_MOST / LINE_MOST);
        // A row that is fewer is still one line, whole.
        let short = one_page(600, 800, "", "BT /F1 12 Tf 10 400 Td (Short row) Tj ET\n");
        assert_eq!(short.text(0).expect("a page").lines.len(), 1);
    }

    #[test]
    fn a_page_that_is_out_of_time_is_cut_inside_the_page_and_gives_what_it_had() {
        let row = "a".repeat(20_000);
        let document = one_page(
            600,
            800,
            "",
            &format!("BT /F1 1 Tf 10 400 Td ({row}) Tj ET\n"),
        );
        // No time at all: the time is looked at once in `LOOK_EVERY` letters, and the page is cut there.
        let (cut, whole) = document.read_text(0, Duration::ZERO);
        let letters: usize = cut.lines.iter().map(|line| line.text.chars().count()).sum();
        assert!(!whole);
        assert!(letters > 0 && letters <= 2 * LOOK_EVERY, "{letters}");
        // With the time it is read whole.
        let (all, whole) = document.read_text(0, PAGE_TIME);
        let letters: usize = all.lines.iter().map(|line| line.text.chars().count()).sum();
        assert!(whole);
        assert_eq!(letters, 20_000);
        // The look for any text at all is cut as well, and takes the page to have some.
        assert!(document.has_text());
    }

    #[test]
    fn what_is_not_a_pdf_is_not_opened() {
        assert_eq!(
            Document::open(Arc::from(&b"%PDF-1.4\nnot really"[..])).err(),
            Some(Unopened::Unreadable)
        );
        assert_eq!(
            Document::open(Arc::from(&b""[..])).err(),
            Some(Unopened::Unreadable)
        );
        // Cut anywhere, it is opened or it is not, and what it says of its text is a text.
        let bytes = double::pdf(3);
        for cut in (0..bytes.len()).step_by(17) {
            if let Ok(document) = Document::open(Arc::from(&bytes[..cut])) {
                for index in 0..document.sizes().len() {
                    let _ = document.text(index);
                }
            }
        }
    }

    #[test]
    fn a_ligature_is_the_letters_it_stands_for() {
        let glyph = |ch: char, x: f32| Glyph1 {
            ch,
            x,
            y: 100.0,
            size: 10.0,
            left: x,
            right: x + 6.0,
        };
        let lines = assemble(vec![
            glyph('o', 10.0),
            glyph('\u{FB01}', 16.0),
            glyph('e', 22.0),
        ]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "ofie");
        assert_eq!(lines[0].xs.len(), 5);
    }

    #[test]
    fn letters_are_lines_by_their_baseline_and_a_gap_is_a_word_or_a_column() {
        let glyph = |ch: char, x: f32, y: f32| Glyph1 {
            ch,
            x,
            y,
            size: 10.0,
            left: x,
            right: x + 5.0,
        };
        // Drawn in the wrong order: the lower line first, and the letters of a word out of order.
        let lines = assemble(vec![
            glyph('d', 10.0, 120.0),
            glyph('b', 10.0, 100.0),
            glyph('a', 16.0, 100.0),
            glyph('a', 5.0, 100.0),
            // A word's gap (3 points past the ink) and a column's (50).
            glyph('c', 24.0, 100.0),
            glyph('z', 90.0, 100.0),
        ]);
        let said: Vec<&str> = lines.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(said, ["aba c", "z", "d"]);
    }

    #[test]
    fn capitals_fold_one_character_at_a_time_so_that_the_places_are_the_lines() {
        assert_eq!(fold('A', false), 'a');
        assert_eq!(fold('A', true), 'A');
        // A capital whose lower case is two characters stays as it is.
        assert_eq!(fold('\u{130}', false), '\u{130}');
    }
}
