//! The bounds Linear's input is held to, in one place so that the page and the command line hold an
//! issue, a comment and an id to the same ones.

/// The longest title, in characters (Linear's own limit).
pub const TITLE_MOST: usize = 255;
/// The longest description of a new issue, in characters.
pub const DESCRIPTION_MOST: usize = 50_000;
/// The most labels one issue is given when it is made.
pub const LABELS_MOST: usize = 50;
/// The longest id or key taken (Linear's ids are UUIDs, 36 characters).
pub const REF_MOST: usize = 64;
/// The most characters of a comment that are posted.
pub const BODY_MOST: usize = 10_000;
/// The longest text an edit takes and gives, in characters: a description or a comment is edited
/// whole, and one that is longer is edited in Linear.
pub const EDIT_MOST: usize = 100_000;
/// The most points an estimate can be: Linear's scales end far below it.
pub const POINTS_MOST: u32 = 1_000;

/// Whether `text` is an id or a key of Linear: letters, digits, dashes and underscores, not too long.
#[must_use]
pub fn is_ref(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= REF_MOST
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `points` as a whole number of points, from 0 to `POINTS_MOST`, or nothing.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn whole_points(points: f64) -> Option<u32> {
    (points.fract() == 0.0 && (0.0..=f64::from(POINTS_MOST)).contains(&points))
        .then_some(points as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_letters_digits_dashes_and_underscores_up_to_the_most() {
        assert!(is_ref("ACM-412"));
        assert!(is_ref(&"a".repeat(REF_MOST)));
        assert!(!is_ref(""));
        assert!(!is_ref(&"a".repeat(REF_MOST + 1)));
        assert!(!is_ref("a b"));
        assert!(!is_ref("a/b"));
    }

    #[test]
    fn points_are_whole_numbers_up_to_the_most() {
        assert_eq!(whole_points(0.0), Some(0));
        assert_eq!(whole_points(5.0), Some(5));
        assert_eq!(whole_points(1_000.0), Some(1_000));
        assert_eq!(whole_points(1_001.0), None);
        assert_eq!(whole_points(2.5), None);
        assert_eq!(whole_points(-1.0), None);
    }
}
