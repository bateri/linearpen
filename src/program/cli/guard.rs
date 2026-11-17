//! What a `linearpen` run in an agent's pane may write. A start of an agent (`work/`) sets
//! `LINEARPEN_AGENT` to Linear's id of the issue it was started on, and a command run with that
//! variable set may change the status of that issue and comment on it, and write nothing else. It is
//! a guard and not a lock: an agent that unsets the variable, or reads the key itself, goes round it;
//! the agent's own questions are what keeps it from doing so. It is asked first by each command that
//! writes, before a word of stdin is read or Linear is asked anything, and again with the issues the
//! command named once Linear has told which they are.

use std::ffi::OsStr;

use super::Stop;
use crate::command_line::{Fields, Update};
use crate::linear::Issue;
use crate::state::draft;

/// What a command may write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guard {
    /// The run was not started for an agent: the command writes what it is asked.
    Off,
    /// The run was started for an agent on the issue with this id (a UUID, in lower case).
    Issue(String),
    /// The run was started for an agent, and the variable holds no id that is a UUID: nothing is
    /// written.
    Nothing,
}

const NO_ISSUES: &str = "linearpen was started for an agent; it makes no issues";
const ONLY_THE_STATUS: &str = "linearpen was started for an agent; it changes only the status";
const OTHER_ISSUE: &str = "linearpen was started for an agent on another issue; it may change only that issue's status and comment on it";
const NOTHING: &str =
    "linearpen was started for an agent with no issue of its own; it writes nothing";

impl Guard {
    /// The guard of a run that finds `variable` (the value of `LINEARPEN_AGENT`, or nothing where
    /// it is not set) in its environment. A variable that is set, whatever it holds (nothing, or
    /// what is not text), is an agent's run: only an id that is a UUID gives it an issue to write.
    #[must_use]
    pub fn of(variable: Option<&OsStr>) -> Self {
        match variable {
            None => Self::Off,
            Some(value) => match value.to_str() {
                Some(id) if draft::is_uuid(id) => Self::Issue(id.to_ascii_lowercase()),
                _ => Self::Nothing,
            },
        }
    }

    /// Whether `create` may go on.
    ///
    /// # Errors
    ///
    /// The words of why it may not.
    pub fn creates(&self) -> Result<(), Stop> {
        match self {
            Self::Off => Ok(()),
            Self::Issue(_) => Err(Stop::no(NO_ISSUES)),
            Self::Nothing => Err(Stop::no(NOTHING)),
        }
    }

    /// Whether `update` may go on, by what it changes: only the status may be changed.
    ///
    /// # Errors
    ///
    /// The words of why it may not.
    pub fn updates(&self, update: &Update) -> Result<(), Stop> {
        let status_only = update.title.is_none()
            && update.unlabel.is_empty()
            && update.fields
                == Fields {
                    status: update.fields.status.clone(),
                    ..Fields::default()
                };
        match self {
            Self::Off => Ok(()),
            Self::Issue(_) if status_only => Ok(()),
            Self::Issue(_) => Err(Stop::no(ONLY_THE_STATUS)),
            Self::Nothing => Err(Stop::no(NOTHING)),
        }
    }

    /// Whether `comment` may go on.
    ///
    /// # Errors
    ///
    /// The words of why it may not.
    pub fn comments(&self) -> Result<(), Stop> {
        match self {
            Self::Off | Self::Issue(_) => Ok(()),
            Self::Nothing => Err(Stop::no(NOTHING)),
        }
    }

    /// Whether the issues a command writes to, as Linear has them, are the one this run was started
    /// on and no other.
    ///
    /// # Errors
    ///
    /// The words of why not.
    pub fn only(&self, issues: &[Issue]) -> Result<(), Stop> {
        match self {
            Self::Off => Ok(()),
            Self::Issue(id) if issues.iter().all(|issue| issue.id.eq_ignore_ascii_case(id)) => {
                Ok(())
            }
            Self::Issue(_) => Err(Stop::no(OTHER_ISSUE)),
            Self::Nothing => Err(Stop::no(NOTHING)),
        }
    }
}
