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
}

impl Error {
    /// The name the page is told: `refused`, `unreachable` or `failed` (`docs/calls.md`).
    #[must_use]
    #[allow(dead_code)] // The calls of the page say it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Refused => "refused",
            Self::Unreachable(_) => "unreachable",
            Self::Failed(_) => "failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused => out.write_str("Linear did not accept the key"),
            Self::Unreachable(why) => write!(out, "Linear could not be reached: {why}"),
            Self::Failed(why) => out.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

/// `text` as one short line of plain words: what Linear sends back is shown to the person, never
/// trusted, so control characters go and the length is held.
pub fn tidy(text: &str) -> String {
    const LONGEST: usize = 200;
    let plain: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
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
    }

    #[test]
    fn what_linear_says_is_one_short_plain_line() {
        assert_eq!(tidy("a\nb\u{1b}[31m  c"), "a b [31m c");
        assert_eq!(tidy(&"x".repeat(500)).chars().count(), 200);
    }
}
