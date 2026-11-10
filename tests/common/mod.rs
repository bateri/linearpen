//! linearpen run as a child process, in an environment of its own: the one place the tests get it
//! from. Whatever starts a process in a test starts it from `Home::command`, so that
//!
//! - `PATH` begins with `test/stand-in/bin`, where `curl` (and, in the tests that start an agent,
//!   `bateri`, `claude`, `codex` and `cursor-agent`) are stand-ins that touch nothing; the system
//!   directories follow only for `sh`, `cat` and the like;
//! - `HOME`, `XDG_*`, `TMPDIR`, `LINEARPEN_CONFIG_DIR` and `KABLO_DIR` are directories of the test,
//!   so nothing is read from or written under the home of whoever runs it;
//! - `LINEARPEN_API_URL` is a loopback address nothing listens on, so that a `curl` that is not the
//!   stand-in finds nobody (and never `api.linear.app`);
//! - the rest of the environment is gone: no `BATERI_TAB_URL` (a command that is run in the user's
//!   open bateri opens a pane in it), no `LINEAR_API_KEY` (their real key).
//!
//! A test that starts a process without this does one of those things for real.

// Each file of tests uses a part of what is here.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// The program the tests run.
pub fn program() -> &'static str {
    env!("CARGO_BIN_EXE_linearpen")
}

/// The directory of the stand-in commands.
pub fn stand_in_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test/stand-in/bin")
}

/// A key that is not one: what the tests give where a key is needed.
pub const KEY: &str = "lin_api_FAKEKEY0123456789abcdef";

/// What a process said, and how it ended.
pub struct Ran {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `command` to its end, or for `limit` and then kills it.
pub fn run(mut command: Command, input: Option<&str>, limit: Duration) -> Ran {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("the process starts");
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(input.as_bytes());
    }
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut out);
            }
            String::from_utf8_lossy(&out).into_owned()
        })
    };
    let stdout = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait().expect("the process can be waited for") {
            break status.code();
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ran {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    }
}

/// A call of the stand-in `curl`, as it recorded it.
#[derive(Debug)]
pub struct Call {
    /// The arguments, one to an entry.
    pub argv: Vec<String>,
    /// What the environment had of `LINEAR_API_KEY`: `set` or `unset`.
    pub env_key: String,
    /// The configuration on its stdin: the address, the key, the body.
    pub stdin: String,
}

/// A home of its own, and the environment of a process in it.
pub struct Home {
    pub dir: TempDir,
}

impl Home {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("linearpen-test-")
            .tempdir()
            .expect("a directory for the test");
        std::fs::create_dir(dir.path().join("tmp")).expect("a TMPDIR");
        Self { dir }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Where the key is kept.
    pub fn config_dir(&self) -> PathBuf {
        self.path("config/linearpen")
    }

    /// The record the stand-in `curl` writes.
    pub fn log(&self) -> PathBuf {
        self.path("standin.log")
    }

    /// A command, in the environment of the tests.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        let path = format!("{}:/usr/bin:/bin", stand_in_bin().display());
        command
            .env_clear()
            .current_dir(self.dir.path())
            .env("PATH", path)
            .env("HOME", self.dir.path())
            .env("TMPDIR", self.path("tmp"))
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("XDG_STATE_HOME", self.path("state"))
            .env("XDG_DATA_HOME", self.path("data"))
            .env("XDG_CACHE_HOME", self.path("cache"))
            .env("LINEARPEN_CONFIG_DIR", self.config_dir())
            .env("KABLO_DIR", self.path("kablo"))
            .env("LINEARPEN_API_URL", "http://127.0.0.1:9/graphql")
            .env("STANDIN_LOG", self.log());
        command
    }

    /// linearpen, in the environment of the tests.
    pub fn linearpen(&self) -> Command {
        self.command(program())
    }

    /// `command` run to its end.
    pub fn run(&self, command: Command) -> Ran {
        run(command, None, Duration::from_secs(60))
    }

    /// The calls the stand-in `curl` has had, in order.
    pub fn calls(&self) -> Vec<Call> {
        let log = std::fs::read_to_string(self.log()).unwrap_or_default();
        let mut calls = Vec::new();
        for block in log.split("--- call\n").skip(1) {
            let block = block.split("--- end").next().unwrap_or_default();
            let (head, stdin) = block.split_once("stdin:\n").unwrap_or((block, ""));
            let mut argv = Vec::new();
            let mut env_key = String::new();
            for line in head.lines() {
                if let Some(arg) = line.strip_prefix("argv: ") {
                    argv.push(arg.to_owned());
                } else if let Some(said) = line.strip_prefix("env LINEAR_API_KEY: ") {
                    env_key = said.to_owned();
                }
            }
            calls.push(Call {
                argv,
                env_key,
                stdin: stdin.to_owned(),
            });
        }
        calls
    }
}
