//! The projects of the workspace, as Linear's Projects page has them: a list of them (`fetch`, the list
//! of projects), and what one carries that its line does not (`detail`: its milestones and the words of
//! its last update), asked when it is opened. Read only.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::Viewer;
use super::{Error, Fetched, Key, Listing, Transport, query, run};

/// How many projects are asked for, and how many milestones of one.
pub const MOST: u32 = 100;
pub const MILESTONES_MOST: u32 = 20;

/// A state of projects: Linear's id of it, its name, its type (`started`, `planned`, `paused`,
/// `backlog`, `completed`, `canceled`) and its place among those of its type.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Status {
    pub id: String,
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub position: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lead {
    pub display_name: String,
    #[serde(default)]
    pub is_me: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Poster {
    display_name: String,
}

/// The last update of a project: when, by whom, Linear's summary of it, and whether Linear holds the
/// health it said outdated.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub created_at: String,
    #[serde(default)]
    pub short_summary: Option<String>,
    #[serde(default)]
    pub is_stale: bool,
    #[serde(default)]
    user: Option<Poster>,
}

impl Update {
    /// Who posted it, where Linear says.
    #[must_use]
    pub fn who(&self) -> Option<&str> {
        self.user.as_ref().map(|user| user.display_name.as_str())
    }
}

/// A project as its line in the list has it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub health: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub start_date_resolution: Option<String>,
    #[serde(default)]
    pub target_date: Option<String>,
    #[serde(default)]
    pub target_date_resolution: Option<String>,
    pub status: Status,
    #[serde(default)]
    pub lead: Option<Lead>,
    #[serde(default)]
    pub last_update: Option<Update>,
}

/// A milestone of a project: its name, how much of it is done (0 to 1) and the day it is due.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub name: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub target_date: Option<String>,
}

/// What a project carries that its line does not.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub update: Option<String>,
    pub milestones: Vec<Milestone>,
}

#[derive(Serialize)]
struct First {
    first: u32,
}

#[derive(Serialize)]
struct One<'a> {
    id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    viewer: Viewer,
    projects: Nodes,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Nodes {
    #[serde(default)]
    nodes: Vec<serde_json::Value>,
    #[serde(default)]
    page_info: Option<PageInfo>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    #[serde(default)]
    has_next_page: bool,
}

#[derive(Deserialize)]
struct DetailData {
    project: Option<RawDetail>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDetail {
    #[serde(default)]
    last_update: Option<Body>,
    #[serde(default)]
    project_milestones: Option<Milestones>,
}

#[derive(Deserialize)]
struct Body {
    #[serde(default)]
    body: String,
}

#[derive(Deserialize)]
struct Milestones {
    #[serde(default)]
    nodes: Vec<Milestone>,
}

/// The projects of the workspace, the first `MOST` that changed last. A project Linear tells in a shape
/// that is not a project's is let go of, and the rest are read.
///
/// # Errors
///
/// As every question to Linear.
pub fn fetch(transport: &dyn Transport, key: &Key) -> Result<Fetched, Error> {
    let data: Data = run(
        transport,
        key,
        &query::PROJECTS,
        Some(First { first: MOST }),
    )?;
    let told = data.projects.nodes.len();
    let projects: Vec<Project> = data
        .projects
        .nodes
        .into_iter()
        .filter_map(|node| serde_json::from_value::<Project>(node).ok())
        .collect();
    // Projects told and none read is not a workspace with none: the shape Linear tells is not the one
    // asked for, and the page says so rather than "No projects".
    if told > 0 && projects.is_empty() {
        return Err(Error::Failed(format!(
            "Linear told {told} projects in a shape linearpen does not read"
        )));
    }
    Ok(Fetched {
        viewer: data.viewer,
        listing: Listing {
            issues: Vec::new(),
            truncated: data
                .projects
                .page_info
                .is_some_and(|info| info.has_next_page),
            notes: HashMap::new(),
            projects,
        },
    })
}

/// The milestones of the project `id` and the words of its last update.
///
/// # Errors
///
/// As every question to Linear; `Error::Failed` where Linear has no such project.
pub fn detail(transport: &dyn Transport, key: &Key, id: &str) -> Result<Detail, Error> {
    let data: DetailData = run(transport, key, &query::PROJECT, Some(One { id }))?;
    let project = data
        .project
        .ok_or_else(|| Error::Failed("Linear has no such project".to_owned()))?;
    Ok(Detail {
        update: project
            .last_update
            .map(|update| update.body)
            .filter(|body| !body.trim().is_empty()),
        milestones: project
            .project_milestones
            .map(|milestones| milestones.nodes)
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::transport::Reply;

    /// Linear as a test says: one reply to every question.
    struct Says(String);

    impl Transport for Says {
        fn post(&self, _: &Key, _: &str) -> Result<Reply, Error> {
            Ok(Reply {
                status: 200,
                body: self.0.clone(),
            })
        }
    }

    #[test]
    fn projects_told_in_a_shape_none_of_which_is_read_are_a_failure_and_not_none() {
        let mut fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../test/stand-in/projects.json")).unwrap();
        let key = Key::new("lin_api_k").unwrap();
        let read = fetch(&Says(fixture.to_string()), &key).expect("the fixture is read");
        assert_eq!(read.listing.projects.len(), 9);
        // Every project without its state: Linear told some, the program read none.
        for node in fixture["data"]["projects"]["nodes"].as_array_mut().unwrap() {
            node.as_object_mut().unwrap().remove("status");
        }
        match fetch(&Says(fixture.to_string()), &key) {
            Err(Error::Failed(words)) => assert!(words.contains("does not read"), "{words}"),
            other => panic!(
                "not a failure: {:?}",
                other.map(|fetched| fetched.listing.projects.len())
            ),
        }
        // None told is a workspace with none.
        fixture["data"]["projects"]["nodes"] = serde_json::json!([]);
        let none = fetch(&Says(fixture.to_string()), &key).expect("none is read");
        assert!(none.listing.projects.is_empty());
    }

    #[test]
    fn a_project_in_a_shape_not_a_projects_is_let_go_of_and_the_others_are_read() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../test/stand-in/projects.json")).unwrap();
        let nodes = fixture["data"]["projects"]["nodes"].as_array().unwrap();
        let read: Vec<Project> = nodes
            .iter()
            .filter_map(|node| serde_json::from_value::<Project>(node.clone()).ok())
            .collect();
        assert_eq!(read.len(), nodes.len() - 1, "the broken one is let go of");
        let billing = read
            .iter()
            .find(|project| project.name == "Billing v2")
            .unwrap();
        assert_eq!(billing.status.kind, "started");
        assert_eq!(billing.health.as_deref(), Some("onTrack"));
        assert_eq!(
            billing.lead.as_ref().map(|lead| lead.display_name.as_str()),
            Some("Deniz")
        );
        assert_eq!(
            billing.last_update.as_ref().and_then(Update::who),
            Some("Deniz")
        );
    }
}
