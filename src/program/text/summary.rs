//! The issues as text: the two lists, grouped as the page groups them, one issue to a line.

use std::fmt::Write as _;

use crate::linear::types::Priority;
use crate::linear::{Issue, Listing, Viewer};
use crate::model::filter::Filter;
use crate::model::groups::group;

/// What the summary is made of.
pub struct Summary<'a> {
    pub viewer: &'a Viewer,
    pub mine: &'a Listing,
    pub all: &'a Listing,
}

/// "Nov 9" of "2026-11-09T14:20:00.000Z"; what is not a date is left as it came.
fn short_date(stamp: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = stamp.get(..10).unwrap_or(stamp).split('-');
    let _year = parts.next();
    let month = parts
        .next()
        .and_then(|m| m.parse::<usize>().ok())
        .and_then(|m| MONTHS.get(m.wrapping_sub(1)));
    let day = parts.next().and_then(|d| d.parse::<u32>().ok());
    match (month, day) {
        (Some(month), Some(day)) => format!("{month} {day}"),
        _ => stamp.to_owned(),
    }
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
        issue.title
    );
    if !issue.labels.is_empty() {
        let names: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
        let _ = write!(out, "  [{}]", names.join(", "));
    }
    if let Some(assignee) = &issue.assignee {
        let _ = write!(out, "  ({})", assignee.display_name);
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
        let _ = writeln!(out, "  {} ({})", group.name, group.issues.len());
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
            self.viewer.organization.name, self.viewer.name
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
    fn dates_are_the_month_and_the_day() {
        assert_eq!(short_date("2026-11-09T14:20:00.000Z"), "Nov 9");
        assert_eq!(short_date("2026-01-31T00:00:00Z"), "Jan 31");
        assert_eq!(short_date("soon"), "soon");
        assert_eq!(short_date("2026-13-01T00:00:00Z"), "2026-13-01T00:00:00Z");
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
}
