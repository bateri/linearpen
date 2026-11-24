//! The list of projects: grouped by their state (the type of the state first: started, planned, paused,
//! backlog, completed, canceled; then its place among those of its type), in a group the one whose target
//! comes first first, those with none last, then by name. And their days as Linear shows them, by their
//! resolution (a day, a month, a quarter, a half, a year), and whether one is late, as of a day given.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::linear::projects::Project;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// One group: the projects of a state.
#[derive(Debug, Clone, PartialEq)]
pub struct Group<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub position: f64,
    pub projects: Vec<&'a Project>,
}

impl Group<'_> {
    /// Whether the group begins folded: what is done, and what was let go of.
    #[must_use]
    pub fn folded_at_first(&self) -> bool {
        matches!(self.kind, "completed" | "canceled")
    }
}

/// Where the type of a state stands among the others.
fn rank(kind: &str) -> u8 {
    match kind {
        "started" => 0,
        "planned" => 1,
        "paused" => 2,
        "backlog" => 3,
        "completed" => 4,
        "canceled" => 5,
        _ => 6,
    }
}

/// Whether a project of a state of this type is done or let go of.
#[must_use]
pub fn closed(kind: &str) -> bool {
    matches!(kind, "completed" | "canceled")
}

/// `2026-12-12` as its parts, where it is one.
fn parts(date: &str) -> Option<(i32, u32, u32)> {
    let mut parts = date.get(..10)?.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts
        .next()?
        .parse()
        .ok()
        .filter(|m| (1..=12).contains(m))?;
    let day = parts
        .next()?
        .parse()
        .ok()
        .filter(|d| (1..=31).contains(d))?;
    Some((year, month, day))
}

/// Whether `text` is a day written as Linear writes one, `2026-11-24`, and nothing more.
#[must_use]
pub fn is_day(text: &str) -> bool {
    text.len() == 10
        && text.bytes().enumerate().all(|(at, byte)| {
            if at == 4 || at == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
        && parts(text).is_some()
}

/// The year of the day `today` (`2026-11-24`), or 0.
#[must_use]
pub fn year_of(today: &str) -> i32 {
    today
        .get(..4)
        .and_then(|year| year.parse().ok())
        .unwrap_or(0)
}

/// The day `date` is shown as, by its `resolution`: `Dec 12`, `Dec`, `Q4`, `H2` or `2027`, with the year
/// after it where it is not `this_year` (`Dec 2027`, `Q1 2027`). What is not a day is left as it came.
#[must_use]
pub fn when(date: &str, resolution: Option<&str>, this_year: i32) -> String {
    let Some((year, month, day)) = parts(date) else {
        return date.to_owned();
    };
    let name = MONTHS[(month - 1) as usize];
    let with_year = |words: String| {
        if year == this_year {
            words
        } else {
            format!("{words} {year}")
        }
    };
    match resolution {
        Some("year") => year.to_string(),
        Some("halfYear") => with_year(format!("H{}", if month <= 6 { 1 } else { 2 })),
        Some("quarter") => with_year(format!("Q{}", (month - 1) / 3 + 1)),
        Some("month") => with_year(name.to_owned()),
        _ if year == this_year => format!("{name} {day}"),
        _ => format!("{name} {day} {year}"),
    }
}

/// The last day of the month of `year`, `month`.
fn last_day(year: i32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The last day a target of `date` at `resolution` stands for: the day, or the end of its month,
/// quarter, half or year.
fn end_of(date: &str, resolution: Option<&str>) -> Option<(i32, u32, u32)> {
    let (year, month, day) = parts(date)?;
    let month_end = |last: u32| (year, last, last_day(year, last));
    Some(match resolution {
        Some("year") => (year, 12, 31),
        Some("halfYear") => month_end(if month <= 6 { 6 } else { 12 }),
        Some("quarter") => month_end((month - 1) / 3 * 3 + 3),
        Some("month") => month_end(month),
        _ => (year, month, day),
    })
}

/// Whether `project` is late on `today` (`2026-11-24`): its target, to the end of what it stands for, has
/// passed, and it is not done nor let go of.
#[must_use]
pub fn late(project: &Project, today: &str) -> bool {
    if closed(&project.status.kind) {
        return false;
    }
    let (Some(target), Some(now)) = (
        project
            .target_date
            .as_deref()
            .and_then(|date| end_of(date, project.target_date_resolution.as_deref())),
        parts(today),
    ) else {
        return false;
    };
    now > target
}

/// The target first (to the end of what it stands for), those with none last, then the name.
fn by_target(a: &Project, b: &Project) -> Ordering {
    let end = |project: &Project| {
        project
            .target_date
            .as_deref()
            .and_then(|date| end_of(date, project.target_date_resolution.as_deref()))
    };
    match (end(a), end(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

/// The groups of `projects` whose name holds `words` (whatever the case; all where there are none).
#[must_use]
pub fn group<'a>(projects: &'a [Project], words: &str) -> Vec<Group<'a>> {
    let words = words.trim().to_lowercase();
    let mut at: HashMap<&str, usize> = HashMap::new();
    let mut groups: Vec<Group<'a>> = Vec::new();
    for project in projects
        .iter()
        .filter(|project| words.is_empty() || project.name.to_lowercase().contains(&words))
    {
        let index = *at.entry(project.status.id.as_str()).or_insert_with(|| {
            groups.push(Group {
                id: &project.status.id,
                name: &project.status.name,
                kind: &project.status.kind,
                position: project.status.position,
                projects: Vec::new(),
            });
            groups.len() - 1
        });
        groups[index].projects.push(project);
    }
    for group in &mut groups {
        group.projects.sort_by(|a, b| by_target(a, b));
    }
    groups.sort_by(|a, b| {
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.position.total_cmp(&b.position))
            .then_with(|| a.name.cmp(b.name))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<Project> {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../../test/stand-in/projects.json")).unwrap();
        value["data"]["projects"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|node| serde_json::from_value(node.clone()).ok())
            .collect()
    }

    #[test]
    fn a_day_is_said_by_its_resolution_with_its_year_where_it_is_not_this_one() {
        assert_eq!(when("2026-12-12", None, 2026), "Dec 12");
        assert_eq!(when("2027-01-05", None, 2026), "Jan 5 2027");
        assert_eq!(when("2026-12-01", Some("month"), 2026), "Dec");
        assert_eq!(when("2027-03-31", Some("quarter"), 2026), "Q1 2027");
        assert_eq!(when("2026-10-01", Some("quarter"), 2026), "Q4");
        assert_eq!(when("2027-01-01", Some("halfYear"), 2026), "H1 2027");
        assert_eq!(when("2027-06-30", Some("year"), 2026), "2027");
        assert_eq!(when("soon", None, 2026), "soon");
    }

    #[test]
    fn a_project_is_late_past_the_end_of_what_its_target_stands_for_and_never_when_done() {
        let projects = fixture();
        let named = |name: &str| {
            projects
                .iter()
                .find(|project| project.name == name)
                .unwrap()
        };
        // Its day was Nov 20.
        assert!(late(named("Public API limits"), "2026-11-24"));
        assert!(!late(named("Public API limits"), "2026-11-20"));
        // A month: late only after the month.
        assert!(!late(
            named("Usage page redesign that shows the plan, the limits and what is left of each"),
            "2026-12-31"
        ));
        assert!(late(
            named("Usage page redesign that shows the plan, the limits and what is left of each"),
            "2027-01-01"
        ));
        // Done: never late. No target: never late.
        assert!(!late(named("Invoices v1"), "2027-06-01"));
        assert!(!late(named("Search v2"), "2030-01-01"));
    }

    #[test]
    fn the_groups_go_by_the_type_of_their_state_and_in_each_the_nearest_target_first() {
        let projects = fixture();
        let groups = group(&projects, "");
        let kinds: Vec<&str> = groups.iter().map(|group| group.kind).collect();
        assert_eq!(
            kinds,
            [
                "started",
                "planned",
                "paused",
                "backlog",
                "completed",
                "canceled"
            ]
        );
        let started: Vec<&str> = groups[0].projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            started,
            ["Public API limits", "Billing v2", "Mobile offline mode"]
        );
        // The one with no target is last of its group.
        assert_eq!(groups[1].projects.last().unwrap().name, "Search v2");
        assert!(groups[4].folded_at_first() && !groups[0].folded_at_first());
        // Words narrow by the name, whatever the case.
        let found = group(&projects, "BILL");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].projects[0].name, "Billing v2");
    }
}
