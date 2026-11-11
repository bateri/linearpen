//! Handing issues to an agent: which agents there are, the file of issues, the worktree and the pane
//! of bateri that runs the agent. It takes and gives plain Rust values and knows nothing of the page.
//!
//! What it needs of the machine (the `PATH`, whether the pen runs in bateri, the folder it started
//! in) is read once, in `main.rs`, as a [`Setup`]; what it runs is run by a [`Runner`], so that a
//! test says what `git` and `bateri` answer without running them.
//!
//!   agents     which agents are installed, and the command that starts one
//!   handoff    the file of issues, and the prompt that names it
//!   worktree   a new `git` worktree for the agent
//!   bateri     the pane that runs the agent, and the wait for its end

pub mod agents;
pub mod bateri;
pub mod handoff;
pub mod runner;
pub mod worktree;

use std::path::{Path, PathBuf};

use crate::linear::Issue;
use agents::Agent;
use bateri::Place;
use runner::Runner;

/// What the machine says: where programs are, whether the pen runs in a pane of bateri, and the
/// folder it started in.
#[derive(Debug, Clone, Default)]
pub struct Setup {
    /// The directories of the `PATH`.
    pub path: Vec<PathBuf>,
    /// The pen runs in a pane of bateri (`BATERI_TAB_URL` is set), which is where a pane can be made.
    pub in_bateri: bool,
    /// The folder the pen was started in.
    pub folder: Option<PathBuf>,
}

impl Setup {
    /// The setup of an environment, given the way to look a variable up and the folder.
    pub fn resolve(get: &dyn Fn(&str) -> Option<String>, folder: Option<PathBuf>) -> Self {
        let path = get("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .filter(|p| !p.as_os_str().is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Self {
            path,
            in_bateri: get("BATERI_TAB_URL").is_some_and(|url| !url.is_empty()),
            folder,
        }
    }

    /// Why nothing can be started here, or nothing.
    #[must_use]
    pub fn unavailable(&self) -> Option<String> {
        if !self.in_bateri {
            return Some(
                "linearpen is not in a bateri pane, which is where an agent is started".to_owned(),
            );
        }
        if !agents::on_path(&self.path, bateri::PROGRAM) {
            return Some("bateri is not in your PATH".to_owned());
        }
        if agents::installed(&self.path).is_empty() {
            return Some(
                "No agent is installed: claude, codex or cursor-agent in your PATH".to_owned(),
            );
        }
        None
    }
}

/// Where the agent works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    /// The folder the pen was started in.
    Here,
    /// A new worktree of the repository that holds it.
    Worktree,
}

impl Folder {
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "here" => Some(Self::Here),
            "worktree" => Some(Self::Worktree),
            _ => None,
        }
    }
}

/// What is asked.
pub struct Request<'a> {
    /// The issues with their descriptions and comments, the first the one that names the worktree.
    pub issues: &'a [Issue],
    /// What the person wrote for the agent.
    pub prompt: &'a str,
    pub agent: Agent,
    pub folder: Folder,
    pub place: Place,
}

/// What was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    pub pane: String,
    pub folder: PathBuf,
    pub context: PathBuf,
}

/// Why nothing was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// It cannot be done from here: bateri or the agent is not there.
    Unavailable(String),
    /// It was tried, and failed.
    Failed(String),
}

/// What a start needs besides the request.
pub struct Env<'a> {
    pub setup: &'a Setup,
    pub runner: &'a dyn Runner,
    /// Where the files of what is handed over are kept.
    pub state: Option<&'a Path>,
    /// Now, seconds since the epoch.
    pub now: u64,
}

/// What a new worktree would be for the first of `issues`, or why there can be none.
///
/// # Errors
///
/// The words of why there can be no worktree.
pub fn worktree_offer(env: &Env<'_>, first: &Issue) -> Result<worktree::Offer, String> {
    let folder = env
        .setup
        .folder
        .as_deref()
        .ok_or_else(|| "the folder the pen was started in is not known".to_owned())?;
    worktree::offer(
        env.runner,
        folder,
        &first.identifier,
        first.branch_name.as_deref(),
    )
}

/// Hands the issues to the agent: writes the file, makes the worktree where it is asked for, and
/// makes the pane that runs the agent.
///
/// # Errors
///
/// Why it could not be done.
pub fn start(env: &Env<'_>, request: &Request<'_>) -> Result<Started, Refused> {
    if let Some(why) = env.setup.unavailable() {
        return Err(Refused::Unavailable(why));
    }
    if !agents::installed(&env.setup.path).contains(&request.agent) {
        return Err(Refused::Unavailable(format!(
            "{} is not installed: {} is not in your PATH",
            request.agent.name(),
            request.agent.program()
        )));
    }
    let Some(first) = request.issues.first() else {
        return Err(Refused::Failed("there is no issue to work on".to_owned()));
    };
    let here = env.setup.folder.clone().ok_or_else(|| {
        Refused::Failed("the folder the pen was started in is not known".to_owned())
    })?;
    // Everything that can be wrong is found before anything is made.
    let offer = match request.folder {
        Folder::Here => None,
        Folder::Worktree => Some(worktree_offer(env, first).map_err(Refused::Failed)?),
    };
    let state = env
        .state
        .ok_or_else(|| Refused::Failed("there is no folder to keep the issues in".to_owned()))?;
    let context = handoff::write(
        state,
        &handoff::stamp(env.now),
        &first.identifier,
        request.issues,
    )
    .map_err(|why| Refused::Failed(format!("the issues could not be written: {why}")))?;
    let folder = match &offer {
        Some(offer) => {
            worktree::create(env.runner, offer).map_err(Refused::Failed)?;
            offer.path.clone()
        }
        None => here,
    };
    let prompt = handoff::prompt(request.prompt, &context);
    let pane = bateri::launch(
        env.runner,
        request.place,
        &folder,
        &first.identifier,
        &request.agent.command(&prompt),
    )
    .map_err(Refused::Failed)?;
    Ok(Started {
        pane,
        folder,
        context,
    })
}

#[cfg(test)]
mod tests;
