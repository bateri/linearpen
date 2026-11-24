//! The call of the list of projects: `project`, what a project carries that its line does not.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::{Failure, call};
use crate::linear::plain;
use crate::session::Session;

/// The most of the words of an update told.
const UPDATE_MOST: usize = 4000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asked {
    id: String,
}

#[derive(Serialize)]
struct MilestoneMessage {
    name: String,
    progress: u8,
    target: Option<String>,
}

#[derive(Serialize)]
struct ProjectMessage {
    id: String,
    update: Option<String>,
    milestones: Vec<MilestoneMessage>,
}

/// Whether `id` may be Linear's id of a project (a UUID, a slug): letters, digits and dashes.
fn check(id: &str) -> Result<(), Failure> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(Failure::BadParams(
            "id is not the id of a project".to_owned(),
        ));
    }
    Ok(())
}

fn project(session: &Session, asked: Asked) -> Result<ProjectMessage, Failure> {
    check(&asked.id)?;
    let detail = session.project(&asked.id).map_err(Failure::Linear)?;
    let this_year = crate::linear::inbox::moment(crate::session::now_seconds())
        .get(..4)
        .and_then(|year| year.parse().ok())
        .unwrap_or(0);
    Ok(ProjectMessage {
        id: asked.id,
        update: detail.update.map(|body| {
            // (Its lines are kept: it is markdown, which the page draws.)
            let cleaned = crate::linear::lines(&body);
            match cleaned.char_indices().nth(UPDATE_MOST) {
                Some((end, _)) => cleaned[..end].to_owned(),
                None => cleaned,
            }
        }),
        milestones: detail
            .milestones
            .into_iter()
            .take(crate::linear::projects::MILESTONES_MOST as usize)
            .map(|milestone| MilestoneMessage {
                name: plain(&milestone.name),
                // (Within 0 and 100 after the clamp.)
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                progress: (milestone.progress.clamp(0.0, 1.0) * 100.0).round() as u8,
                target: milestone
                    .target_date
                    .as_deref()
                    .map(|date| crate::model::projects::when(date, None, this_year)),
            })
            .collect(),
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "project", project)
}
