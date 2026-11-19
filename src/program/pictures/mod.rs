//! The files: the pictures and PDFs of Linear that the page draws (fetched with the key from
//! `uploads.linear.app`, held by the terminal under the name `attach` gave them), the files of Linear the
//! person saves (fetched into a copy of the program's own and sent on to the place the person chose),
//! and the files the person gives the page (received from the terminal by the handle it made, into a
//! copy of the program's own, and put in Linear's storage when a comment, an issue or a description is
//! sent). It knows nothing of the page: it takes and gives plain Rust values, and the terminal is a
//! `Files` that a test stands in for.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use pencere::{FileError, ViewControl};

pub mod document;
pub use document::{Document, QUERY_MOST, Query, Unopened};

use crate::linear::images::{self, BYTES_MOST, FILE_MOST, FILE_MOST_MIB, Format, SIDE_MOST};
use crate::linear::pdf;
use crate::linear::staged::Staged;
use crate::linear::transport::{NOT_UPLOADS, TOO_LARGE};
use crate::linear::{Error, Key, NotSent, Transport, send_file};

/// Most bytes of pictures held by the terminal for the page at once: the oldest let go first.
pub const HELD_MOST: u64 = 64 * 1024 * 1024;

/// Most files fetched at once.
const DOWNLOADS: usize = 4;

/// Why the terminal did not do what it was asked.
#[derive(Debug)]
pub enum FilesError {
    /// No view is up that holds files: nothing was sent.
    Unsupported,
    /// The terminal said no (what it said is a code of its own, which tells the program nothing it
    /// can use).
    Refused,
    /// The file the person gave is more than was to be taken.
    TooLarge,
}

/// What holds the files: the terminal, by the program's hand on its view.
pub trait Files: Send + Sync {
    /// Gives `bytes` to the terminal and returns the name the page draws them by.
    fn attach(&self, bytes: Vec<u8>) -> Result<String, FilesError>;

    /// Lets the file of `name` go.
    fn detach(&self, name: &str);

    /// The file the person gave, by `handle`, received whole into the new file `to`, at most `most`
    /// bytes (`TooLarge` where there are more, and none of it is kept): how many bytes.
    fn receive(&self, handle: &str, to: &Path, most: u64) -> Result<u64, FilesError>;

    /// The file at `from` sent to the place the person chose to save in, by `handle`: how many bytes.
    fn save(&self, handle: &str, from: &Path) -> Result<u64, FilesError>;
}

fn files_error(error: FileError) -> FilesError {
    match error {
        FileError::Unsupported => FilesError::Unsupported,
        _ => FilesError::Refused,
    }
}

impl Files for ViewControl {
    fn attach(&self, bytes: Vec<u8>) -> Result<String, FilesError> {
        ViewControl::attach(self, bytes).map_err(files_error)
    }

    fn detach(&self, name: &str) {
        ViewControl::detach(self, name);
    }

    fn receive(&self, handle: &str, to: &Path, most: u64) -> Result<u64, FilesError> {
        let over = AtomicBool::new(false);
        let got = ViewControl::receive(self, handle, to, |so_far| {
            if so_far > most {
                over.store(true, Ordering::Relaxed);
                return false;
            }
            true
        });
        match got {
            Ok(size) if size <= most => Ok(size),
            Ok(_) => Err(FilesError::TooLarge),
            Err(_) if over.load(Ordering::Relaxed) => Err(FilesError::TooLarge),
            Err(error) => Err(files_error(error)),
        }
    }

    fn save(&self, handle: &str, from: &Path) -> Result<u64, FilesError> {
        ViewControl::send(self, handle, from, |_| ()).map_err(files_error)
    }
}

/// Why a file of Linear is not drawn; the page draws its card in place of it (its name, what it is, why it
/// is not drawn, and its one verb: Save).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Not a PNG, a JPEG or a PDF (a GIF, a WebP, a video, a log, bytes that are no picture).
    Format,
    /// Over 8 MB, or a picture over 4096 pixels on a side.
    TooLarge,
    /// An address that is not one of Linear's files: it is not fetched.
    NotLinear,
    /// Linear did not give it (the network, the key, a missing file).
    Unreachable,
    /// There is no view that holds files: the page draws what it drew before there were pictures.
    NoView,
}

/// A file that is not drawn: why, and what the program knows of it that the page tells the person with the
/// card (what kind it is by its first bytes, where the bytes say a kind; its size in pixels, where they say
/// that and it is too large).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal {
    pub why: Why,
    /// `gif`, `webp`, `png`, `jpg` or `pdf` (by the bytes), where they say; none for any other.
    pub kind: Option<&'static str>,
    /// The width and the height in pixels of a picture that is over what is drawn.
    pub size: Option<(u32, u32)>,
}

impl From<Why> for Refusal {
    fn from(why: Why) -> Self {
        Self {
            why,
            kind: None,
            size: None,
        }
    }
}

impl Why {
    /// The word the page is told.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Format => "format",
            Self::TooLarge => "too-large",
            Self::NotLinear => "not-linear",
            Self::Unreachable => "unreachable",
            Self::NoView => "no-view",
        }
    }
}

/// A file the page can draw: a picture, or a PDF (`pages` is then how many pages it has, where the
/// file says; `width` and `height` are its first page's, in points).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub pdf: bool,
    pub pages: Option<u32>,
}

/// One held: what the page draws it by, how large, for whom, and when it was last asked for.
#[derive(Debug)]
struct Entry {
    shown: Shown,
    bytes: u64,
    owners: BTreeSet<String>,
    tick: u64,
}

#[derive(Debug, Default)]
struct Held {
    by_src: HashMap<String, Entry>,
    tick: u64,
}

/// How many fetches are at once: a call waits for its turn.
struct Gate {
    going: Mutex<usize>,
    freed: Condvar,
}

struct Turn<'a>(&'a Gate);

impl Gate {
    fn enter(&self) -> Turn<'_> {
        let mut going = self.going.lock().unwrap_or_else(PoisonError::into_inner);
        while *going >= DOWNLOADS {
            going = self
                .freed
                .wait(going)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *going += 1;
        Turn(self)
    }
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *self.0.going.lock().unwrap_or_else(PoisonError::into_inner) -= 1;
        self.0.freed.notify_one();
    }
}

/// A file the person gave the page, as the page names it: the handle the terminal made of it and
/// the name the person's file has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Given {
    pub handle: String,
    pub name: String,
}

/// Why a file could not be sent: for the person, in words with no address in them.
#[derive(Debug, PartialEq, Eq)]
pub enum SendFailed {
    /// The key is not taken.
    Refused,
    /// With the words that say why, the file's name in them.
    Words(String),
}

/// Why a PDF could not be read for its pages and words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unread {
    /// An address that is not one of Linear's files: it is not fetched.
    NotLinear,
    /// Over 8 MB.
    TooLarge,
    /// Linear did not give it.
    Unreachable,
    /// It is no PDF that can be read (broken, encrypted, with no page).
    Unreadable,
    /// It has more pages than a view takes.
    TooManyPages,
}

impl Unread {
    /// The words for the person, with no address in them.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::NotLinear => "That file is not one of Linear's",
            Self::TooLarge => "That PDF is over 8 MB",
            Self::Unreachable => "Linear did not give that PDF",
            Self::Unreadable => "That PDF cannot be read",
            Self::TooManyPages => "That PDF has more pages than can be shown",
        }
    }
}

/// The PDF that is read for the page: shown large, for the issue `owner`.
struct Reading {
    owner: String,
    document: Arc<Document>,
}

pub struct Pictures {
    files: Mutex<Arc<dyn Files>>,
    held: Mutex<Held>,
    /// Held while a picture is given to the terminal and entered in `held`, and while what nobody holds
    /// is let go of it: a name the terminal was just given cannot be let go by another call before the
    /// picture that holds it is entered, and two calls for one address (fetched at the same time) end as
    /// one entry that is the owners' together.
    commit: Mutex<()>,
    /// At most one PDF is read at a time, the one being shown large.
    reading: Mutex<Option<Reading>>,
    gate: Gate,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Pictures {
    #[must_use]
    pub fn new(files: Arc<dyn Files>) -> Self {
        Self {
            files: Mutex::new(files),
            held: Mutex::new(Held::default()),
            commit: Mutex::new(()),
            reading: Mutex::new(None),
            gate: Gate {
                going: Mutex::new(0),
                freed: Condvar::new(),
            },
        }
    }

    /// From now on the files are held by `files` (a test's, in place of the terminal).
    #[cfg(test)]
    pub fn using(&self, files: Arc<dyn Files>) {
        *lock(&self.files) = files;
    }

    fn files(&self) -> Arc<dyn Files> {
        Arc::clone(&lock(&self.files))
    }

    /// The picture `src` held for `owner`, if it is held (it is then `owner`'s too).
    fn have(&self, src: &str, owner: &str) -> Option<Shown> {
        let mut held = lock(&self.held);
        held.tick += 1;
        let tick = held.tick;
        let entry = held.by_src.get_mut(src)?;
        entry.owners.insert(owner.to_owned());
        entry.tick = tick;
        Some(entry.shown.clone())
    }

    /// The picture at `src`, an address of Linear's pictures, fetched with `key` (unless it is held)
    /// and given to the terminal for `owner`; or why it is not drawn.
    pub fn show(
        &self,
        transport: &dyn Transport,
        key: &Key,
        src: &str,
        owner: &str,
    ) -> Result<Shown, Refusal> {
        if !images::is_upload(src) {
            return Err(Why::NotLinear.into());
        }
        if let Some(shown) = self.have(src, owner) {
            return Ok(shown);
        }
        let _turn = self.gate.enter();
        // (Another call may have fetched it while this one waited.)
        if let Some(shown) = self.have(src, owner) {
            return Ok(shown);
        }
        let bytes = transport.fetch(key, src).map_err(|error| match error {
            Error::Failed(why) if why == TOO_LARGE => Why::TooLarge,
            Error::Failed(why) if why == NOT_UPLOADS => Why::NotLinear,
            _ => Why::Unreachable,
        })?;
        if bytes.len() as u64 > BYTES_MOST {
            return Err(Why::TooLarge.into());
        }
        let (width, height, pages, is_pdf) =
            if images::sniff(&bytes).content_type == "application/pdf" {
                let facts = pdf::facts(&bytes);
                let (width, height) = facts.size.unwrap_or(pdf::DEFAULT_SIZE);
                (width, height, facts.pages, true)
            } else {
                let not_drawn = Refusal {
                    why: Why::Format,
                    kind: images::kind_of(&bytes),
                    size: None,
                };
                let format = Format::of(&bytes).ok_or(not_drawn)?;
                let size = images::size_of(&bytes, format).ok_or(not_drawn)?;
                if size.width > SIDE_MOST || size.height > SIDE_MOST {
                    return Err(Refusal {
                        why: Why::TooLarge,
                        kind: images::kind_of(&bytes),
                        size: Some((size.width, size.height)),
                    });
                }
                (size.width, size.height, None, false)
            };
        let length = bytes.len() as u64;
        let _commit = lock(&self.commit);
        // (Another call for this address may have entered it while this one fetched: its owner is added
        // to that entry, and the bytes are not given to the terminal twice.)
        if let Some(shown) = self.have(src, owner) {
            return Ok(shown);
        }
        let name = self.files().attach(bytes).map_err(|why| match why {
            FilesError::Unsupported => Why::NoView,
            FilesError::Refused | FilesError::TooLarge => Why::Unreachable,
        })?;
        let shown = Shown {
            name,
            width,
            height,
            pdf: is_pdf,
            pages,
        };
        let gone = {
            let mut held = lock(&self.held);
            held.tick += 1;
            let tick = held.tick;
            let mut gone = Vec::new();
            // The oldest go until this fits.
            while held.by_src.values().map(|entry| entry.bytes).sum::<u64>() + length > HELD_MOST {
                let Some(oldest) = held
                    .by_src
                    .iter()
                    .min_by_key(|(_, entry)| entry.tick)
                    .map(|(src, _)| src.clone())
                else {
                    break;
                };
                if let Some(entry) = held.by_src.remove(&oldest) {
                    gone.push(entry.shown.name);
                }
            }
            held.by_src.insert(
                src.to_owned(),
                Entry {
                    shown: shown.clone(),
                    bytes: length,
                    owners: BTreeSet::from([owner.to_owned()]),
                    tick,
                },
            );
            gone
        };
        // (The same bytes held twice are one file of the terminal's: let go only what no other holds.)
        let files = self.files();
        for name in gone {
            if !lock(&self.held)
                .by_src
                .values()
                .any(|entry| entry.shown.name == name)
            {
                files.detach(&name);
            }
        }
        Ok(shown)
    }

    /// The PDF at `src`, an address of Linear's files, fetched again with `key` (the terminal holds the
    /// bytes it draws; the page's words come from the program's own copy) and read, for `owner`; the PDF
    /// read before is let go. It is held until `close_pdf`, `release`, or another is opened.
    ///
    /// # Errors
    ///
    /// Why it is not read.
    pub fn open_pdf(
        &self,
        transport: &dyn Transport,
        key: &Key,
        src: &str,
        owner: &str,
    ) -> Result<Arc<Document>, Unread> {
        if !images::is_upload(src) {
            return Err(Unread::NotLinear);
        }
        let bytes = {
            let _turn = self.gate.enter();
            transport.fetch(key, src).map_err(|error| match error {
                Error::Failed(why) if why == TOO_LARGE => Unread::TooLarge,
                Error::Failed(why) if why == NOT_UPLOADS => Unread::NotLinear,
                _ => Unread::Unreachable,
            })?
        };
        if bytes.len() as u64 > BYTES_MOST {
            return Err(Unread::TooLarge);
        }
        if images::sniff(&bytes).content_type != "application/pdf" {
            return Err(Unread::Unreadable);
        }
        let document = Document::open(Arc::from(bytes)).map_err(|why| match why {
            Unopened::Unreadable => Unread::Unreadable,
            Unopened::TooManyPages => Unread::TooManyPages,
        })?;
        let document = Arc::new(document);
        *lock(&self.reading) = Some(Reading {
            owner: owner.to_owned(),
            document: Arc::clone(&document),
        });
        Ok(document)
    }

    /// The PDF that is read for `owner`, if it is.
    #[must_use]
    pub fn document(&self, owner: &str) -> Option<Arc<Document>> {
        lock(&self.reading)
            .as_ref()
            .filter(|reading| reading.owner == owner)
            .map(|reading| Arc::clone(&reading.document))
    }

    /// The PDF read for `owner` is let go.
    pub fn close_pdf(&self, owner: &str) {
        let mut reading = lock(&self.reading);
        if reading.as_ref().is_some_and(|one| one.owner == owner) {
            *reading = None;
        }
    }

    /// What is held for `owner` is let go: the pictures nobody else asked for go from the terminal.
    pub fn release(&self, owner: &str) {
        self.close_pdf(owner);
        let _commit = lock(&self.commit);
        let gone: Vec<String> = {
            let mut held = lock(&self.held);
            for entry in held.by_src.values_mut() {
                entry.owners.remove(owner);
            }
            let empty: Vec<String> = held
                .by_src
                .iter()
                .filter(|(_, entry)| entry.owners.is_empty())
                .map(|(src, _)| src.clone())
                .collect();
            let mut names = Vec::new();
            for src in empty {
                if let Some(entry) = held.by_src.remove(&src) {
                    names.push(entry.shown.name);
                }
            }
            names.retain(|name| !held.by_src.values().any(|entry| &entry.shown.name == name));
            names
        };
        let files = self.files();
        for name in gone {
            files.detach(&name);
        }
    }

    /// How many pictures are held, and the names of them (for the tests).
    #[cfg(test)]
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = lock(&self.held)
            .by_src
            .values()
            .map(|entry| entry.shown.name.clone())
            .collect();
        names.sort();
        names
    }

    /// Puts each of `given` in Linear's storage, in order, and gives the Markdown line of each. The
    /// first that cannot be sent ends it: nothing is sent after it, and the words say which and why.
    /// `what` is what was to be done with them (`posted`, `made`, `added`), for the words. Each is
    /// received from the terminal into a copy of the program's own (at most 100 MiB, a piece at a time:
    /// not held in memory), which is what is sent, and is gone when the file is sent or not.
    pub fn send(
        &self,
        transport: &dyn Transport,
        key: &Key,
        given: &[Given],
        what: &str,
    ) -> Result<Vec<String>, SendFailed> {
        let mut lines = Vec::new();
        for one in given {
            let name = images::clean_name(&one.name);
            let not = |why: &str| SendFailed::Words(format!("{name} {why}, so nothing was {what}"));
            let staged = Staged::new().map_err(|_| not("could not be kept to be sent"))?;
            match self.files().receive(&one.handle, staged.path(), FILE_MOST) {
                Ok(_) => {}
                Err(FilesError::TooLarge) => {
                    return Err(not(&format!("is over {FILE_MOST_MIB} MiB")));
                }
                Err(_) => return Err(not("could not be read")),
            }
            staged
                .private()
                .map_err(|_| not("could not be kept to be sent"))?;
            match send_file(transport, key, &name, staged.path()) {
                Ok(line) => lines.push(line),
                Err(NotSent::Empty) => return Err(not("is empty")),
                Err(NotSent::TooLarge) => {
                    return Err(not(&format!("is over {FILE_MOST_MIB} MiB")));
                }
                Err(NotSent::Unreadable) => return Err(not("could not be read")),
                Err(NotSent::Linear(Error::Refused)) => return Err(SendFailed::Refused),
                Err(NotSent::Linear(error)) => return Err(not(&format!("was not sent ({error})"))),
            }
        }
        Ok(lines)
    }

    /// The file at `src`, an address of Linear's files, fetched with `key` (at most 100 MiB, into a copy
    /// of the program's own, not into memory) and sent on to the place the person chose to save it in,
    /// by `handle`: how many bytes.
    pub fn save(
        &self,
        transport: &dyn Transport,
        key: &Key,
        src: &str,
        handle: &str,
    ) -> Result<u64, Saved> {
        if !images::is_upload(src) {
            return Err(Saved::NotLinear);
        }
        let _turn = self.gate.enter();
        let staged = Staged::new().map_err(|_| Saved::Unreachable)?;
        transport
            .fetch_to(key, src, staged.path(), FILE_MOST)
            .map_err(|error| match error {
                Error::Failed(why) if why == TOO_LARGE => Saved::TooLarge,
                Error::Failed(why) if why == NOT_UPLOADS => Saved::NotLinear,
                _ => Saved::Unreachable,
            })?;
        self.files()
            .save(handle, staged.path())
            .map_err(|why| match why {
                FilesError::Unsupported => Saved::NoView,
                _ => Saved::Refused,
            })
    }
}

/// Why a file of Linear was not saved where the person chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// An address that is not one of Linear's files: it is not fetched.
    NotLinear,
    /// Over `FILE_MOST`.
    TooLarge,
    /// Linear did not give it.
    Unreachable,
    /// The terminal did not take it (the place is full, or it is gone).
    Refused,
    /// There is no view that holds files.
    NoView,
}

#[cfg(test)]
pub mod double;

#[cfg(test)]
mod tests;
