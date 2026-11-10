//! The API key, as a value that keeps itself out of text: it has no `Debug` that shows it and no
//! `Display`; the one way to the characters is [`Key::expose`], called where the key goes into the
//! stdin of `curl`.

use std::fmt;

/// The longest key taken. Linear's are some 50 characters.
const LONGEST: usize = 256;

/// Why a text is not an API key. The words never hold the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyError {
    Empty,
    TooLong,
    /// Something other than letters, digits, `_`, `-` and `.`: a space, a quote, a line break.
    BadCharacter,
}

impl fmt::Display for KeyError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Empty => "the key is empty",
            Self::TooLong => "the key is too long to be one",
            Self::BadCharacter => "the key has a character that no Linear API key has",
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Key(String);

impl Key {
    /// The key written in `text`, with the white space around it cut. What `curl` is given in its
    /// configuration is quoted by the rules of a config line; a key that needs no quoting at all
    /// cannot become another line of it.
    pub fn new(text: &str) -> Result<Self, KeyError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(KeyError::Empty);
        }
        if text.len() > LONGEST {
            return Err(KeyError::TooLong);
        }
        if !text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
        {
            return Err(KeyError::BadCharacter);
        }
        Ok(Self(text.to_owned()))
    }

    /// The characters. Only the transport asks for them.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Key(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_cut_of_the_space_around_it_and_kept() {
        let key = Key::new("  lin_api_Abc123-x.y\n").unwrap();
        assert_eq!(key.expose(), "lin_api_Abc123-x.y");
    }

    #[test]
    fn what_could_start_another_line_of_a_config_is_not_a_key() {
        assert_eq!(Key::new(""), Err(KeyError::Empty));
        assert_eq!(Key::new(" \n"), Err(KeyError::Empty));
        for bad in [
            "a b", "a\nurl=x", "a\"b", "a\\b", "a\u{0}b", "a\u{e9}b", "a=b",
        ] {
            assert_eq!(Key::new(bad), Err(KeyError::BadCharacter), "{bad:?}");
        }
        assert_eq!(Key::new(&"a".repeat(300)), Err(KeyError::TooLong));
    }

    #[test]
    fn the_key_is_not_in_its_debug_text() {
        let key = Key::new("lin_api_secret").unwrap();
        assert!(!format!("{key:?}").contains("secret"));
    }
}
