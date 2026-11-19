//! How a question gets to Linear: `curl`, and nothing else. There is no HTTP or TLS crate in the
//! program: the proxy, the certificates and the TLS of the machine are those of its `curl`.
//!
//! The key is never an argument of `curl` (which `ps` shows) and not in its environment: it goes
//! in the configuration that `curl -q --config -` reads from stdin, a line of it, with the address and
//! the body. (`-q` is first, so that no `.curlrc` of the person's is read; every `curl` of the program
//! is made by `curl_command`, which is the one place its arguments are.) The environment of the child
//! is the program's less `LINEAR_API_KEY`.
//!
//! The key goes to two hosts and no other: `api.linear.app` (the questions) and, to fetch a file,
//! `uploads.linear.app` (`fetch` and `fetch_to`, which refuse any other address). A redirect of a
//! fetch is followed by the program, not by `curl`: one step at a time, over https only, and the key is
//! given in a step only where its address is one of Linear's files. A file is sent to the storage
//! Linear names for it with `put`, which has no key to give: the address it is sent to is signed.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use super::Key;
use super::error::Error;
use super::images;

/// Linear's GraphQL endpoint. `LINEARPEN_API_URL` replaces it, for tests only.
pub const DEFAULT_URL: &str = "https://api.linear.app/graphql";

/// What came back: the HTTP status and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

/// Sends a request body to Linear with a key and gives back the reply. It is `Err` only where there
/// is no reply to read (`Error::Unreachable`); what a reply says is for the client to read.
pub trait Transport: Send + Sync {
    fn post(&self, key: &Key, body: &str) -> Result<Reply, Error>;

    /// The picture at `url`, an address of `https://uploads.linear.app/`, fetched with the key:
    /// its bytes, at most `images::BYTES_MOST` of them (`Failed(TOO_LARGE)` where there are more). An
    /// address that is not one of Linear's files is not fetched.
    fn fetch(&self, key: &Key, url: &str) -> Result<Vec<u8>, Error> {
        let _ = (key, url);
        Err(Error::Unreachable(
            "this transport fetches no pictures".to_owned(),
        ))
    }

    /// The file at `url`, an address of `https://uploads.linear.app/`, fetched with the key into the
    /// new file `to`, at most `most` bytes (`Failed(TOO_LARGE)` where there are more, and `to` is then
    /// gone): how many bytes it is. An address that is not one of Linear's files is not fetched.
    fn fetch_to(&self, key: &Key, url: &str, to: &Path, most: u64) -> Result<u64, Error> {
        let _ = (key, url, to, most);
        Err(Error::Unreachable(
            "this transport fetches no files".to_owned(),
        ))
    }

    /// Sends the file `file` (of `size` bytes: a copy the program made, see `Staged`) to `url` (the
    /// address Linear signed for it) as a `PUT` with `headers`, and gives the HTTP status. The key is
    /// not sent: the address is the permission.
    fn put(
        &self,
        url: &str,
        headers: &[(String, String)],
        file: &Path,
        size: u64,
    ) -> Result<u16, Error> {
        let _ = (url, headers, file, size);
        Err(Error::Unreachable(
            "this transport sends no files".to_owned(),
        ))
    }
}

/// What `fetch` says of a file larger than it was told to take.
pub const TOO_LARGE: &str = "the file is too large";

/// What `fetch` says of an address that is not one of Linear's files.
pub const NOT_UPLOADS: &str = "the address is not one of Linear's files";

/// `curl`, found on the `PATH`.
pub struct Curl {
    url: String,
}

impl Curl {
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

/// `text` as a quoted value of a curl config line: `\` and `"` escaped, a line break as the
/// escape `\n` (which curl reads back as the character, inside the value, never as the end of the
/// line). A control character that has no escape is refused: it could not be said at all.
fn quote(text: &str) -> Result<String, Error> {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{b}' => out.push_str("\\v"),
            c if c.is_control() => {
                return Err(Error::Failed(
                    "the request has a character that cannot be sent".to_owned(),
                ));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    Ok(out)
}

/// The configuration `curl --config -` reads: the options, the address, the key as a header, the
/// body, and the HTTP status written after it on a line of its own.
pub fn config(url: &str, key: &Key, body: &str) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str("silent\nmax-time = 30\nconnect-timeout = 10\n");
    out.push_str(&format!("url = {}\n", quote(url)?));
    out.push_str(&format!(
        "header = {}\n",
        quote(&format!("Authorization: {}", key.expose()))?
    ));
    out.push_str("header = \"Content-Type: application/json\"\n");
    out.push_str(&format!("data-raw = {}\n", quote(body)?));
    out.push_str("write-out = \"\\n%{http_code}\"\n");
    Ok(out)
}

/// The body and the status of what `curl` wrote: the body, a line break, the status.
pub fn reply_of(stdout: &str) -> Result<Reply, Error> {
    let unread = || Error::Unreachable("curl wrote no HTTP status".to_owned());
    let (body, code) = stdout.rsplit_once('\n').ok_or_else(unread)?;
    let status = code.trim().parse::<u16>().map_err(|_| unread())?;
    if status == 0 {
        return Err(unread());
    }
    Ok(Reply {
        status,
        body: body.to_owned(),
    })
}

// What an `Unreachable` says where the request never left the machine: no curl, no host, no
// connection, no TLS. Nothing was sent then, so a write that failed so was not made.
const NO_CURL: &str = "curl is not installed (Linear is spoken to with curl)";
const NOT_STARTED: &str = "curl did not start: ";
const NO_HOST: &str = "the host name was not found";
const NO_CONNECTION: &str = "the connection was refused";
const NO_TLS: &str = "the TLS connection failed";

/// Whether the words of an `Unreachable` say that the request never left the machine, so that a
/// write that failed so was not made. Every other `Unreachable` (a request that ran out of time, a
/// curl that was stopped) may have been sent, and what it asked may have been done.
#[must_use]
pub fn never_sent(why: &str) -> bool {
    [NO_CURL, NO_HOST, NO_CONNECTION, NO_TLS].contains(&why) || why.starts_with(NOT_STARTED)
}

/// What a `curl` that ended with `code` was unable to do, in a few words.
fn why(code: Option<i32>) -> String {
    match code {
        Some(5 | 6) => NO_HOST.to_owned(),
        Some(7) => NO_CONNECTION.to_owned(),
        Some(28) => "the request ran out of time".to_owned(),
        Some(35 | 60) => NO_TLS.to_owned(),
        Some(code) => format!("curl ended with status {code}"),
        None => "curl was stopped".to_owned(),
    }
}

/// The arguments of every `curl` the program runs: `-q` first (no `.curlrc` is read, which could add
/// options of its own, a proxy or a header, to what is sent with the key), then the configuration from
/// stdin.
pub const CURL_ARGS: [&str; 3] = ["-q", "--config", "-"];

/// The one `curl` command of the program: the arguments above, and an environment that is the
/// program's less `LINEAR_API_KEY`.
fn curl_command() -> Command {
    let mut command = Command::new("curl");
    command.args(CURL_ARGS).env_remove("LINEAR_API_KEY");
    command
}

/// A `curl` that reads its configuration from stdin, which is given `config`.
fn start(config: &str) -> Result<Child, Error> {
    let mut child = curl_command()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| {
            Error::Unreachable(if why.kind() == std::io::ErrorKind::NotFound {
                NO_CURL.to_owned()
            } else {
                format!("{NOT_STARTED}{}", why.kind())
            })
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        // A curl that ended early says why in its status; the write fails then, and that is fine.
        let _ = stdin.write_all(config.as_bytes());
    }
    Ok(child)
}

/// How long a transfer of `size` bytes may take: a minute, and a second for each 100 kB (a megabit a
/// second, for a connection that is slow and a file that is large).
fn time_for(size: u64) -> u64 {
    60 + size / 100_000
}

/// The most steps of a fetch: the address asked, and the places it was sent on to.
const STEPS: usize = 5;

/// The longest address a redirect may name, and the most `curl` writes after a body (the status and the
/// address of a redirect, which the program holds back from the file until it knows what they say).
const REDIRECT_MOST: usize = 4096;
const TRAILER_MOST: usize = 128 * 1024;

/// The configuration that fetches the file at `url`: only over https, no redirect followed (the
/// program follows it, step by step, and gives the key only where `key` is `Some`), no more than
/// `most` bytes, and no body for an HTTP error. After the body `curl` writes a line break, the HTTP
/// status, a line break, the address a redirect names (none otherwise) and a line break.
pub fn fetch_config(url: &str, key: Option<&Key>, most: u64) -> Result<String, Error> {
    let mut out = String::new();
    out.push_str(&format!(
        "silent\nmax-time = {}\nconnect-timeout = 10\n",
        time_for(most)
    ));
    out.push_str("proto = \"=https\"\n");
    out.push_str(&format!("max-filesize = {most}\n"));
    out.push_str("globoff\nfail\n");
    out.push_str(&format!("url = {}\n", quote(url)?));
    if let Some(key) = key {
        out.push_str(&format!(
            "header = {}\n",
            quote(&format!("Authorization: {}", key.expose()))?
        ));
    }
    out.push_str("write-out = \"\\n%{http_code}\\n%{redirect_url}\\n\"\n");
    Ok(out)
}

/// Whether `name` is a header's name: letters, digits and dashes.
fn is_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// The configuration that sends the file at `path`, of `size` bytes, to `url`: a `PUT` over https, the
/// headers Linear named (and no other that is the person's: no `Authorization` of any kind), no
/// redirect followed. It has no key to hold: the function is not given one.
pub fn put_config(
    url: &str,
    headers: &[(String, String)],
    path: &Path,
    size: u64,
) -> Result<String, Error> {
    if !url.starts_with("https://") || url.len() > 4096 || url.chars().any(char::is_control) {
        return Err(Error::Failed(
            "the place to send the picture to is not an https address".to_owned(),
        ));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "silent\nmax-time = {}\nconnect-timeout = 10\nproto = \"=https\"\ngloboff\n",
        60 + time_for(size)
    ));
    out.push_str(&format!("url = {}\n", quote(url)?));
    out.push_str("request = \"PUT\"\n");
    for (name, value) in headers {
        // (A line break or any other control character in a header is refused, not escaped: Linear's
        // storage names plain headers.)
        if !is_header_name(name)
            || name.eq_ignore_ascii_case("authorization")
            || value.chars().any(char::is_control)
        {
            return Err(Error::Failed(
                "the storage asked for a header that is not sent".to_owned(),
            ));
        }
        out.push_str(&format!(
            "header = {}\n",
            quote(&format!("{name}: {value}"))?
        ));
    }
    out.push_str(&format!(
        "upload-file = {}\n",
        quote(&path.to_string_lossy())?
    ));
    out.push_str("output = \"/dev/null\"\nwrite-out = \"\\n%{http_code}\"\n");
    Ok(out)
}

/// Where a fetched file goes. It can be started again: a redirect has a body of its own, which is not the
/// file.
trait Sink: Write {
    fn restart(&mut self) -> std::io::Result<()>;
}

impl Sink for Vec<u8> {
    fn restart(&mut self) -> std::io::Result<()> {
        self.clear();
        Ok(())
    }
}

impl Sink for fs::File {
    fn restart(&mut self) -> std::io::Result<()> {
        self.set_len(0)?;
        self.seek(SeekFrom::Start(0)).map(|_| ())
    }
}

/// What a step of a fetch came to.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// The file, so many bytes of it.
    Done(u64),
    /// The place it was sent on to.
    Redirect(String),
}

/// What `curl` wrote after the body, and where the body ends in `tail`: a line break, the status, a line
/// break, the address of a redirect (none otherwise) and a line break. Nil where it is not so.
fn trailer_of(tail: &[u8]) -> Option<(usize, u16, String)> {
    let without = tail.strip_suffix(b"\n")?;
    let address_at = without.iter().rposition(|byte| *byte == b'\n')? + 1;
    let before = &without[..address_at - 1];
    let status_at = before.iter().rposition(|byte| *byte == b'\n')? + 1;
    let status = std::str::from_utf8(&before[status_at..])
        .ok()?
        .parse::<u16>()
        .ok()?;
    let address = String::from_utf8_lossy(&without[address_at..]).into_owned();
    Some((status_at - 1, status, address))
}

/// What a status and the address it names make of a step: the file is there (2xx), or it is sent on to
/// an https address that is not too long (3xx), or it is not given.
fn next_of(status: u16, redirect: &str) -> Result<Option<String>, Error> {
    match status {
        200..=299 => Ok(None),
        300..=399 if !redirect.is_empty() => {
            let https = redirect
                .get(..8)
                .is_some_and(|start| start.eq_ignore_ascii_case("https://"));
            if !https || redirect.len() > REDIRECT_MOST || redirect.chars().any(char::is_control) {
                Err(Error::Failed(
                    "Linear sent the file on to a place that is not an https address".to_owned(),
                ))
            } else {
                Ok(Some(redirect.to_owned()))
            }
        }
        _ => Err(Error::Failed("Linear did not give the file".to_owned())),
    }
}

/// One request for `url`, with the key where `key` is given: what it brings goes to `sink`, no more than
/// `most` bytes of it (and, held back until its end, what `curl` writes after it).
fn step(key: Option<&Key>, url: &str, most: u64, sink: &mut dyn Sink) -> Result<Step, Error> {
    let mut child = start(&fetch_config(url, key, most)?)?;
    let mut read: u64 = 0;
    let mut written: u64 = 0;
    let mut tail: Vec<u8> = Vec::new();
    let mut gone = None;
    if let Some(stdout) = child.stdout.take() {
        let mut stdout = stdout.take(most + TRAILER_MOST as u64 + 1);
        let mut piece = vec![0u8; 1 << 16];
        loop {
            let got = match stdout.read(&mut piece) {
                Ok(0) | Err(_) => break,
                Ok(got) => got,
            };
            read += got as u64;
            tail.extend_from_slice(&piece[..got]);
            if tail.len() > TRAILER_MOST {
                // What is before the last bytes is the body, whatever comes after it.
                let flush = tail.len() - TRAILER_MOST;
                written += flush as u64;
                if written > most {
                    break;
                }
                if sink.write_all(&tail[..flush]).is_err() {
                    gone = Some(Error::Unreachable(
                        "the file could not be written here".to_owned(),
                    ));
                    break;
                }
                tail.drain(..flush);
            }
        }
    }
    if read > most + TRAILER_MOST as u64 || written > most || gone.is_some() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(gone.unwrap_or_else(|| Error::Failed(TOO_LARGE.to_owned())));
    }
    let status = child
        .wait()
        .map_err(|why| Error::Unreachable(format!("curl did not end: {}", why.kind())))?;
    match status.code() {
        Some(0) => {}
        // `max-filesize`: the file said how large it is, and it is more than is taken.
        Some(63) => return Err(Error::Failed(TOO_LARGE.to_owned())),
        // `fail`: Linear answered with an HTTP error.
        Some(22) => return Err(Error::Failed("Linear did not give the file".to_owned())),
        code => return Err(Error::Unreachable(why(code))),
    }
    let (body_end, code, redirect) = trailer_of(&tail)
        .ok_or_else(|| Error::Unreachable("curl wrote no HTTP status".to_owned()))?;
    match next_of(code, &redirect)? {
        Some(next) => {
            // The body of a redirect is not the file.
            if written > 0 {
                sink.restart().map_err(|_| {
                    Error::Unreachable("the file could not be written here".to_owned())
                })?;
            }
            Ok(Step::Redirect(next))
        }
        None => {
            written += body_end as u64;
            if written > most {
                return Err(Error::Failed(TOO_LARGE.to_owned()));
            }
            sink.write_all(&tail[..body_end])
                .map_err(|_| Error::Unreachable("the file could not be written here".to_owned()))?;
            Ok(Step::Done(written))
        }
    }
}

/// The key a step of a fetch at `url` is given: the key where `url` is one of Linear's files, and none
/// anywhere else (a place a redirect sent the fetch on to).
fn key_for<'a>(key: &'a Key, url: &str) -> Option<&'a Key> {
    images::is_upload(url).then_some(key)
}

/// Fetches the file at `url` (one of Linear's) and gives what it brings to `sink`, no more than `most`
/// bytes: how many. A redirect is followed here, a step at a time, over https only and at most `STEPS`
/// requests in all; the key goes in a step only where the address of the step is one of Linear's files,
/// and any other place is asked without it.
fn stream(key: &Key, url: &str, most: u64, sink: &mut dyn Sink) -> Result<u64, Error> {
    if !images::is_upload(url) {
        return Err(Error::Failed(NOT_UPLOADS.to_owned()));
    }
    let mut at = url.to_owned();
    for _ in 0..STEPS {
        match step(key_for(key, &at), &at, most, sink)? {
            Step::Done(total) => return Ok(total),
            Step::Redirect(next) => at = next,
        }
    }
    Err(Error::Failed(
        "Linear sent the file on too many times".to_owned(),
    ))
}

impl Transport for Curl {
    fn fetch(&self, key: &Key, url: &str) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        stream(key, url, images::BYTES_MOST, &mut bytes)?;
        Ok(bytes)
    }

    fn fetch_to(&self, key: &Key, url: &str, to: &Path, most: u64) -> Result<u64, Error> {
        if !images::is_upload(url) {
            return Err(Error::Failed(NOT_UPLOADS.to_owned()));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(to)
            .map_err(|why| Error::Unreachable(format!("{NOT_STARTED}{}", why.kind())))?;
        let done = stream(key, url, most, &mut file);
        drop(file);
        if done.is_err() {
            let _ = fs::remove_file(to);
        }
        done
    }

    fn put(
        &self,
        url: &str,
        headers: &[(String, String)],
        file: &Path,
        size: u64,
    ) -> Result<u16, Error> {
        let child = start(&put_config(url, headers, file, size)?)?;
        let out = child
            .wait_with_output()
            .map_err(|why| Error::Unreachable(format!("curl did not end: {}", why.kind())))?;
        if !out.status.success() {
            return Err(Error::Unreachable(why(out.status.code())));
        }
        reply_of(&String::from_utf8_lossy(&out.stdout)).map(|reply| reply.status)
    }

    fn post(&self, key: &Key, body: &str) -> Result<Reply, Error> {
        let config = config(&self.url, key, body)?;
        let child = start(&config)?;
        let out = child
            .wait_with_output()
            .map_err(|why| Error::Unreachable(format!("curl did not end: {}", why.kind())))?;
        if !out.status.success() {
            return Err(Error::Unreachable(why(out.status.code())));
        }
        reply_of(&String::from_utf8_lossy(&out.stdout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_that_never_left_the_machine_is_told_from_one_that_may_have() {
        for code in [5, 6, 7, 35, 60] {
            assert!(never_sent(&why(Some(code))), "{code}");
        }
        for code in [Some(28), Some(52), Some(56), None] {
            assert!(!never_sent(&why(code)), "{code:?}");
        }
        assert!(never_sent(NO_CURL));
        assert!(never_sent(&format!("{NOT_STARTED}permission denied")));
    }

    #[test]
    fn the_key_is_a_line_of_the_config_and_the_body_is_quoted() {
        let key = Key::new("lin_api_k").unwrap();
        let body = r#"{"query":"query A { x }","variables":{"a":"b\n\u00e9"}}"#;
        let config = config(DEFAULT_URL, &key, body).unwrap();
        assert!(config.contains("header = \"Authorization: lin_api_k\"\n"));
        assert!(config.contains(&format!("url = \"{DEFAULT_URL}\"\n")));
        // Every backslash and quote of the body is escaped, so curl reads the body back whole.
        assert!(config.contains(
            r#"data-raw = "{\"query\":\"query A { x }\",\"variables\":{\"a\":\"b\\n\\u00e9\"}}""#
        ));
        assert!(config.ends_with("write-out = \"\\n%{http_code}\"\n"));
        // One option to a line: the lines are the ones written here, none made by the body.
        assert_eq!(config.lines().count(), 8);
    }

    #[test]
    fn a_line_break_in_a_value_cannot_make_another_option() {
        let key = Key::new("k").unwrap();
        let config = config("http://x/\nurl = \"http://evil/\"", &key, "{}").unwrap();
        assert!(config.contains(r#"url = "http://x/\nurl = \"http://evil/\"""#));
        assert_eq!(config.lines().count(), 8);
        assert!(super::config("a\u{0}b", &key, "{}").is_err());
    }

    #[test]
    fn every_curl_has_the_same_arguments_and_there_is_one_place_that_starts_it() {
        // `-q` first: no `.curlrc` is read.
        assert_eq!(CURL_ARGS, ["-q", "--config", "-"]);
        let command = curl_command();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, ["-q", "--config", "-"]);
        // The program starts `curl` here and nowhere else: a source file that names it as a command
        // elsewhere would run it with arguments of its own.
        let mut found = Vec::new();
        let mut folders = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let command = ["Command::new(", "\"curl\"", ")"].concat();
                    for (n, line) in text.lines().enumerate() {
                        if line.contains(&command) && !line.trim_start().starts_with("//") {
                            found.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        // (This test's own lines say it in pieces, so that they are not counted.)
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("transport.rs"), "{found:?}");
    }

    #[test]
    fn a_picture_is_fetched_over_https_with_the_key_and_no_redirect_is_followed_by_curl() {
        let key = Key::new("lin_api_k").unwrap();
        let url = "https://uploads.linear.app/ws/a/b.png";
        let config = fetch_config(url, Some(&key), images::BYTES_MOST).unwrap();
        assert!(config.contains(&format!("url = \"{url}\"\n")));
        assert!(config.contains("header = \"Authorization: lin_api_k\"\n"));
        assert_eq!(config.matches("Authorization").count(), 1);
        for line in [
            "proto = \"=https\"",
            "globoff",
            "fail",
            "max-filesize = 8388608",
            "write-out = \"\\n%{http_code}\\n%{redirect_url}\\n\"",
        ] {
            assert!(config.lines().any(|each| each == line), "{line}\n{config}");
        }
        // `curl` follows nothing: not `location`, not `location-trusted`, no `proto-redir`, no `max-redirs`.
        for option in ["location", "proto-redir", "max-redirs"] {
            assert!(!config.contains(option), "{option}");
        }
        assert!(!config.contains("data"));
        // A larger file is more time and its own bound; the key is as alone as it was.
        let large = fetch_config(url, Some(&key), 100 * 1024 * 1024).unwrap();
        assert!(large.lines().any(|each| each == "max-filesize = 104857600"));
        assert!(
            large.lines().any(|each| each == "max-time = 1108"),
            "{large}"
        );
        assert_eq!(large.matches("Authorization").count(), 1);
    }

    #[test]
    fn a_redirect_to_another_host_is_asked_without_the_key() {
        let key = Key::new("lin_api_k").unwrap();
        // A step at a place that is not Linear's files has no key to give: the program gives it only
        // where the address is one of them.
        for url in [
            "https://cdn.example/file.png",
            "https://uploads.linear.app.evil.example/a.png",
            "https://uploads.linear.app@evil.example/a.png",
            "https://uploads.linear.app:444/a.png",
        ] {
            let given = key_for(&key, url);
            assert!(given.is_none(), "{url}");
            let config = fetch_config(url, given, images::BYTES_MOST).unwrap();
            assert!(
                !config.to_ascii_lowercase().contains("authorization"),
                "{url}\n{config}"
            );
            assert!(!config.contains("lin_api_k"));
        }
        assert!(key_for(&key, "https://uploads.linear.app/ws/b.png").is_some());
    }

    #[test]
    fn what_curl_writes_after_the_body_says_where_it_ends_and_where_a_redirect_goes() {
        let ended = trailer_of(b"a body\nof lines\n200\n\n").unwrap();
        assert_eq!(ended, (15, 200, String::new()));
        assert_eq!(&b"a body\nof lines\n200\n\n"[..15], b"a body\nof lines");
        assert_eq!(
            trailer_of(b"\n302\nhttps://cdn.example/a?b=c\n").unwrap(),
            (0, 302, "https://cdn.example/a?b=c".to_owned())
        );
        assert_eq!(trailer_of(b"\n200\n\n").unwrap().0, 0);
        assert!(trailer_of(b"no trailer").is_none());
        assert!(trailer_of(b"body\nnot a number\n\n").is_none());
        // 2xx is the file; 3xx with an https address that is not long goes on; anything else is not given.
        assert_eq!(next_of(200, "").unwrap(), None);
        assert_eq!(
            next_of(302, "https://cdn.example/a").unwrap(),
            Some("https://cdn.example/a".to_owned())
        );
        assert_eq!(
            next_of(307, "HTTPS://cdn.example/a").unwrap(),
            Some("HTTPS://cdn.example/a".to_owned())
        );
        for (status, address) in [
            (302, "http://cdn.example/a"),
            (302, "ftp://cdn.example/a"),
            (302, "file:///etc/passwd"),
            (302, "//cdn.example/a"),
            (302, ""),
            (302, "https://x/\u{1}"),
            (304, ""),
            (403, ""),
            (100, ""),
        ] {
            assert!(next_of(status, address).is_err(), "{status} {address}");
        }
        let long = format!("https://cdn.example/{}", "a".repeat(REDIRECT_MOST));
        assert!(next_of(302, &long).is_err());
    }

    #[test]
    fn an_address_that_is_not_linears_is_not_fetched_and_the_key_does_not_leave() {
        let key = Key::new("lin_api_k").unwrap();
        for url in [
            "https://evil.example/a.png",
            "https://uploads.linear.app.evil.example/a.png",
            "https://uploads.linear.app@evil.example/a.png",
            "https://uploads.linear.app:444/a.png",
            "http://uploads.linear.app/a.png",
            "https://api.linear.app/graphql",
        ] {
            // (Refused before any process is started.)
            let got = Curl::new(DEFAULT_URL).fetch(&key, url);
            assert_eq!(got, Err(Error::Failed(NOT_UPLOADS.to_owned())), "{url}");
        }
    }

    #[test]
    fn the_file_is_sent_with_the_headers_named_and_no_key() {
        let path = std::path::Path::new("/tmp/folder/picture");
        let headers = vec![
            ("x-goog-meta-a".to_owned(), "b".to_owned()),
            ("Content-Type".to_owned(), "image/png".to_owned()),
        ];
        let config =
            put_config("https://storage.example/put?sig=abc", &headers, path, 1000).unwrap();
        for line in [
            "request = \"PUT\"",
            "proto = \"=https\"",
            "globoff",
            "header = \"x-goog-meta-a: b\"",
            "header = \"Content-Type: image/png\"",
            "upload-file = \"/tmp/folder/picture\"",
        ] {
            assert!(config.lines().any(|each| each == line), "{line}\n{config}");
        }
        // No key is given to this, and none of its kind is asked for by the storage.
        assert!(!config.to_ascii_lowercase().contains("authorization"));
        assert!(!config.contains("location"), "no redirect is followed");
        // The time it may take grows with the file.
        let big = put_config(
            "https://storage.example/x",
            &headers,
            path,
            100 * 1024 * 1024,
        )
        .unwrap();
        assert!(big.lines().any(|each| each == "max-time = 1168"), "{big}");
        let key_header = vec![("Authorization".to_owned(), "lin_api_k".to_owned())];
        assert!(put_config("https://storage.example/x", &key_header, path, 1).is_err());
        let spaced = vec![("a b".to_owned(), "c".to_owned())];
        assert!(put_config("https://storage.example/x", &spaced, path, 1).is_err());
        // A line break, a carriage return or any control character in a header is refused, not
        // escaped.
        for value in [
            "c\nurl = \"http://evil/\"",
            "c\r\nX: y",
            "c\u{0}d",
            "c\td",
            "c\u{7f}d",
        ] {
            let broken = vec![("a".to_owned(), value.to_owned())];
            assert!(
                put_config("https://storage.example/x", &broken, path, 1).is_err(),
                "{value:?}"
            );
        }
        assert!(
            put_config(
                "https://storage.example/x\nurl = \"http://evil/\"",
                &headers,
                path,
                1
            )
            .is_err()
        );
        assert!(put_config("http://storage.example/x", &headers, path, 1).is_err());
    }

    #[test]
    fn the_reply_is_the_body_and_the_status_after_it() {
        assert_eq!(
            reply_of("{\"data\":null}\n200").unwrap(),
            Reply {
                status: 200,
                body: "{\"data\":null}".to_owned()
            }
        );
        // A body of several lines keeps them.
        assert_eq!(reply_of("a\nb\n401").unwrap().body, "a\nb");
        assert!(reply_of("no status").is_err());
        assert!(reply_of("{}\n000").is_err());
    }
}
