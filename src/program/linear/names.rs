//! The lists a person can go to, by name: what Linear starred, the teams (their issues and the
//! cycle that runs), the saved views and the projects that are not finished. Four questions, each a
//! page at a time up to a ceiling; what a name is called is Linear's text and goes through `plain`.
//!
//! Only names and ids are read here: the issues of a list are asked for when it is shown (`fetch`).

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::json;

use super::error::plain;
use super::query::{self, Named, Operation};
use super::types::{PageInfo, cycle_label};
use super::{Error, Key, Scope, Transport, run};

/// The most names taken of each question: a workspace with more shows the first of them.
pub const NAMES_MOST: usize = 200;

/// How many names are asked for at a time.
const NAMES_PAGE: u32 = 100;

/// Where in the card a list stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Favorite,
    Team,
    View,
    Project,
}

impl Section {
    /// The word the page is told.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Favorite => "favorite",
            Self::Team => "team",
            Self::View => "view",
            Self::Project => "project",
        }
    }
}

/// A cycle, as much as the card tells of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleInfo {
    pub number: u32,
    /// When it ends, as Linear says it (`2026-11-15T00:00:00.000Z`): the page says the day.
    pub ends_at: String,
}

/// One list of the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub scope: Scope,
    pub section: Section,
    pub title: String,
    /// The key of the team it is of (`ACM`), where it is of one.
    pub team: Option<String>,
    pub cycle: Option<CycleInfo>,
}

#[derive(Deserialize)]
struct Nodes<T> {
    nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct TeamKey {
    key: String,
}

#[derive(Deserialize)]
struct WireView {
    id: String,
    name: String,
    #[serde(rename = "modelName")]
    model_name: String,
    team: Option<TeamKey>,
}

#[derive(Deserialize)]
struct WireProject {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct WireCycle {
    id: String,
    number: f64,
    name: Option<String>,
    #[serde(rename = "endsAt")]
    ends_at: String,
    team: Option<TeamKey>,
}

#[derive(Deserialize)]
struct WireLabel {
    id: String,
    name: String,
    team: Option<TeamKey>,
}

#[derive(Deserialize)]
struct WireFavorite {
    #[serde(default, rename = "sortOrder")]
    sort_order: f64,
    #[serde(default, rename = "customView")]
    custom_view: Option<WireView>,
    #[serde(default)]
    project: Option<WireProject>,
    #[serde(default)]
    cycle: Option<WireCycle>,
    #[serde(default)]
    label: Option<WireLabel>,
}

#[derive(Deserialize)]
struct WireMembership {
    #[serde(default, rename = "sortOrder")]
    sort_order: f64,
    team: WireTeam,
}

#[derive(Deserialize)]
struct WireTeam {
    id: String,
    key: String,
    #[serde(rename = "activeCycle")]
    active_cycle: Option<WireCycle>,
}

#[derive(Deserialize)]
struct FavoritesData {
    favorites: Nodes<WireFavorite>,
}

#[derive(Deserialize)]
struct TeamsData {
    viewer: Memberships,
}

#[derive(Deserialize)]
struct Memberships {
    #[serde(rename = "teamMemberships")]
    team_memberships: Nodes<WireMembership>,
}

#[derive(Deserialize)]
struct ViewsData {
    #[serde(rename = "customViews")]
    custom_views: Nodes<WireView>,
}

#[derive(Deserialize)]
struct ProjectsData {
    projects: Nodes<WireProject>,
}

/// Every node of one question, a page at a time, up to [`NAMES_MOST`].
fn pages<D, T>(
    transport: &dyn Transport,
    key: &Key,
    operation: &Operation,
    filter: Option<serde_json::Value>,
    nodes: fn(D) -> Nodes<T>,
) -> Result<Vec<T>, Error>
where
    D: serde::de::DeserializeOwned,
{
    let mut all: Vec<T> = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let variables = Named {
            first: NAMES_PAGE,
            after: after.as_deref(),
            filter: filter.clone(),
        };
        let page = nodes(run(transport, key, operation, Some(variables))?);
        all.extend(page.nodes);
        if all.len() >= NAMES_MOST {
            all.truncate(NAMES_MOST);
            return Ok(all);
        }
        match (page.page_info.has_next_page, page.page_info.end_cursor) {
            (true, Some(cursor)) => after = Some(cursor),
            _ => return Ok(all),
        }
    }
}

fn team_key(team: Option<TeamKey>) -> Option<String> {
    team.map(|team| plain(&team.key))
        .filter(|key| !key.is_empty())
}

/// A cycle that is a number of cycles: Linear's number is a float, and one that is not a count is no
/// cycle.
fn cycle_info(cycle: &WireCycle) -> Option<CycleInfo> {
    let number = cycle.number;
    if !(number.is_finite()
        && number >= 0.0
        && number.fract() == 0.0
        && number <= f64::from(u32::MAX))
    {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(CycleInfo {
        number: number as u32,
        ends_at: plain(&cycle.ends_at),
    })
}

/// A list nobody can read the name of is not offered.
fn titled(name: &str) -> Option<String> {
    Some(plain(name)).filter(|name| !name.is_empty())
}

fn view_entry(view: WireView, section: Section) -> Option<Entry> {
    // Views of projects and of other things have no issues to show.
    if view.model_name != "Issue" || view.id.is_empty() {
        return None;
    }
    Some(Entry {
        scope: Scope::View(view.id),
        section,
        title: titled(&view.name)?,
        team: team_key(view.team),
        cycle: None,
    })
}

fn project_entry(project: WireProject, section: Section) -> Option<Entry> {
    if project.id.is_empty() {
        return None;
    }
    Some(Entry {
        scope: Scope::Project(project.id),
        section,
        title: titled(&project.name)?,
        team: None,
        cycle: None,
    })
}

fn cycle_entry(cycle: WireCycle, section: Section, title: Option<&str>) -> Option<Entry> {
    let info = cycle_info(&cycle)?;
    let team = team_key(cycle.team);
    let title = match title {
        Some(title) => title.to_owned(),
        None => {
            let label = cycle_label(info.number, cycle.name.as_deref());
            match &team {
                Some(team) => format!("{team} \u{b7} {}", plain(&label)),
                None => plain(&label),
            }
        }
    };
    if cycle.id.is_empty() {
        return None;
    }
    Some(Entry {
        scope: Scope::Cycle(cycle.id),
        section,
        title,
        team,
        cycle: Some(info),
    })
}

fn favorite_entry(favorite: WireFavorite) -> Option<Entry> {
    let section = Section::Favorite;
    if let Some(view) = favorite.custom_view {
        return view_entry(view, section);
    }
    if let Some(project) = favorite.project {
        return project_entry(project, section);
    }
    if let Some(cycle) = favorite.cycle {
        return cycle_entry(cycle, section, None);
    }
    let label = favorite.label?;
    if label.id.is_empty() {
        return None;
    }
    Some(Entry {
        scope: Scope::Label(label.id),
        section,
        title: titled(&label.name)?,
        team: team_key(label.team),
        cycle: None,
    })
}

/// The lists the person can go to, in the order of the card: the favorites by their sort order, the
/// teams the person is a member of, in the order of their own list of teams (each with the cycle that
/// runs and its issues), the views and the projects by
/// name. A list is in the card once: the first place it has is its place.
pub fn names(transport: &dyn Transport, key: &Key) -> Result<Vec<Entry>, Error> {
    let mut favorites = pages(
        transport,
        key,
        &query::LIST_FAVORITES,
        None,
        |data: FavoritesData| data.favorites,
    )?;
    let mut teams = pages(
        transport,
        key,
        &query::LIST_TEAMS,
        None,
        |data: TeamsData| data.viewer.team_memberships,
    )?;
    let views = pages(
        transport,
        key,
        &query::LIST_VIEWS,
        None,
        |data: ViewsData| data.custom_views,
    )?;
    let projects = pages(
        transport,
        key,
        &query::LIST_PROJECTS,
        Some(json!({ "completedAt": { "null": true }, "canceledAt": { "null": true } })),
        |data: ProjectsData| data.projects,
    )?;
    favorites.sort_by(|a, b| a.sort_order.total_cmp(&b.sort_order));
    // The teams in the order of the person's own list of teams (the sidebar of Linear).
    teams.sort_by(|a, b| a.sort_order.total_cmp(&b.sort_order));
    let mut found: Vec<Entry> = favorites.into_iter().filter_map(favorite_entry).collect();
    for team in teams.into_iter().map(|membership| membership.team) {
        let key = plain(&team.key);
        if let Some(cycle) = team.active_cycle {
            let title = format!("{key} \u{b7} Current cycle");
            if let Some(mut entry) = cycle_entry(cycle, Section::Team, Some(&title)) {
                entry.team = Some(key.clone());
                found.push(entry);
            }
        }
        if !team.id.is_empty() {
            found.push(Entry {
                scope: Scope::Team(team.id),
                section: Section::Team,
                title: format!("{key} \u{b7} Issues"),
                team: Some(key),
                cycle: None,
            });
        }
    }
    let mut views: Vec<Entry> = views
        .into_iter()
        .filter_map(|view| view_entry(view, Section::View))
        .collect();
    views.sort_by_key(|entry| entry.title.to_lowercase());
    let mut projects: Vec<Entry> = projects
        .into_iter()
        .filter_map(|project| project_entry(project, Section::Project))
        .collect();
    projects.sort_by_key(|entry| entry.title.to_lowercase());
    found.extend(views);
    found.extend(projects);
    let mut seen: HashSet<Scope> = HashSet::new();
    found.retain(|entry| seen.insert(entry.scope.clone()));
    Ok(found)
}
