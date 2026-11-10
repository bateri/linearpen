//! How a question gets to Linear: `curl`, and nothing else. There is no HTTP or TLS crate in the
//! program: the proxy, the certificates and the TLS of the machine are those of its `curl`.
//!
//! The key is never an argument of `curl` (which `ps` shows) and not in its environment: it goes
//! in the configuration that `curl --config -` reads from stdin, a line of it, with the address and
//! the body. The environment of the child is the program's less `LINEAR_API_KEY`.

use std::io::Write;
use std::process::{Command, Stdio};

use super::Key;
use super::error::Error;

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
}

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

/// What a `curl` that ended with `code` was unable to do, in a few words.
fn why(code: Option<i32>) -> String {
    match code {
        Some(5 | 6) => "the host name was not found".to_owned(),
        Some(7) => "the connection was refused".to_owned(),
        Some(28) => "the request ran out of time".to_owned(),
        Some(35 | 60) => "the TLS connection failed".to_owned(),
        Some(code) => format!("curl ended with status {code}"),
        None => "curl was stopped".to_owned(),
    }
}

impl Transport for Curl {
    fn post(&self, key: &Key, body: &str) -> Result<Reply, Error> {
        let config = config(&self.url, key, body)?;
        let mut child = Command::new("curl")
            .args(["--config", "-"])
            .env_remove("LINEAR_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|why| {
                Error::Unreachable(if why.kind() == std::io::ErrorKind::NotFound {
                    "curl is not installed (Linear is spoken to with curl)".to_owned()
                } else {
                    format!("curl did not start: {}", why.kind())
                })
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            // A curl that ended early says why in its status; the write fails then, and that is fine.
            let _ = stdin.write_all(config.as_bytes());
        }
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
