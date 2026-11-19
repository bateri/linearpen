use std::path::Path;
use std::sync::{Arc, Mutex};

use super::double::Double;
use super::*;
use crate::linear::transport::Reply;

fn png_of(width: u32, height: u32, padding: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes.resize(bytes.len() + padding, 7);
    bytes
}

const KEY: &str = "lin_api_picture_key";
const SRC: &str = "https://uploads.linear.app/ws/a/one.png";

/// What was sent with `put`: the address, the headers and the bytes of the file at that moment.
type Put = (String, Vec<(String, String)>, Vec<u8>);

/// Linear and its storage, in memory.
#[derive(Default)]
struct Memory {
    /// The pictures by address.
    files: Mutex<HashMap<String, Result<Vec<u8>, Error>>>,
    /// Every address `fetch` was asked for, with the key it was given.
    fetched: Mutex<Vec<(String, String)>>,
    /// What was sent with `put`: the address, the headers and the bytes.
    put: Mutex<Vec<Put>>,
    /// The status `put` answers with.
    status: Mutex<u16>,
    uploads: Mutex<u32>,
}

impl Memory {
    fn with(src: &str, bytes: Vec<u8>) -> Arc<Self> {
        let memory = Arc::new(Self::default());
        memory.add(src, Ok(bytes));
        *lock(&memory.status) = 200;
        memory
    }

    fn add(&self, src: &str, answer: Result<Vec<u8>, Error>) {
        lock(&self.files).insert(src.to_owned(), answer);
    }
}

impl Transport for Memory {
    fn post(&self, _: &Key, body: &str) -> Result<crate::linear::transport::Reply, Error> {
        assert!(body.contains("\"operationName\":\"FileUpload\""), "{body}");
        let mut uploads = lock(&self.uploads);
        *uploads += 1;
        let n = *uploads;
        Ok(Reply {
            status: 200,
            body: format!(
                r#"{{"data":{{"fileUpload":{{"success":true,"uploadFile":{{"uploadUrl":"https://storage.stand-in/put/{n}?sig=SECRET","assetUrl":"https://uploads.linear.app/stand-in/{n}","headers":[{{"key":"x-goog-meta-a","value":"b"}},{{"key":"Content-Type","value":"wrong"}}]}}}}}}}}"#
            ),
        })
    }

    fn fetch(&self, key: &Key, url: &str) -> Result<Vec<u8>, Error> {
        lock(&self.fetched).push((url.to_owned(), key.expose().to_owned()));
        lock(&self.files)
            .get(url)
            .cloned()
            .unwrap_or(Err(Error::Failed("Linear did not give the picture".into())))
    }

    fn fetch_to(&self, key: &Key, url: &str, to: &Path, most: u64) -> Result<u64, Error> {
        let bytes = self.fetch(key, url)?;
        if bytes.len() as u64 > most {
            return Err(Error::Failed(TOO_LARGE.to_owned()));
        }
        std::fs::write(to, &bytes).map_err(|_| Error::Unreachable("write".into()))?;
        Ok(bytes.len() as u64)
    }

    fn put(
        &self,
        url: &str,
        headers: &[(String, String)],
        file: &Path,
        size: u64,
    ) -> Result<u16, Error> {
        let body = std::fs::read(file).expect("the copy is there when it is sent");
        assert_eq!(body.len() as u64, size);
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(file).unwrap().permissions().mode() & 0o777;
            let folder = file.parent().unwrap();
            let folder_mode = std::fs::metadata(folder).unwrap().permissions().mode() & 0o777;
            assert_eq!((mode, folder_mode), (0o600, 0o700), "{file:?}");
        }
        lock(&self.put).push((url.to_owned(), headers.to_vec(), body));
        Ok(*lock(&self.status))
    }
}

fn key() -> Key {
    Key::new(KEY).unwrap()
}

fn store() -> (Pictures, Arc<Double>) {
    let double = Arc::new(Double::default());
    let files: Arc<dyn Files> = double.clone();
    (Pictures::new(files), double)
}

#[test]
fn a_picture_of_linear_is_fetched_once_for_every_issue_that_shows_it_and_let_go_with_the_last() {
    let (pictures, double) = store();
    let memory = Memory::with(SRC, png_of(640, 480, 10));
    let shown = pictures.show(&*memory, &key(), SRC, "issue-1").unwrap();
    assert_eq!((shown.width, shown.height), (640, 480));
    assert!(shown.name.starts_with("sha256:"));
    assert_eq!(double.attached().len(), 1);
    // Another issue shows the same picture: it is not fetched, nor attached, again.
    assert_eq!(
        pictures.show(&*memory, &key(), SRC, "issue-2").unwrap(),
        shown
    );
    assert_eq!(lock(&memory.fetched).len(), 1);
    assert_eq!(double.attached().len(), 1);
    // The first goes: the picture stays for the second, and goes with it.
    pictures.release("issue-1");
    assert!(double.detached().is_empty());
    pictures.release("issue-2");
    assert_eq!(double.detached(), [shown.name]);
    assert!(pictures.names().is_empty());
}

/// A transport whose fetch waits for another call to be fetching too, so that two calls for the same
/// address are both on their way when either is entered.
struct Together {
    inside: Arc<Memory>,
    both: std::sync::Barrier,
}

impl Transport for Together {
    fn post(&self, key: &Key, body: &str) -> Result<Reply, Error> {
        self.inside.post(key, body)
    }

    fn fetch(&self, key: &Key, url: &str) -> Result<Vec<u8>, Error> {
        let bytes = self.inside.fetch(key, url);
        self.both.wait();
        bytes
    }

    fn fetch_to(&self, key: &Key, url: &str, to: &Path, most: u64) -> Result<u64, Error> {
        self.inside.fetch_to(key, url, to, most)
    }

    fn put(
        &self,
        url: &str,
        headers: &[(String, String)],
        file: &Path,
        size: u64,
    ) -> Result<u16, Error> {
        self.inside.put(url, headers, file, size)
    }
}

#[test]
fn two_calls_for_one_picture_at_the_same_time_make_one_entry_of_both_owners_and_none_is_let_go_early()
 {
    let (pictures, double) = store();
    let together = Together {
        inside: Memory::with(SRC, png_of(640, 480, 10)),
        both: std::sync::Barrier::new(2),
    };
    let (first, second) = std::thread::scope(|scope| {
        let one = scope.spawn(|| pictures.show(&together, &key(), SRC, "issue-1"));
        let two = scope.spawn(|| pictures.show(&together, &key(), SRC, "issue-2"));
        (one.join().unwrap().unwrap(), two.join().unwrap().unwrap())
    });
    // Both were fetched (the barrier saw to it), and the terminal was given the bytes once.
    assert_eq!(lock(&together.inside.fetched).len(), 2);
    assert_eq!(first, second);
    assert_eq!(double.attached().len(), 1);
    assert_eq!(pictures.names().len(), 1);
    // The picture is both owners': the first to let go does not take it from the second.
    pictures.release("issue-1");
    assert!(double.detached().is_empty(), "{:?}", double.detached());
    assert_eq!(pictures.names().len(), 1);
    pictures.release("issue-2");
    assert_eq!(double.detached(), [first.name]);
    assert!(pictures.names().is_empty());
}

#[test]
fn only_an_address_of_linears_pictures_is_fetched_and_the_key_goes_with_it_alone() {
    let (pictures, _) = store();
    let memory = Memory::with(SRC, png_of(4, 4, 0));
    for src in [
        "https://evil.example/a.png",
        "https://uploads.linear.app.evil.example/a.png",
        "http://uploads.linear.app/a.png",
        "https://user@uploads.linear.app/a.png",
        "https://uploads.linear.app:8443/a.png",
        "file:///etc/passwd",
        "a.png",
    ] {
        assert_eq!(
            pictures.show(&*memory, &key(), src, "issue-1"),
            Err(Why::NotLinear.into()),
            "{src}"
        );
    }
    assert!(lock(&memory.fetched).is_empty(), "nothing was fetched");
    pictures.show(&*memory, &key(), SRC, "issue-1").unwrap();
    assert_eq!(
        lock(&memory.fetched).as_slice(),
        [(SRC.to_owned(), KEY.to_owned())]
    );
}

#[test]
fn what_is_not_a_png_a_jpeg_or_a_pdf_or_is_too_large_is_refused_by_name() {
    let (pictures, double) = store();
    let memory = Memory::with(SRC, png_of(4, 4, 0));
    let refused = |src: &str, answer: Result<Vec<u8>, Error>| {
        memory.add(src, answer);
        pictures.show(&*memory, &key(), src, "issue-1").unwrap_err()
    };
    let why = |src: &str, answer: Result<Vec<u8>, Error>| refused(src, answer).why;
    let at = |n: u32| format!("https://uploads.linear.app/ws/{n}");
    assert_eq!(why(&at(1), Ok(b"GIF89a....".to_vec())), Why::Format);
    assert_eq!(
        why(&at(2), Ok(b"RIFF\0\0\0\0WEBPVP8 ".to_vec())),
        Why::Format
    );
    assert_eq!(why(&at(3), Ok(png_of(4097, 10, 0))), Why::TooLarge);
    assert_eq!(why(&at(4), Ok(png_of(10, 4097, 0))), Why::TooLarge);
    assert_eq!(
        why(&at(5), Err(Error::Failed(TOO_LARGE.to_owned()))),
        Why::TooLarge
    );
    assert_eq!(
        why(&at(6), Err(Error::Unreachable("x".into()))),
        Why::Unreachable
    );
    assert_eq!(
        why(&at(7), Err(Error::Failed("no".into()))),
        Why::Unreachable
    );
    assert_eq!(why(&at(9), Ok(b"PK\x03\x04zip".to_vec())), Why::Format);
    assert_eq!(
        why(&at(10), Ok(b"plain text of a log".to_vec())),
        Why::Format
    );
    // A PDF over 8 MiB is a file to save, not one to draw.
    let mut big_pdf = b"%PDF-1.7\n".to_vec();
    big_pdf.resize(BYTES_MOST as usize + 1, b' ');
    assert_eq!(why(&at(11), Ok(big_pdf)), Why::TooLarge);
    // What the bytes say is told with the reason, so the card can say it.
    let gif = refused(&at(12), Ok(b"GIF89a....".to_vec()));
    assert_eq!((gif.kind, gif.size), (Some("gif"), None));
    let webp = refused(&at(13), Ok(b"RIFF\0\0\0\0WEBPVP8 ".to_vec()));
    assert_eq!(webp.kind, Some("webp"));
    let wide = refused(&at(14), Ok(png_of(5120, 2880, 0)));
    assert_eq!((wide.kind, wide.size), (Some("png"), Some((5120, 2880))));
    let log = refused(&at(15), Ok(b"plain text of a log".to_vec()));
    assert_eq!((log.kind, log.size), (None, None));
    // A refused picture is never given to the terminal.
    assert!(double.attached().is_empty());
    // The edge is drawn.
    assert!(
        pictures
            .show(
                &*Memory::with(&at(8), png_of(4096, 4096, 0)),
                &key(),
                &at(8),
                "issue-1"
            )
            .is_ok()
    );
}

#[test]
fn a_pdf_is_held_by_the_terminal_and_the_page_is_told_its_pages_and_its_size() {
    let (pictures, double) = store();
    let pdf = b"%PDF-1.4\n2 0 obj\n<< /Type /Pages /Count 4 /MediaBox [0 0 595 842] >>\nendobj\n"
        .to_vec();
    let memory = Memory::with(SRC, pdf.clone());
    let shown = pictures.show(&*memory, &key(), SRC, "issue-1").unwrap();
    assert!(shown.pdf);
    assert_eq!(
        (shown.width, shown.height, shown.pages),
        (595, 842, Some(4))
    );
    assert_eq!(double.attached(), [pdf]);
    // One that says nothing of its pages is shown all the same, with its pages not known.
    let at = "https://uploads.linear.app/ws/plain.pdf";
    memory.add(at, Ok(b"%PDF-1.7\nstream\n\0\x01\nendstream".to_vec()));
    let plain = pictures.show(&*memory, &key(), at, "issue-1").unwrap();
    assert_eq!((plain.width, plain.height, plain.pages), (612, 792, None));
    // It goes with the issue like a picture.
    pictures.release("issue-1");
    assert_eq!(double.detached().len(), 2);
}

#[test]
fn a_pdf_is_read_for_the_issue_that_shows_it_large_one_at_a_time_and_let_go_with_it() {
    let (pictures, _double) = store();
    let paper = "https://uploads.linear.app/ws/a/paper.pdf";
    let other = "https://uploads.linear.app/ws/a/other.pdf";
    let memory = Memory::with(paper, double::pdf(3));
    memory.add(other, Ok(double::pdf(2)));
    memory.add(SRC, Ok(png_of(4, 4, 0)));
    // Fetched again with the key (the terminal holds what it draws), and read: its pages and its words.
    let document = pictures
        .open_pdf(&*memory, &key(), paper, "issue-1")
        .unwrap();
    assert_eq!(document.sizes().len(), 3);
    assert!(Arc::ptr_eq(
        &pictures.document("issue-1").unwrap(),
        &document
    ));
    assert!(
        pictures.document("issue-2").is_none(),
        "it is the issue's that shows it"
    );
    assert_eq!(
        lock(&memory.fetched).clone(),
        [(paper.to_owned(), KEY.to_owned())]
    );
    // Another is read in its place: at most one is held.
    let second = pictures
        .open_pdf(&*memory, &key(), other, "issue-1")
        .unwrap();
    assert_eq!(second.sizes().len(), 2);
    assert!(Arc::ptr_eq(&pictures.document("issue-1").unwrap(), &second));
    // Closing for another issue lets nothing go; closing for its own does.
    pictures.close_pdf("issue-2");
    assert!(pictures.document("issue-1").is_some());
    pictures.close_pdf("issue-1");
    assert!(pictures.document("issue-1").is_none());
    // The issue put away lets the PDF go with its pictures.
    pictures
        .open_pdf(&*memory, &key(), paper, "issue-1")
        .unwrap();
    pictures.release("issue-1");
    assert!(pictures.document("issue-1").is_none());
}

#[test]
fn a_pdf_that_cannot_be_read_says_why_and_nothing_is_held() {
    let (pictures, _double) = store();
    let memory = Memory::with(SRC, png_of(4, 4, 0));
    // A picture is no PDF; an address that is not Linear's is not fetched, with the key or without;
    // a PDF that is cut off is not one that can be read; a file Linear does not give.
    assert_eq!(
        pictures.open_pdf(&*memory, &key(), SRC, "issue-1").err(),
        Some(Unread::Unreadable)
    );
    assert_eq!(
        pictures
            .open_pdf(&*memory, &key(), "https://example.com/a.pdf", "issue-1")
            .err(),
        Some(Unread::NotLinear)
    );
    let broken = "https://uploads.linear.app/ws/a/broken.pdf";
    memory.add(broken, Ok(b"%PDF-1.4\nnot really".to_vec()));
    assert_eq!(
        pictures.open_pdf(&*memory, &key(), broken, "issue-1").err(),
        Some(Unread::Unreadable)
    );
    assert_eq!(
        pictures
            .open_pdf(
                &*memory,
                &key(),
                "https://uploads.linear.app/ws/a/none.pdf",
                "issue-1"
            )
            .err(),
        Some(Unread::Unreachable)
    );
    assert!(pictures.document("issue-1").is_none());
    assert_eq!(
        lock(&memory.fetched).len(),
        3,
        "the other address was not fetched"
    );
    for why in [
        Unread::NotLinear,
        Unread::TooLarge,
        Unread::Unreachable,
        Unread::Unreadable,
        Unread::TooManyPages,
    ] {
        assert!(!why.words().contains("http"), "{why:?}");
    }
}

#[test]
fn where_no_view_holds_files_the_page_is_told_so() {
    let (pictures, double) = store();
    double.no_view();
    let memory = Memory::with(SRC, png_of(4, 4, 0));
    assert_eq!(
        pictures.show(&*memory, &key(), SRC, "issue-1"),
        Err(Why::NoView.into())
    );
}

#[test]
fn the_oldest_are_let_go_where_more_than_the_most_is_held() {
    let (pictures, double) = store();
    let memory = Arc::new(Memory::default());
    let at = |n: u32| format!("https://uploads.linear.app/ws/{n}");
    // Ten of seven megabytes, each its own (the padding says which): sixty-three fit, the tenth
    // makes the first go.
    let mut first = None;
    for n in 0..10u32 {
        let mut bytes = png_of(10, 10, 7 * 1024 * 1024 - 100);
        let last = bytes.len() - 1;
        bytes[last] = n as u8;
        memory.add(&at(n), Ok(bytes));
        let shown = pictures.show(&*memory, &key(), &at(n), "issue-1").unwrap();
        first.get_or_insert(shown.name);
    }
    assert_eq!(pictures.names().len(), 9);
    assert_eq!(double.detached(), [first.unwrap()]);
}

fn given(double: &Double, handle: &str, name: &str, bytes: &[u8]) -> Given {
    double.give(handle, bytes);
    Given {
        handle: handle.to_owned(),
        name: name.to_owned(),
    }
}

#[test]
fn the_files_the_person_gave_are_received_whole_and_sent_and_named_by_lines() {
    let (pictures, double) = store();
    let memory = Arc::new(Memory::default());
    *lock(&memory.status) = 200;
    // More than a piece (1 MiB), a picture and a file that is not one (a PDF, and bytes that are
    // nothing the program knows): the pictures are put as pictures and the others as links.
    let large = png_of(30, 20, 2 * 1024 * 1024 + 5);
    let one = given(&double, "01", "shot (1).png", &large);
    let two = given(&double, "02", "b.gif", b"GIF89a.......");
    let three = given(&double, "03", "report.pdf", b"%PDF-1.7\n");
    let four = given(&double, "04", "build log.txt", b"line one\nline two\n");
    let lines = pictures
        .send(&*memory, &key(), &[one, two, three, four], "posted")
        .unwrap();
    assert_eq!(
        lines,
        [
            "![shot 1.png](https://uploads.linear.app/stand-in/1)",
            "![b.gif](https://uploads.linear.app/stand-in/2)",
            "[report.pdf](https://uploads.linear.app/stand-in/3)",
            "[build log.txt](https://uploads.linear.app/stand-in/4)"
        ]
    );
    let put = lock(&memory.put);
    assert_eq!(put.len(), 4);
    assert_eq!(put[3].2, b"line one\nline two\n");
    let types: Vec<String> = put
        .iter()
        .map(|put| {
            put.1
                .iter()
                .find(|(name, _)| name == "Content-Type")
                .map(|(_, value)| value.clone())
                .unwrap()
        })
        .collect();
    assert_eq!(
        types,
        [
            "image/png",
            "image/gif",
            "application/pdf",
            "application/octet-stream"
        ]
    );
    assert_eq!(put[0].2, large);
    assert_eq!(put[0].0, "https://storage.stand-in/put/1?sig=SECRET");
    // Linear's headers go, with the type of the file in place of the one it named, and the key does not.
    let headers = &put[0].1;
    assert!(headers.contains(&("x-goog-meta-a".to_owned(), "b".to_owned())));
    assert!(headers.contains(&("Content-Type".to_owned(), "image/png".to_owned())));
    assert!(
        !headers
            .iter()
            .any(|(_, value)| value.contains(KEY) || value == "wrong")
    );
    assert_eq!(
        headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .count(),
        1
    );
    assert!(
        headers
            .iter()
            .any(|(name, value)| name == "Cache-Control" && value.contains("max-age=31536000"))
    );
}

#[test]
fn the_first_that_cannot_be_sent_ends_it_and_the_words_never_hold_the_address() {
    let (pictures, double) = store();
    let memory = Arc::new(Memory::default());
    *lock(&memory.status) = 200;
    let good = given(&double, "01", "a.png", &png_of(2, 2, 0));
    let empty = given(&double, "02", "b.txt", b"");
    let after = given(&double, "03", "c.png", &png_of(2, 2, 0));
    let failed = pictures
        .send(
            &*memory,
            &key(),
            &[good.clone(), empty, after.clone()],
            "posted",
        )
        .unwrap_err();
    assert_eq!(
        failed,
        SendFailed::Words("b.txt is empty, so nothing was posted".to_owned())
    );
    // Nothing after it was received or sent.
    assert_eq!(lock(&memory.put).len(), 1);
    assert!(!double.read().contains(&"03".to_owned()));
    // A file that is no longer there.
    let gone = Given {
        handle: "ff".to_owned(),
        name: "d.png".to_owned(),
    };
    assert_eq!(
        pictures
            .send(&*memory, &key(), &[gone], "made")
            .unwrap_err(),
        SendFailed::Words("d.png could not be read, so nothing was made".to_owned())
    );
    // Over the most (the terminal says so while it is received; none of it is kept).
    double.give_huge("04");
    let big = Given {
        handle: "04".to_owned(),
        name: "big.mov".to_owned(),
    };
    assert_eq!(
        pictures
            .send(&*memory, &key(), &[big], "added")
            .unwrap_err(),
        SendFailed::Words("big.mov is over 100 MiB, so nothing was added".to_owned())
    );
    // The storage says no: the words say so, and not where it was to be put.
    *lock(&memory.status) = 403;
    let SendFailed::Words(words) = pictures
        .send(&*memory, &key(), &[after], "posted")
        .unwrap_err()
    else {
        panic!("words")
    };
    assert!(words.contains("HTTP 403"), "{words}");
    assert!(
        !words.contains("SECRET") && !words.contains("storage.stand-in"),
        "{words}"
    );
}

#[test]
fn the_copy_of_a_file_that_is_sent_is_gone_when_it_has_been_sent_or_has_not() {
    let (pictures, double) = store();
    let memory = Arc::new(Memory::default());
    *lock(&memory.status) = 200;
    let seen = Arc::new(Mutex::new(Vec::<std::path::PathBuf>::new()));
    struct Watching(Arc<Memory>, Arc<Mutex<Vec<std::path::PathBuf>>>);
    impl Transport for Watching {
        fn post(&self, key: &Key, body: &str) -> Result<crate::linear::transport::Reply, Error> {
            self.0.post(key, body)
        }
        fn put(
            &self,
            url: &str,
            headers: &[(String, String)],
            file: &Path,
            size: u64,
        ) -> Result<u16, Error> {
            lock(&self.1).push(file.to_owned());
            self.0.put(url, headers, file, size)
        }
    }
    let one = given(&double, "01", "a.txt", b"words");
    let watching = Watching(Arc::clone(&memory), Arc::clone(&seen));
    pictures.send(&watching, &key(), &[one], "posted").unwrap();
    let seen = lock(&seen);
    assert_eq!(seen.len(), 1);
    assert!(!seen[0].exists(), "the file is gone");
    assert!(!seen[0].parent().unwrap().exists(), "and its folder");
}

#[test]
fn a_file_of_linear_is_saved_where_the_person_chose_from_a_copy_that_is_then_gone() {
    let (pictures, double) = store();
    let at = "https://uploads.linear.app/ws/a/build.log";
    let memory = Memory::with(at, b"the log\n".to_vec());
    let saved = pictures.save(&*memory, &key(), at, "place-1").unwrap();
    assert_eq!(saved, 8);
    assert_eq!(
        double.saved(),
        [("place-1".to_owned(), b"the log\n".to_vec())]
    );
    assert_eq!(
        lock(&memory.fetched).as_slice(),
        [(at.to_owned(), KEY.to_owned())]
    );
    // Nothing that is not Linear's is fetched, and the key stays.
    assert_eq!(
        pictures.save(&*memory, &key(), "https://evil.example/a", "place-2"),
        Err(Saved::NotLinear)
    );
    assert_eq!(lock(&memory.fetched).len(), 1);
    // Linear does not give it; the place is full; no view.
    assert_eq!(
        pictures.save(&*memory, &key(), "https://uploads.linear.app/ws/none", "p"),
        Err(Saved::Unreachable)
    );
    double.save_fails();
    assert_eq!(
        pictures.save(&*memory, &key(), at, "place-3"),
        Err(Saved::Refused)
    );
    double.no_view();
    assert_eq!(
        pictures.save(&*memory, &key(), at, "place-4"),
        Err(Saved::NoView)
    );
}

#[test]
fn the_test_picture_is_one_a_viewer_takes() {
    let bytes = super::double::png(6, 4, [10, 20, 30]);
    assert_eq!(
        crate::linear::images::Format::of(&bytes),
        Some(crate::linear::images::Format::Png)
    );
    let size =
        crate::linear::images::size_of(&bytes, crate::linear::images::Format::Png).expect("a size");
    assert_eq!((size.width, size.height), (6, 4));
}
