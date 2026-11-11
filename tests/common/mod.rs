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
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

/// The program the tests run.
pub fn program() -> &'static str {
    env!("CARGO_BIN_EXE_linearpen")
}

/// The directory of the stand-in commands.
pub fn stand_in_bin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test/stand-in/bin")
}

/// The stand-in terminal (`pencere-fake-host`), checked once: the one `FAKE_HOST` names, or the one
/// `make fake-host` builds in `.cache/pencere`. Nothing is skipped: a stand-in that is not there, or
/// one too old to show a whole result, fails the run and says which, since a gate that passes because
/// it did not look proves nothing.
pub fn host() -> &'static Path {
    static HOST: OnceLock<PathBuf> = OnceLock::new();
    HOST.get_or_init(|| {
        // A path from here (the package, where Cargo runs the tests), since the stand-in is started
        // in the directory of each test.
        let host = match std::env::var_os("FAKE_HOST") {
            Some(path) if !path.is_empty() => {
                std::path::absolute(&path).unwrap_or_else(|_| PathBuf::from(path))
            }
            _ => Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".cache/pencere/target/debug/pencere-fake-host"),
        };
        assert!(
            host.is_file(),
            "the tests cannot run: pencere-fake-host is not built ({}): make fake-host, or FAKE_HOST=<a stand-in built in a pencere checkout>",
            host.display()
        );
        let help = Command::new(&host)
            .arg("--help")
            .output()
            .unwrap_or_else(|why| panic!("{} cannot be run: {why}", host.display()));
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&help.stdout),
            String::from_utf8_lossy(&help.stderr)
        );
        assert!(
            said.contains("--subscribe") && said.contains("PENCERE_FAKE_GIST"),
            "the tests cannot run: pencere-fake-host is too old ({})",
            host.display()
        );
        host
    })
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

    /// The record the stand-in `bateri` writes.
    pub fn bateri_log(&self) -> PathBuf {
        self.path("bateri.log")
    }

    /// The calls the stand-in `bateri` has had, in order: the arguments of each.
    pub fn bateri_calls(&self) -> Vec<Vec<String>> {
        let record = std::fs::read(self.bateri_log()).unwrap_or_default();
        let mut calls = Vec::new();
        let mut call = Vec::new();
        for word in record
            .split(|byte| *byte == 0)
            .filter(|word| !word.is_empty())
        {
            if word == [1] {
                calls.push(std::mem::take(&mut call));
            } else {
                call.push(String::from_utf8_lossy(word).into_owned());
            }
        }
        calls
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
            .env("STANDIN_LOG", self.log())
            .env("STANDIN_BATERI_LOG", self.bateri_log());
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

/// What the page asks of the program, and how the pane ends.
pub struct Script {
    calls: Vec<(String, Value)>,
    then: Vec<String>,
    limit: u64,
    env: Vec<(String, String)>,
    args: Vec<String>,
    host_args: Vec<String>,
}

impl Script {
    /// The page makes no calls; then ⌃C is typed; linearpen is run with no arguments.
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            then: vec!["--then".to_owned(), "interrupt".to_owned()],
            limit: 25,
            env: Vec::new(),
            args: Vec::new(),
            host_args: Vec::new(),
        }
    }

    /// The page calls `method` with `params`, after the answer to the call before.
    pub fn call(mut self, method: &str, params: Value) -> Self {
        self.calls.push((method.to_owned(), params));
        self
    }

    /// What happens when the page has its answers (the stand-in's `--then` and the like).
    pub fn then(mut self, then: &[&str]) -> Self {
        self.then = then.iter().map(|word| (*word).to_owned()).collect();
        self
    }

    /// How long the stand-in lets the program run, in seconds.
    pub fn limit(mut self, seconds: u64) -> Self {
        self.limit = seconds;
        self
    }

    /// A variable of the environment the program is run in (on top of the home's).
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    /// More of the stand-in's options (`--subscribe issues`).
    pub fn host_args(mut self, args: &[&str]) -> Self {
        self.host_args = args.iter().map(|arg| (*arg).to_owned()).collect();
        self
    }
}

/// What the page heard in answer to a call.
#[derive(Debug, Clone)]
pub enum Answer {
    Result(Value),
    Error {
        code: String,
        message: Option<String>,
    },
}

impl Answer {
    /// The result of a call that was answered with one.
    pub fn result(&self) -> &Value {
        match self {
            Self::Result(value) => value,
            Self::Error { code, message } => panic!("an error, {code}: {message:?}"),
        }
    }

    /// Whether the call failed with `code`.
    pub fn failed(&self, code: &str) -> bool {
        matches!(self, Self::Error { code: said, .. } if said == code)
    }

    /// The words of an error.
    pub fn message(&self) -> &str {
        match self {
            Self::Error {
                message: Some(message),
                ..
            } => message,
            other => panic!("no words: {other:?}"),
        }
    }
}

/// The answers the page heard, in order, in the stand-in's record.
pub fn answers(log: &str) -> Vec<Answer> {
    let mut found = Vec::new();
    for line in log.lines() {
        let Some(rest) = line.strip_prefix("view: <- response id=") else {
            continue;
        };
        let Some((_, said)) = rest.split_once(' ') else {
            continue;
        };
        if let Some(result) = said.strip_prefix("result=") {
            found.push(Answer::Result(
                serde_json::from_str(result).expect("a whole result"),
            ));
        } else if let Some(error) = said.strip_prefix("error code=") {
            let (code, message) = match error.split_once(" message=") {
                Some((code, message)) => (
                    code.to_owned(),
                    Some(serde_json::from_str(message).expect("words that are a JSON string")),
                ),
                None => (error.to_owned(), None),
            };
            found.push(Answer::Error { code, message });
        }
    }
    found
}

/// The values the page heard pushed, in order.
pub fn pushes(log: &str) -> Vec<Value> {
    log.lines()
        .filter_map(|line| line.strip_prefix("view: <- push id="))
        .filter_map(|rest| rest.split_once(" value="))
        .map(|(_, value)| serde_json::from_str(value).expect("a whole value"))
        .collect()
}

impl Home {
    /// The stand-in terminal, ready to run the pane of `script`, in the environment of the tests.
    pub fn host_command(&self, script: &Script) -> Command {
        let mut command = self.command(host());
        command.env("PENCERE_FAKE_GIST", "100000000").args([
            "--size",
            "100x30",
            "--limit",
            &script.limit.to_string(),
        ]);
        command.args(&script.host_args);
        for (method, params) in &script.calls {
            command.args(["--call", &format!("{method}={params}")]);
        }
        command
            .args(&script.then)
            .arg("--")
            .arg(program())
            .args(&script.args);
        for (name, value) in &script.env {
            command.env(name, value);
        }
        command
    }

    /// The stand-in's record of a pane in which the page makes the calls of `script`.
    pub fn record(&self, script: &Script) -> String {
        run(
            self.host_command(script),
            None,
            Duration::from_secs(script.limit + 15),
        )
        .stderr
    }

    /// The answers to the calls of `script`, one for each: the record's tail says why when not.
    pub fn ask(&self, script: &Script) -> Vec<Answer> {
        let log = self.record(script);
        let heard = answers(&log);
        let tail: String = {
            let chars: Vec<char> = log.chars().collect();
            chars[chars.len().saturating_sub(1500)..].iter().collect()
        };
        assert_eq!(
            heard.len(),
            script.calls.len(),
            "every call was answered:\n{tail}"
        );
        heard
    }
}
