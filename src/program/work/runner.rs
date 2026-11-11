//! The way the program runs another program: with its arguments as a list, never through a shell,
//! with nothing on its stdin. It is a trait so that a test can say what `git` and `bateri` answer
//! without running them.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// How a program ended, and what it said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub ok: bool,
    pub out: String,
    pub err: String,
}

impl Ran {
    /// The first line the program said on its stderr, or `fallback`: the words for the person.
    #[must_use]
    pub fn reason(&self, fallback: &str) -> String {
        self.err
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map_or_else(|| fallback.to_owned(), str::to_owned)
    }
}

pub trait Runner: Send + Sync {
    /// Runs `program` (found in the `PATH`) with `args`, in `cwd` where it is given, to its end.
    ///
    /// # Errors
    ///
    /// The program could not be started.
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Ran>;
}

/// The system's: a child process.
pub struct System;

impl Runner for System {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Ran> {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = command.output()?;
        Ok(Ran {
            ok: output.status.success(),
            out: String::from_utf8_lossy(&output.stdout).into_owned(),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_is_the_first_line_that_says_something() {
        let ran = Ran {
            ok: false,
            out: String::new(),
            err: "\n  fatal: not a git repository\nmore\n".to_owned(),
        };
        assert_eq!(ran.reason("it failed"), "fatal: not a git repository");
        let quiet = Ran {
            ok: false,
            out: String::new(),
            err: String::new(),
        };
        assert_eq!(quiet.reason("it failed"), "it failed");
    }
}
