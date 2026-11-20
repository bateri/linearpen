//! Files on the command line: the files `--attach` sends and the files `show` finds in an issue and,
//! with `--files`, saves. The fetching and the sending are the pane's (`linear::Transport::fetch_to`,
//! `linear::send_file`): this holds only what the command line adds to them, which is paths, names and
//! words.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::{Access, Stop, asked_wrong};
use crate::linear::images::{self, FILE_MOST, FILE_MOST_MIB, SAVED_BYTES_MOST, SAVED_FILES_MOST};
use crate::linear::staged::{CopyFailed, Staged};
use crate::linear::transport::TOO_LARGE;
use crate::linear::{self, Error, Issue, NotSent, plain};

/// A file to send, as `--attach` named it: looked at, not yet copied.
pub struct Sendable {
    pub name: String,
    pub path: PathBuf,
}

/// Looks at the files of `--attach`, before Linear is asked anything: each is a file (not a folder, not
/// a device), not empty and at most 100 MiB. What is not is told with status 2. Nothing is read of them
/// yet: `put` copies each as it sends it.
pub fn read_files(paths: &[String]) -> Result<Vec<Sendable>, Stop> {
    let mut files = Vec::new();
    for path in paths {
        let shown = plain(path);
        let cannot =
            |why: std::io::Error| Stop::usage(format!("the file {shown} cannot be read: {why}"));
        // What is not a file (a folder, a pipe that would wait for a writer) is not opened at all.
        let found = fs::metadata(path).map_err(cannot)?;
        if !found.is_file() {
            return Err(Stop::usage(format!("{shown} is not a file")));
        }
        let meta = fs::File::open(path)
            .and_then(|file| file.metadata())
            .map_err(cannot)?;
        if !meta.is_file() {
            return Err(Stop::usage(format!("{shown} is not a file")));
        }
        if meta.len() == 0 {
            return Err(Stop::usage(format!("the file {shown} is empty")));
        }
        if meta.len() > FILE_MOST {
            return Err(Stop::usage(format!(
                "the file {shown} is over {} MiB",
                FILE_MOST_MIB
            )));
        }
        let name = Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        files.push(Sendable {
            name: images::clean_name(&name),
            path: PathBuf::from(path),
        });
    }
    Ok(files)
}

/// Puts the files in Linear's storage, one after the other, and gives the Markdown line of each. Each
/// is copied, as it is put, into a file of the program's own that nothing else can read, so that what
/// was looked at is what is sent; the copy is gone when it has been sent or has not. The first that
/// cannot be put there ends it with status 1, and nothing has been written to an issue (a file
/// already in storage is no part of any).
pub fn put(access: &Access, files: &[Sendable]) -> Result<Vec<String>, Stop> {
    let mut lines = Vec::new();
    for file in files {
        let name = plain(&file.name);
        let staged = Staged::new().map_err(|why| {
            Stop::no(format!(
                "the file {name} could not be copied to be sent: {why}; nothing was written"
            ))
        })?;
        let mut source = fs::File::open(&file.path)
            .map_err(|why| Stop::usage(format!("the file {name} cannot be read: {why}")))?;
        match staged.copy_in(&mut source, FILE_MOST) {
            Ok(_) => {}
            Err(CopyFailed::TooLarge) => {
                return Err(Stop::usage(format!(
                    "the file {name} is over {} MiB",
                    FILE_MOST_MIB
                )));
            }
            Err(CopyFailed::Io) => {
                return Err(Stop::usage(format!("the file {name} cannot be read")));
            }
        }
        match linear::send_file(access.transport, access.key, &file.name, staged.path()) {
            Ok(line) => lines.push(line),
            Err(NotSent::Empty) => {
                return Err(Stop::usage(format!("the file {name} is empty")));
            }
            Err(NotSent::TooLarge) => {
                return Err(Stop::usage(format!(
                    "the file {name} is over {} MiB",
                    FILE_MOST_MIB
                )));
            }
            Err(NotSent::Unreadable) => {
                return Err(Stop::usage(format!("the file {name} cannot be read")));
            }
            Err(NotSent::Linear(error)) => {
                let why = asked_wrong(access, &error).words;
                return Err(Stop::no(format!(
                    "the file {name} could not be put in Linear's storage ({why}); nothing was written"
                )));
            }
        }
    }
    Ok(lines)
}

/// `with 2 files`, for the end of what a command prints (nothing for none).
#[must_use]
pub fn with_words(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => "with 1 file".to_owned(),
        n => format!("with {n} files"),
    }
}

// --- What `show` finds ------------------------------------------------------------------------------

/// Where a file stands in the issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    Description,
    /// The place of the comment among the comments (from 0) and who wrote it.
    Comment {
        index: usize,
        author: String,
    },
}

/// Why a file was not saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NotOnLinear,
    TooLarge,
    CouldNotBeFetched,
    /// More files than a run saves (`SAVED_FILES_MOST`).
    TooMany,
    /// The files before it came to what a run saves (`SAVED_BYTES_MOST`) and this would be more.
    TooMuch,
}

impl Refused {
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::NotOnLinear => "not on Linear",
            Self::TooLarge => "too large",
            Self::CouldNotBeFetched => "could not be fetched",
            Self::TooMany => "too many files",
            Self::TooMuch => "too much in all",
        }
    }
}

/// A picture or a file of an issue: its number, its name (the alt of a picture, the words of a link), its
/// address, whether it is written as a picture, where it stands, and, after `save`, its file or why
/// there is none.
#[derive(Debug, Clone)]
pub struct Seen {
    pub n: usize,
    pub name: String,
    pub src: String,
    pub image: bool,
    pub place: Place,
    pub path: Option<PathBuf>,
    pub refused: Option<Refused>,
}

/// The issue's texts with each picture put as `[image N: name]` and each file as `[file N: name]`, and
/// them in the order of the issue: the description, then each comment.
pub struct Marked {
    pub description: Option<String>,
    pub comments: Vec<String>,
    pub seen: Vec<Seen>,
}

/// `text` with its pictures put as `[image N: name]` and its files as `[file N: name]`, numbered from
/// `first`; the ones it names.
fn mark_text(text: &str, first: usize, place: &Place) -> (String, Vec<Seen>) {
    let mut out = String::new();
    let mut seen = Vec::new();
    let mut at = 0;
    for named in images::find(text) {
        out.push_str(&text[at..named.start]);
        let n = first + seen.len();
        let shown = if named.name.trim().is_empty() {
            let from_address = images::name_in(&named.src);
            if from_address.is_empty() {
                if named.image { "image" } else { "file" }.to_owned()
            } else {
                from_address
            }
        } else {
            named.name.trim().to_owned()
        };
        let kind = if named.image { "image" } else { "file" };
        out.push_str(&format!("[{kind} {n}: {}]", plain(&shown)));
        seen.push(Seen {
            n,
            name: named.name,
            src: named.src,
            image: named.image,
            place: place.clone(),
            path: None,
            refused: None,
        });
        at = named.end;
    }
    out.push_str(&text[at..]);
    (out, seen)
}

/// The pictures and files of `issue`, and its texts as they are shown.
#[must_use]
pub fn mark(issue: &Issue) -> Marked {
    let mut seen: Vec<Seen> = Vec::new();
    let description = issue.description.as_deref().map(|text| {
        let (marked, found) = mark_text(text, 1, &Place::Description);
        seen.extend(found);
        marked
    });
    let mut comments = Vec::new();
    for (index, comment) in issue
        .comments
        .iter()
        .flat_map(|comments| comments.nodes.iter())
        .enumerate()
    {
        let author = comment
            .user
            .as_ref()
            .map_or_else(|| "Someone".to_owned(), |who| plain(&who.display_name));
        let (marked, found) = mark_text(
            &comment.body,
            seen.len() + 1,
            &Place::Comment { index, author },
        );
        seen.extend(found);
        comments.push(marked);
    }
    Marked {
        description,
        comments,
        seen,
    }
}

/// Fetches each file of `seen` that is on Linear and writes it in `dir`, with the name `N-name.ext`
/// (`name` from the words or the address, `ext` from the bytes, else from the name, else none): a file
/// of that name that holds the same bytes is the one used, one that holds others makes the name
/// `N-name-2.ext`, `-3`, and nothing is written over. `dir` is made, mode 0700, where it is not there.
/// Each is fetched into a file of the program's own (not into memory) and copied from there. A file that
/// cannot be had gets its reason, and the rest go on. At most `SAVED_FILES_MOST` files are fetched, and
/// no more than `SAVED_BYTES_MOST` of them together (a file past either is `too many files` or `too
/// much in all`). Only a `dir` that cannot be made stops it (status 1).
pub fn save(access: &Access, dir: &Path, seen: &mut [Seen]) -> Result<(), Stop> {
    save_within(access, dir, seen, SAVED_FILES_MOST, SAVED_BYTES_MOST)
}

/// `save` with the most files and the most bytes it may take given.
fn save_within(
    access: &Access,
    dir: &Path,
    seen: &mut [Seen],
    files_most: usize,
    bytes_most: u64,
) -> Result<(), Stop> {
    let mut ready: Option<PathBuf> = None;
    let mut fetched_files = 0;
    let mut fetched_bytes: u64 = 0;
    for one in seen.iter_mut() {
        let on_linear = images::is_upload(&one.src);
        if on_linear && fetched_files >= files_most {
            one.refused = Some(Refused::TooMany);
            continue;
        }
        let room = bytes_most.saturating_sub(fetched_bytes);
        if on_linear && room == 0 {
            one.refused = Some(Refused::TooMuch);
            continue;
        }
        if on_linear {
            fetched_files += 1;
        }
        match fetch(access, &one.src, room.min(FILE_MOST)) {
            Err(why) => one.refused = Some(why),
            Ok((fetched, head, size)) => {
                fetched_bytes += size;
                if ready.is_none() {
                    ready = Some(make_dir(dir)?);
                }
                let folder = ready.as_deref().unwrap_or(dir);
                let words = if one.name.trim().is_empty() {
                    images::name_in(&one.src)
                } else {
                    one.name.clone()
                };
                let stem = images::file_stem(&words);
                let ext = images::kind_of(&head)
                    .map(str::to_owned)
                    .or_else(|| images::name_ext(&words))
                    .or_else(|| images::name_ext(&images::name_in(&one.src)));
                match store(folder, one.n, &stem, ext.as_deref(), fetched.path()) {
                    Some(path) => one.path = Some(path),
                    None => one.refused = Some(Refused::CouldNotBeFetched),
                }
            }
        }
    }
    Ok(())
}

/// The file at `src` fetched into a copy of the program's own (at most `most` bytes), with the first
/// bytes of it and how large it is, or why not.
fn fetch(access: &Access, src: &str, most: u64) -> Result<(Staged, Vec<u8>, u64), Refused> {
    if !images::is_upload(src) {
        return Err(Refused::NotOnLinear);
    }
    let staged = Staged::new().map_err(|_| Refused::CouldNotBeFetched)?;
    match access
        .transport
        .fetch_to(access.key, src, staged.path(), most)
    {
        Ok(_) => {}
        Err(Error::Failed(why)) if why == TOO_LARGE => {
            // (Where `most` is what the run has room for and not what a file may be, it is the run that
            // is full.)
            return Err(if most < FILE_MOST {
                Refused::TooMuch
            } else {
                Refused::TooLarge
            });
        }
        Err(_) => return Err(Refused::CouldNotBeFetched),
    }
    let mut head = Vec::new();
    fs::File::open(staged.path())
        .and_then(|opened| opened.take(16).read_to_end(&mut head))
        .map_err(|_| Refused::CouldNotBeFetched)?;
    let size = fs::metadata(staged.path()).map_or(0, |meta| meta.len());
    Ok((staged, head, size))
}

/// `dir`, made with mode 0700 where it is not there, as an absolute path.
fn make_dir(dir: &Path) -> Result<PathBuf, Stop> {
    let shown = plain(&dir.display().to_string());
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|why| Stop::no(format!("the folder {shown} could not be made: {why}")))?;
    if !dir.is_dir() {
        return Err(Stop::no(format!("{shown} is not a folder")));
    }
    std::path::absolute(dir)
        .map_err(|why| Stop::no(format!("the folder {shown} could not be found out: {why}")))
}

/// Whether the files at `a` and `b` hold the same bytes (read a piece at a time).
fn same_bytes(a: &Path, b: &Path) -> bool {
    let (Ok(mut one), Ok(mut other)) = (fs::File::open(a), fs::File::open(b)) else {
        return false;
    };
    if one.metadata().map(|meta| meta.len()).ok() != other.metadata().map(|meta| meta.len()).ok() {
        return false;
    }
    let (mut left, mut right) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        let Ok(got) = one.read(&mut left) else {
            return false;
        };
        if got == 0 {
            return true;
        }
        if other.read_exact(&mut right[..got]).is_err() || left[..got] != right[..got] {
            return false;
        }
    }
}

/// Copies the file at `from` into `folder` under the first name that is free or has the same bytes in
/// it. Nothing is written over a file, and a link is never followed to one (the file is made new or not
/// at all).
fn store(folder: &Path, n: usize, stem: &str, ext: Option<&str>, from: &Path) -> Option<PathBuf> {
    let dot = ext.map(|ext| format!(".{ext}")).unwrap_or_default();
    for attempt in 1..=999 {
        let name = if attempt == 1 {
            format!("{n}-{stem}{dot}")
        } else {
            format!("{n}-{stem}-{attempt}{dot}")
        };
        let path = folder.join(name);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut file) => {
                let copied = fs::File::open(from)
                    .and_then(|mut source| std::io::copy(&mut source, &mut file))
                    .and_then(|_| file.flush());
                if copied.is_err() {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return None;
                }
                return Some(path);
            }
            Err(why) if why.kind() == ErrorKind::AlreadyExists => {
                let same = fs::symlink_metadata(&path)
                    .is_ok_and(|meta| meta.is_file() && same_bytes(&path, from));
                if same {
                    return Some(path);
                }
            }
            Err(_) => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::Key;
    use crate::linear::transport::{Reply, Transport};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Linear's files, each of the bytes its address ends in `.<n>` says (that many `x`), and a count of
    /// the files asked for.
    struct Sized {
        asked: AtomicUsize,
    }

    impl Transport for Sized {
        fn post(&self, _: &Key, _: &str) -> Result<Reply, Error> {
            Err(Error::Unreachable("no questions here".to_owned()))
        }

        fn fetch_to(&self, _: &Key, url: &str, to: &Path, most: u64) -> Result<u64, Error> {
            self.asked.fetch_add(1, Ordering::SeqCst);
            let size: u64 = url.rsplit('.').next().unwrap().parse().unwrap();
            if size > most {
                return Err(Error::Failed(TOO_LARGE.to_owned()));
            }
            fs::write(to, vec![b'x'; usize::try_from(size).unwrap()])
                .map_err(|_| Error::Unreachable("write".to_owned()))?;
            Ok(size)
        }
    }

    fn seen(addresses: &[&str]) -> Vec<Seen> {
        addresses
            .iter()
            .enumerate()
            .map(|(at, src)| Seen {
                n: at + 1,
                name: format!("f{}", at + 1),
                src: (*src).to_owned(),
                image: false,
                place: Place::Description,
                path: None,
                refused: None,
            })
            .collect()
    }

    fn run(
        addresses: &[&str],
        files_most: usize,
        bytes_most: u64,
    ) -> (Vec<Option<Refused>>, usize) {
        let transport = Sized {
            asked: AtomicUsize::new(0),
        };
        let key = Key::new("k").unwrap();
        let access = Access {
            transport: &transport,
            key: &key,
            key_source: "test",
            panes: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let mut found = seen(addresses);
        save_within(
            &access,
            &dir.path().join("out"),
            &mut found,
            files_most,
            bytes_most,
        )
        .unwrap();
        (
            found.iter().map(|one| one.refused).collect(),
            transport.asked.load(Ordering::SeqCst),
        )
    }

    const A: &str = "https://uploads.linear.app/w/a.4";

    #[test]
    fn past_the_most_files_the_rest_are_told_too_many_and_not_fetched() {
        // The one that is not Linear's is not a file of the run and does not count.
        let (refused, asked) = run(&[A, A, "https://example.com/x.4", A, A, A], 3, 1000);
        assert_eq!(
            refused,
            [
                None,
                None,
                Some(Refused::NotOnLinear),
                None,
                Some(Refused::TooMany),
                Some(Refused::TooMany)
            ]
        );
        assert_eq!(asked, 3);
    }

    #[test]
    fn past_the_most_bytes_the_rest_are_told_too_much_in_all_and_what_would_overrun_is_not_taken() {
        // Ten bytes in all, four to a file: two are taken, the third would come to twelve.
        let (refused, asked) = run(&[A, A, A, A], 100, 10);
        assert_eq!(
            refused,
            [None, None, Some(Refused::TooMuch), Some(Refused::TooMuch)]
        );
        assert_eq!(asked, 4);
        // Eight in all: the room is used up, and the third is not even asked for.
        let (refused, asked) = run(&[A, A, A], 100, 8);
        assert_eq!(refused, [None, None, Some(Refused::TooMuch)]);
        assert_eq!(asked, 2);
        assert_eq!(Refused::TooMany.words(), "too many files");
        assert_eq!(Refused::TooMuch.words(), "too much in all");
    }
}
