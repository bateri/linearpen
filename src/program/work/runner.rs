//! The way the program runs another program: with its arguments as a list, never through a shell,
//! with nothing on its stdin. It is a trait so that a test can say what `bateri` answers without
//! running it.

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
    /// The line of the program's stderr that says why it failed, or `fallback`: the words for the
    /// person. A line that says `fatal:` or `error:` is it (`git` tells what it is doing first);
    /// else the last line that says something.
    #[must_use]
    pub fn reason(&self, fallback: &str) -> String {
        let lines = || {
            self.err
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
        };
        lines()
            .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
            .or_else(|| lines().next_back())
            .map_or_else(|| fallback.to_owned(), str::to_owned)
    }
}

pub trait Runner: Send + Sync {
    /// Runs `program` (found in the `PATH`) with `args`, in `cwd` where it is given, to its end, with
    /// the variables `env` set in its environment (each set to its value, an empty one included,
    /// over what this program's own environment holds).
    ///
    /// # Errors
    ///
    /// The program could not be started.
    fn run_with(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &[(&str, &str)],
    ) -> io::Result<Ran>;

    /// Runs `program` as `run_with` does, with no variable of its own set.
    ///
    /// # Errors
    ///
    /// The program could not be started.
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> io::Result<Ran> {
        self.run_with(program, args, cwd, &[])
    }
}

/// The system's: a child process.
pub struct System;

/// The child to run: no shell, nothing on its stdin, and without the key that may be in this
/// program's environment (`bateri` and the agent it starts are not its readers).
pub(super) fn command_of(program: &str, args: &[String], cwd: Option<&Path>) -> Command {
    let mut command = Command::new(program);
    command
        .args(args)
        .env_remove("LINEAR_API_KEY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
}

impl Runner for System {
    fn run_with(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&Path>,
        env: &[(&str, &str)],
    ) -> io::Result<Ran> {
        let mut command = command_of(program, args, cwd);
        // After the key is taken out: a variable given is set whatever this environment holds.
        for (name, value) in env {
            command.env(name, value);
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
    fn the_reason_is_the_line_that_says_fatal_or_else_the_last_that_says_something() {
        let ran = Ran {
            ok: false,
            out: String::new(),
            err: "\n  fatal: not a git repository\nmore\n".to_owned(),
        };
        assert_eq!(ran.reason("it failed"), "fatal: not a git repository");
        let doing = Ran {
            ok: false,
            out: String::new(),
            err: "Preparing worktree (checking out 'feat')\nfatal: 'x' is already used\n"
                .to_owned(),
        };
        assert_eq!(doing.reason("it failed"), "fatal: 'x' is already used");
        let plain = Ran {
            ok: false,
            out: String::new(),
            err: "first\nlast\n".to_owned(),
        };
        assert_eq!(plain.reason("it failed"), "last");
        let quiet = Ran {
            ok: false,
            out: String::new(),
            err: String::new(),
        };
        assert_eq!(quiet.reason("it failed"), "it failed");
    }

    #[test]
    fn a_child_is_given_the_variables_a_run_names_and_still_not_the_key() {
        let ran = System
            .run_with(
                "sh",
                &[
                    "-c".to_owned(),
                    "printf '%s|%s|%s' \"$LINEARPEN_RUNNER_A\" \"${LINEARPEN_RUNNER_B-unset}\" \"${LINEAR_API_KEY-unset}\"".to_owned(),
                ],
                None,
                &[("LINEARPEN_RUNNER_A", "x y"), ("LINEARPEN_RUNNER_B", "")],
            )
            .unwrap();
        assert_eq!(ran.out, "x y||unset");
        // Without them there is none.
        let none = System
            .run(
                "sh",
                &[
                    "-c".to_owned(),
                    "printf '%s' \"${LINEARPEN_RUNNER_A-unset}\"".to_owned(),
                ],
                None,
            )
            .unwrap();
        assert_eq!(none.out, "unset");
    }

    #[test]
    fn a_child_does_not_get_the_key_of_this_programs_environment() {
        let command = command_of("git", &["status".to_owned()], None);
        let key = command
            .get_envs()
            .find(|(name, _)| *name == "LINEAR_API_KEY")
            .expect("the variable is named");
        assert_eq!(key.1, None, "it is removed for the child");
    }
}
