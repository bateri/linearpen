//! The issues as text: the two lists, grouped as the page groups them, one issue to a line.

use std::fmt::Write as _;

use crate::linear::types::Priority;
use crate::linear::{Issue, Listing, Viewer, plain};
use crate::model::filter::Filter;
use crate::model::groups::group;
use crate::model::time::short_date;

/// What the summary is made of.
pub struct Summary<'a> {
    pub viewer: &'a Viewer,
    pub mine: &'a Listing,
    pub all: &'a Listing,
}

fn priority_words(priority: Priority) -> &'static str {
    match priority {
        Priority::None => "-",
        Priority::Urgent => "Urgent",
        Priority::High => "High",
        Priority::Medium => "Medium",
        Priority::Low => "Low",
    }
}

fn line(issue: &Issue, widest: usize) -> String {
    let mut out = format!(
        "    {:<widest$}  {:<6}  {:<6}  {}",
        issue.identifier,
        priority_words(issue.priority),
        short_date(&issue.updated_at),
        plain(&issue.title)
    );
    if !issue.labels.is_empty() {
        let names: Vec<String> = issue.labels.iter().map(|l| plain(&l.name)).collect();
        let _ = write!(out, "  [{}]", names.join(", "));
    }
    if let Some(assignee) = &issue.assignee {
        let _ = write!(out, "  ({})", plain(&assignee.display_name));
    }
    out
}

fn section(out: &mut String, title: &str, listing: &Listing, empty: &str) {
    let _ = writeln!(out, "{title} ({})", listing.issues.len());
    if listing.truncated {
        let _ = writeln!(
            out,
            "  Only the first {} are listed; there are more in Linear.",
            listing.issues.len()
        );
    }
    let groups = group(&listing.issues, &Filter::default());
    if groups.is_empty() {
        let _ = writeln!(out, "  {empty}");
        return;
    }
    let widest = listing
        .issues
        .iter()
        .map(|issue| issue.identifier.chars().count())
        .max()
        .unwrap_or(0);
    for group in groups {
        let _ = writeln!(out, "  {} ({})", plain(group.name), group.issues.len());
        for issue in group.issues {
            let _ = writeln!(out, "{}", line(issue, widest));
        }
    }
}

impl Summary<'_> {
    /// The text: who and where, then My issues, then All issues, with `note` last if there is one.
    #[must_use]
    pub fn text(&self, note: Option<&str>) -> String {
        let mut out = format!(
            "{} - {}\n\n",
            plain(&self.viewer.organization.name),
            plain(&self.viewer.name)
        );
        section(
            &mut out,
            "My issues",
            self.mine,
            "Nothing is assigned to you.",
        );
        out.push('\n');
        section(
            &mut out,
            "All issues",
            self.all,
            "There are no open issues.",
        );
        if let Some(note) = note {
            let _ = write!(out, "\n{note}\n");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::types::{Assignee, Label, Organization};
    use crate::model::groups::tests::issue;

    fn viewer() -> Viewer {
        Viewer {
            id: "u".to_owned(),
            name: "Ada".to_owned(),
            organization: Organization {
                name: "Acme".to_owned(),
                url_key: "acme".to_owned(),
            },
        }
    }

    #[test]
    fn the_text_has_both_lists_grouped_and_says_when_one_is_empty() {
        let mut a = issue(
            "ACM-412",
            "Add rate limiting",
            "In Progress",
            "started",
            1.0,
            "2026-11-09",
        );
        a.labels.push(Label {
            name: "api".to_owned(),
            color: "#3e63dd".to_owned(),
        });
        let mut b = issue(
            "ACM-7",
            "Plan the trip",
            "Backlog",
            "backlog",
            0.0,
            "2026-10-01",
        );
        b.assignee = Some(Assignee {
            display_name: "Deniz".to_owned(),
            is_me: false,
        });
        let all = Listing {
            issues: vec![a, b],
            truncated: false,
        };
        let viewer = viewer();
        let text = Summary {
            viewer: &viewer,
            mine: &Listing::default(),
            all: &all,
        }
        .text(Some("a note"));
        let expected = "\
Acme - Ada

My issues (0)
  Nothing is assigned to you.

All issues (2)
  In Progress (1)
    ACM-412  -       Nov 9   Add rate limiting  [api]
  Backlog (1)
    ACM-7    -       Oct 1   Plan the trip  (Deniz)

a note
";
        assert_eq!(text, expected);
    }

    #[test]
    fn a_cut_list_says_so() {
        let listing = Listing {
            issues: vec![issue("A-1", "t", "Todo", "unstarted", 0.0, "2026-11-01")],
            truncated: true,
        };
        let viewer = viewer();
        let text = Summary {
            viewer: &viewer,
            mine: &listing,
            all: &Listing::default(),
        }
        .text(None);
        assert!(text.contains("Only the first 1 are listed"), "{text}");
        assert!(text.contains("There are no open issues."));
    }

    #[test]
    fn what_linear_wrote_prints_without_a_control_character() {
        let mut a = issue(
            "ACM-1",
            "Fix \u{1b}]52;c;AAAA\u{7} the thing",
            "Todo",
            "unstarted",
            0.0,
            "2026-11-01",
        );
        a.labels.push(Label {
            name: "bug\u{1b}[31m".to_owned(),
            color: "#ff0000".to_owned(),
        });
        let mine = Listing {
            issues: vec![a],
            truncated: false,
        };
        let all = Listing {
            issues: Vec::new(),
            truncated: false,
        };
        let text = Summary {
            viewer: &viewer(),
            mine: &mine,
            all: &all,
        }
        .text(None);
        assert!(
            !text.contains('\u{1b}') && !text.contains('\u{7}'),
            "{text:?}"
        );
        assert!(text.contains("Fix ]52;c;AAAA the thing"), "{text:?}");
    }
}
