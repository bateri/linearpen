//! What Linear, or the way to it, can answer instead of issues.

use std::fmt;

/// Why a question to Linear came back without its answer. The three are told apart because the
/// person does something different about each: gives another key, looks at the network, or waits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Linear did not accept the key (HTTP 401, or `AUTHENTICATION_ERROR`).
    Refused,
    /// There was no answer to read: `curl` is not there, or could not reach Linear, or ran out of time.
    Unreachable(String),
    /// Linear answered, but not with what was asked: GraphQL `errors`, another HTTP status, or a
    /// body that is not what the query says.
    Failed(String),
    /// Linear took the key and said it may not do this (HTTP 403, or an error of the kind
    /// `forbidden`): a key that can read and not write.
    Forbidden,
    /// A write was sent, and what came back does not say whether Linear made it (no answer, or
    /// an error), and asking for it afterwards did not settle it either.
    Unsure,
}

impl Error {
    /// The name the page is told: `refused`, `unreachable` or `failed` (`docs/calls.md`); a key that
    /// may not do what was asked is `failed` too, and the call that writes says it in its own words.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Unreachable(_) => "unreachable",
            Self::Failed(_) | Self::Forbidden | Self::Unsure => "failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused => out.write_str("Linear did not accept the key"),
            Self::Unreachable(why) => write!(out, "Linear could not be reached: {why}"),
            Self::Failed(why) => out.write_str(why),
            Self::Forbidden => out.write_str("Linear did not allow this key to do that"),
            Self::Unsure => out.write_str("it is not known whether Linear made it"),
        }
    }
}

impl std::error::Error for Error {}

/// `text` as one line with no control character in it (a terminal takes an escape sequence in what
/// is printed for a command), runs of white space made one space. The length is not held.
#[must_use]
pub fn plain(text: &str) -> String {
    let plain: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` as lines: the line breaks are kept (`\r\n` and a lone `\r` are one), and every other
/// control character, a tab and an escape that a terminal reads as a command among them, is made a
/// space. What Linear's descriptions and comments are printed
/// through.
#[must_use]
pub fn lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            '\n' => out.push(c),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// `text` as one short line of plain words: what Linear sends back is shown to the person, never
/// trusted, so control characters go and the length is held.
pub fn tidy(text: &str) -> String {
    const LONGEST: usize = 200;
    let plain = plain(text);
    if plain.chars().count() > LONGEST {
        let mut cut: String = plain.chars().take(LONGEST - 1).collect();
        cut.push('\u{2026}');
        cut
    } else {
        plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kinds_have_the_names_of_the_calls() {
        assert_eq!(Error::Refused.kind(), "refused");
        assert_eq!(Error::Unreachable(String::new()).kind(), "unreachable");
        assert_eq!(Error::Failed(String::new()).kind(), "failed");
        assert_eq!(Error::Forbidden.kind(), "failed");
        assert_eq!(Error::Unsure.kind(), "failed");
    }

    #[test]
    fn lines_keep_their_breaks_and_lose_every_control_character_else() {
        assert_eq!(lines("a\r\nb\rc\nd"), "a\nb\nc\nd");
        assert_eq!(lines("x\u{1b}[31mred\u{7}\u{9b}"), "x [31mred  ");
        assert_eq!(lines("a\tb"), "a b");
        assert!(!lines("\u{1b}]52;c;AAAA\u{7}\n\u{85}").contains(['\u{1b}', '\u{7}', '\u{85}']));
    }

    #[test]
    fn what_linear_says_is_one_short_plain_line() {
        assert_eq!(tidy("a\nb\u{1b}[31m  c"), "a b [31m c");
        assert_eq!(tidy(&"x".repeat(500)).chars().count(), 200);
    }
}
