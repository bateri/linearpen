//! The filter bar: what is typed matches an issue when the text is in its title, its identifier or
//! the name of one of its labels, whatever the case.

use crate::linear::Issue;

/// The words typed, made ready to match: trimmed and lower-cased once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter(String);

impl Filter {
    #[must_use]
    pub fn new(typed: &str) -> Self {
        Self(typed.trim().to_lowercase())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn matches(&self, issue: &Issue) -> bool {
        if self.0.is_empty() {
            return true;
        }
        let has = |text: &str| text.to_lowercase().contains(&self.0);
        has(&issue.title) || has(&issue.identifier) || issue.labels.iter().any(|l| has(&l.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::groups::tests::issue;

    #[test]
    fn nothing_typed_matches_everything() {
        let it = issue("ACM-1", "Fix login", "Todo", "unstarted", 0.0, "2026-11-01");
        assert!(Filter::new("").matches(&it));
        assert!(Filter::new("   ").matches(&it));
        assert!(Filter::new("").is_empty());
    }

    #[test]
    fn title_identifier_and_label_match_by_substring_whatever_the_case() {
        let mut it = issue(
            "ACM-412",
            "Add Rate Limiting",
            "Todo",
            "unstarted",
            0.0,
            "2026-11-01",
        );
        it.labels.push(crate::linear::types::Label {
            name: "Security".to_owned(),
            color: "#30a46c".to_owned(),
        });
        assert!(Filter::new("rate lim").matches(&it));
        assert!(Filter::new("acm-4").matches(&it));
        assert!(Filter::new("  SECUR ").matches(&it));
        assert!(!Filter::new("billing").matches(&it));
        // What is not looked in: the state's name and the description.
        assert!(!Filter::new("todo").matches(&it));
    }

    #[test]
    fn letters_beyond_ascii_fold_too() {
        let it = issue(
            "ACM-1",
            "Ödeme sayfası",
            "Todo",
            "unstarted",
            0.0,
            "2026-11-01",
        );
        assert!(Filter::new("ÖDEME").matches(&it));
        assert!(Filter::new("ödeme").matches(&it));
    }
}
