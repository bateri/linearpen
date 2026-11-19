//! Handing an issue to an agent: which agents there are, the file of the issue, and the pane
//! of bateri that runs the agent. It takes and gives plain Rust values and knows nothing of the page.
//!
//! What it needs of the machine (the `PATH`, whether the pen runs in bateri, the folder it started
//! in) is read once, in `main.rs`, as a [`Setup`]; what it runs is run by a [`Runner`], so that a
//! test says what `bateri` answers without running it.
//!
//!   agents     which agents are installed, and the command that starts one
//!   handoff    the file of the issue, and the prompt that names it
//!   worktree   the branch Linear suggests, checked before the file gives it (the pen makes no worktree)
//!   bateri     the pane that runs the agent, and the wait for its end
//!   progress   what a start moves in Linear where the person chose it (an issue not begun, forward)

pub mod agents;
pub mod bateri;
pub mod handoff;
pub mod progress;
pub mod runner;
pub mod worktree;

use std::path::{Path, PathBuf};

use crate::linear::Issue;
use crate::state::draft;
use agents::Agent;
use bateri::Place;
use runner::Runner;

/// What the machine says: where programs are, whether the pen runs in a pane of bateri, and the
/// folder it started in.
#[derive(Debug, Clone, Default)]
pub struct Setup {
    /// The directories of the `PATH`.
    pub path: Vec<PathBuf>,
    /// The pen runs in a pane of bateri, which is where a pane can be made: one of `PANE_ADDRESS` is
    /// set.
    pub in_bateri: bool,
    /// The folder the pen was started in.
    pub folder: Option<PathBuf>,
    /// The program the pen is, as the agent is told to run it; where it is not known the agent is
    /// told `linearpen`.
    pub exe: Option<PathBuf>,
}

/// The variable that tells a `linearpen` run in an agent's pane that it was started for an agent,
/// and for which issue (Linear's id of it): the command line then writes nothing but that issue's
/// status and comments (`cli/guard.rs`). A start sets it in the environment of the `bateri` that makes
/// the pane, and bateri starts the pane's program in that environment, so the agent's own command is
/// as it is for any run; `main.rs` reads it.
pub const AGENT_VARIABLE: &str = "LINEARPEN_AGENT";

/// The variables that hold the address of the pane the pen runs in: the first is in the shells
/// bateri holds on its own machine, the second is what `ssh` carries to a server, whose shells have
/// no other. bateri's own command takes either to know whose pane asks, so a pane on a server is
/// one too.
const PANE_ADDRESS: [&str; 2] = ["BATERI_TAB_URL", "LC_BATERI_TAB_URL"];

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
            in_bateri: PANE_ADDRESS
                .iter()
                .any(|name| get(name).is_some_and(|url| !url.is_empty())),
            folder,
            exe: None,
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
    /// A new worktree, which the agent makes by the project's own rules, of the repository the work
    /// is in: the folder's own, or one under it. The pen does not look: it asks `git` nothing.
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
    /// The issue with its description and comments; it names the pane.
    pub issue: &'a Issue,
    pub agent: Agent,
    pub folder: Folder,
    pub place: Place,
    /// What the person wrote for the agent, as [`handoff::words`] made it: it ends the prompt, after
    /// the pen's own sentences.
    pub words: Option<&'a str>,
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

/// Hands the issue to the agent: writes the file and makes the pane that runs the agent, in the
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
    let issue = request.issue;
    let here = env.setup.folder.clone().ok_or_else(|| {
        Refused::Failed("the folder the pen was started in is not known".to_owned())
    })?;
    // Everything that can be wrong is found before anything is made.
    let worktree = request.folder == Folder::Worktree;
    let state = env
        .state
        .ok_or_else(|| Refused::Failed("there is no folder to keep the issue in".to_owned()))?;
    let context = handoff::write(state, &handoff::stamp(env.now), issue)
        .map_err(|why| Refused::Failed(format!("the issue could not be written: {why}")))?;
    // The one thing of Linear's in the prompt is the id of the issue, and only where it is a UUID. The
    // variable is set for the pane in either case (empty where there is no id), so that one in the
    // pen's own environment does not go to the agent.
    let id = draft::is_uuid(&issue.id).then_some(issue.id.as_str());
    let prompt = handoff::prompt(
        &context,
        worktree,
        id.map(|issue| handoff::Writes {
            exe: env.setup.exe.as_deref(),
            issue,
        }),
        request.words,
    );
    let pane = bateri::launch(
        env.runner,
        request.place,
        &here,
        &issue.identifier,
        &request.agent.command(&prompt),
        &[(AGENT_VARIABLE, id.unwrap_or_default())],
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
