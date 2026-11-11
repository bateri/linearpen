//! Handing issues to an agent: which agents there are, the file of issues, and the pane
//! of bateri that runs the agent. It takes and gives plain Rust values and knows nothing of the page.
//!
//! What it needs of the machine (the `PATH`, whether the pen runs in bateri, the folder it started
//! in) is read once, in `main.rs`, as a [`Setup`]; what it runs is run by a [`Runner`], so that a
//! test says what `git` and `bateri` answer without running them.
//!
//!   agents     which agents are installed, and the command that starts one
//!   handoff    the file of issues, and the prompt that names it
//!   worktree   what the agent is told of a new `git` worktree (the pen makes none)
//!   bateri     the pane that runs the agent, and the wait for its end
//!   progress   what a start moves in Linear where the person chose it (issues not begun, forward)

pub mod agents;
pub mod bateri;
pub mod handoff;
pub mod progress;
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
    /// A new worktree, which the agent makes by the project's own rules.
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
    /// The issues with their descriptions and comments, the first the one that names the pane.
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

/// Whether the agent can be asked for a new worktree from the pen's folder.
///
/// # Errors
///
/// The words of why not.
pub fn worktree_available(env: &Env<'_>) -> Result<(), String> {
    let folder = env
        .setup
        .folder
        .as_deref()
        .ok_or_else(|| "the folder the pen was started in is not known".to_owned())?;
    worktree::available(env.runner, folder)
}

/// Hands the issues to the agent: writes the file and makes the pane that runs the agent, in the
/// pen's folder. A new worktree is the agent's to make: the prompt asks it for one.
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
    let worktree = request.folder == Folder::Worktree;
    if worktree {
        worktree_available(env).map_err(Refused::Failed)?;
    }
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
    let prompt = handoff::prompt(request.prompt, &context, worktree);
    let pane = bateri::launch(
        env.runner,
        request.place,
        &here,
        &first.identifier,
        &request.agent.command(&prompt),
    )
    .map_err(|no| {
        // Nothing runs there, so the file made for it goes. (Where a pane may be there, it is
        // left what it runs on.)
        if !no.maybe {
            handoff::discard(&context);
        }
        Refused::Failed(no.why)
    })?;
    Ok(Started {
        pane,
        folder: here,
        context,
    })
}

#[cfg(test)]
mod tests;
