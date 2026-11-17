//! The commands of the command line: what `linearpen list`, `show`, `create`, `update`, `comment` and
//! `teams` ask of Linear and what they print. A command takes the parsed words (`command_line`),
//! the way to Linear and what stdin holds, and gives what to print and how to end; it reads nothing
//! of the environment, so that it is tested with a transport of memory. It knows nothing of the
//! page.
//!
//! Names are found out before anything is written: a name that is none of what Linear has, or more
//! than one, stops the command with the names there are. A write is sent once and never again; where
//! its answer did not come, the command says that it is not known whether Linear made it.

mod json;
mod read;
mod resolve;
mod write;

use std::io::Read;

use serde::Serialize;

use crate::command_line::Command;
use crate::linear::transport::never_sent;
use crate::linear::{Error, Key, Transport, left_open, plain, tidy};

/// The way to Linear a command is given, and where the key came from (said where Linear refuses it).
pub struct Access<'a> {
    pub transport: &'a dyn Transport,
    pub key: &'a Key,
    pub key_source: &'a str,
}

/// What a command comes to: what to print on stdout and on stderr, and the status to end with.
#[derive(Debug, PartialEq, Eq)]
pub struct Done {
    pub out: String,
    pub err: String,
    pub status: u8,
}

/// Why a command ends before it is done: the words (without `linearpen: `) and the status.
#[derive(Debug, PartialEq, Eq)]
pub struct Stop {
    pub words: String,
    pub status: u8,
}

impl Stop {
    /// Linear cannot do it, said no, or has nothing by a name: status 1.
    pub fn no(words: impl Into<String>) -> Self {
        Self {
            words: words.into(),
            status: 1,
        }
    }

    /// What was given is not what a command takes (stdin that is not text, or too long): status 2.
    pub fn usage(words: impl Into<String>) -> Self {
        Self {
            words: words.into(),
            status: 2,
        }
    }
}

/// What a question to Linear that failed says: status 1.
fn asked_wrong(access: &Access, error: &Error) -> Stop {
    Stop::no(match error {
        Error::Refused => format!("Linear did not accept the key from {}", access.key_source),
        Error::Failed(why) => tidy(why),
        other => other.to_string(),
    })
}

/// Whether a write that failed with `error` may have been made: nothing came back, or what came
/// cannot be read. A request that never left the machine (no host, no connection) was not made.
fn unknown(error: &Error) -> bool {
    match error {
        Error::Refused | Error::Forbidden => false,
        Error::Failed(why) => left_open(why),
        Error::Unreachable(why) => !never_sent(why),
        Error::Unsure => true,
    }
}

/// What a write that failed says. Where Linear answered that it did not make it, status 1; where
/// nothing came or what came cannot be read, the write may have been made: status 3, and the words
/// say to look in Linear before it is run again.
fn written_wrong(access: &Access, error: &Error) -> Stop {
    if !unknown(error) {
        return match error {
            Error::Forbidden => Stop::no("Linear did not allow this key to write that"),
            other => asked_wrong(access, other),
        };
    }
    Stop {
        words: format!(
            "{}; it is not known whether Linear made it; look in Linear before running it again",
            tidy(&error.to_string())
        ),
        status: 3,
    }
}

/// `value` as pretty JSON and a line break.
fn json<T: Serialize>(value: &T) -> Result<String, Stop> {
    serde_json::to_string_pretty(value)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|why| Stop::no(format!("the JSON could not be made: {why}")))
}

/// Runs `command`.
pub fn run(access: &Access, command: &Command, stdin: &mut dyn Read) -> Done {
    let done = match command {
        Command::List(list) => read::list(access, list),
        Command::Show { issue, json } => read::show(access, issue, *json),
        Command::Teams { team, json } => read::teams(access, team.as_deref(), *json),
        Command::Create(create) => write::create(access, create, stdin),
        Command::Update(update) => write::update(access, update),
        Command::Comment(comment) => write::comment(access, comment, stdin),
    };
    match done {
        Ok(out) => Done {
            out,
            err: String::new(),
            status: 0,
        },
        // The words may hold names Linear gave (the labels there are, a team's key): they are made one
        // line with no control character here, the one way they go to stderr.
        Err(stop) => Done {
            out: String::new(),
            err: format!("linearpen: {}\n", plain(&stop.words)),
            status: stop.status,
        },
    }
}

#[cfg(test)]
mod tests;
