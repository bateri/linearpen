use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Mutex;

use super::runner::{Ran, Runner};
use super::*;
use crate::linear::types::fixture_issue;

/// A runner that runs nothing: it answers as `bateri` and `git` would, and keeps what it was asked.
struct Recorder {
    asked: Mutex<Vec<String>>,
    /// What `bateri split` says: its output, and whether it went well.
    pane: Mutex<(bool, String)>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            asked: Mutex::new(Vec::new()),
            pane: Mutex::new((true, "p41\n".to_owned())),
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Runner for Recorder {
    fn run(&self, program: &str, args: &[String], _: Option<&Path>) -> std::io::Result<Ran> {
        self.asked
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        let say = |out: &str| Ran {
            ok: true,
            out: out.to_owned(),
            err: String::new(),
        };
        Ok(match (program, args.first().map(String::as_str)) {
            ("bateri", Some("split" | "tab")) => {
                let (ok, said) = self.pane.lock().unwrap().clone();
                if ok {
                    say(&said)
                } else {
                    Ran {
                        ok: false,
                        out: String::new(),
                        err: said,
                    }
                }
            }
            ("git", Some("rev-parse"))
                if args.get(1).map(String::as_str) == Some("--show-toplevel") =>
            {
                say("/work/acme-app\n")
            }
            ("git", Some("rev-parse")) => Ran {
                ok: false,
                out: String::new(),
                err: String::new(),
            },
            _ => say(""),
        })
    }
}

/// A directory of the `PATH` with the programs `names`, which can be run.
fn bin(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in names {
        let file = dir.path().join(name);
        std::fs::write(&file, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn setup(bin: &tempfile::TempDir, in_bateri: bool) -> Setup {
    Setup {
        path: vec![bin.path().to_owned()],
        in_bateri,
        folder: Some(PathBuf::from("/work/acme-app/api")),
    }
}

fn request(issues: &[Issue], folder: Folder) -> Request<'_> {
    Request {
        issues,
        prompt: "Start with the limiter.",
        agent: Agent::Claude,
        folder,
        place: Place::Split,
    }
}

#[test]
fn nothing_is_started_where_bateri_or_every_agent_is_missing() {
    let full = bin(&["bateri", "claude"]);
    assert_eq!(setup(&full, true).unavailable(), None);
    assert!(
        setup(&full, false)
            .unavailable()
            .unwrap()
            .contains("not in a bateri pane")
    );
    let no_bateri = bin(&["claude"]);
    assert_eq!(
        setup(&no_bateri, true).unavailable().as_deref(),
        Some("bateri is not in your PATH")
    );
    let no_agent = bin(&["bateri"]);
    assert!(
        setup(&no_agent, true)
            .unavailable()
            .unwrap()
            .contains("No agent is installed")
    );
}

#[test]
fn the_setup_is_read_from_the_environment_it_is_given() {
    let vars = [("PATH", "/a::/b"), ("BATERI_TAB_URL", "unix:/x")];
    let get = |name: &str| {
        vars.iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| (*v).to_owned())
    };
    let found = Setup::resolve(&get, Some(PathBuf::from("/here")));
    assert_eq!(found.path, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    assert!(found.in_bateri);
    let empty = |name: &str| (name == "BATERI_TAB_URL").then(String::new);
    assert!(!Setup::resolve(&empty, None).in_bateri);
}

#[test]
fn an_agent_is_started_in_a_pane_with_a_prompt_that_names_the_file_and_never_holds_the_issue() {
    let bin = bin(&["bateri", "claude", "codex"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Recorder::new();
    let setup = setup(&bin, true);
    let mut issue = fixture_issue();
    issue.description = Some("SECRET-DESCRIPTION written by others".to_owned());
    let issues = [issue];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 1_762_830_900,
    };
    let started = start(&env, &request(&issues, Folder::Here)).unwrap();
    assert_eq!(started.pane, "p41");
    assert_eq!(started.folder, Path::new("/work/acme-app/api"));
    assert_eq!(
        started.context,
        state.path().join("work/20251111T031500Z-ACM-412/issues.md")
    );
    // The issue is in the file, and only there.
    let file = std::fs::read_to_string(&started.context).unwrap();
    assert!(file.contains("SECRET-DESCRIPTION"), "{file}");
    let asked = runner.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    let line = &asked[0];
    assert!(
        line.starts_with("bateri split --right --cwd /work/acme-app/api --label ACM-412 -- claude Work on the Linear issues in "),
        "{line}"
    );
    assert!(
        line.contains(&started.context.display().to_string()),
        "{line}"
    );
    assert!(line.ends_with("Start with the limiter."), "{line}");
    assert!(!line.contains("SECRET-DESCRIPTION"), "{line}");
    assert!(
        !line.contains("dangerously") && !line.contains("--yolo") && !line.contains("--focus"),
        "{line}"
    );
}

#[test]
fn a_new_worktree_is_the_agents_to_make_the_pen_makes_none_and_the_prompt_asks_for_one() {
    let bin = bin(&["bateri", "claude"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Recorder::new();
    let setup = setup(&bin, true);
    let issues = [fixture_issue()];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 0,
    };
    let started = start(&env, &request(&issues, Folder::Worktree)).unwrap();
    // The agent starts in the pen's folder, where the project's own notes for it are.
    assert_eq!(started.folder, Path::new("/work/acme-app/api"));
    let asked = runner.asked();
    // The pen asks `git` one thing, whether the folder is in a repository, and makes nothing.
    assert_eq!(asked[0], "git rev-parse --show-toplevel");
    for made in [
        "git worktree",
        "git branch",
        "git check-ref-format",
        "git checkout",
    ] {
        assert!(
            !asked.iter().any(|line| line.starts_with(made)),
            "{made}: {asked:?}"
        );
    }
    let split = asked.last().unwrap();
    assert!(split.starts_with("bateri split"), "{asked:?}");
    assert!(split.contains("--cwd /work/acme-app/api "), "{asked:?}");
    assert!(split.contains(handoff::WORKTREE), "{asked:?}");
    assert!(split.ends_with("Start with the limiter."), "{asked:?}");
    // The branch Linear suggests is in the file, not in the command.
    assert!(!split.contains("ada/acm-412"), "{asked:?}");
    let text = std::fs::read_to_string(&started.context).unwrap();
    assert!(
        text.contains("- Branch Linear suggests: ada/acm-412-add-rate-limiting"),
        "{text}"
    );
    // In this folder nothing of a worktree is said.
    let here = start(&env, &request(&issues, Folder::Here)).unwrap();
    assert_eq!(here.folder, Path::new("/work/acme-app/api"));
    assert!(!runner.asked().last().unwrap().contains("worktree"));
}

#[test]
fn a_branch_name_git_would_not_take_is_left_out_of_the_file_and_reaches_no_command() {
    let bin = bin(&["bateri", "claude"]);
    let state = tempfile::tempdir().unwrap();
    let setup = setup(&bin, true);
    for bad in ["-x", "a..b", "has space"] {
        let runner = Recorder::new();
        let mut issue = fixture_issue();
        issue.branch_name = Some(bad.to_owned());
        let issues = [issue];
        let env = Env {
            setup: &setup,
            runner: &runner,
            state: Some(state.path()),
            now: 0,
        };
        let started = start(&env, &request(&issues, Folder::Worktree)).unwrap();
        let text = std::fs::read_to_string(&started.context).unwrap();
        assert!(!text.contains("Branch Linear suggests"), "{bad}: {text}");
        for line in runner.asked() {
            assert!(!line.contains(bad), "{bad}: {line}");
        }
    }
}

#[test]
fn outside_a_repository_a_new_worktree_is_refused_before_anything_is_made() {
    /// `git` that says the folder is in no repository.
    struct Nowhere(Recorder);
    impl Runner for Nowhere {
        fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<Ran> {
            if program == "git" {
                self.0.run(program, &["rev-parse".to_owned()], cwd)
            } else {
                self.0.run(program, args, cwd)
            }
        }
    }
    let bin = bin(&["bateri", "claude"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Nowhere(Recorder::new());
    let setup = setup(&bin, true);
    let issues = [fixture_issue()];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 0,
    };
    let refused = start(&env, &request(&issues, Folder::Worktree)).unwrap_err();
    assert!(
        matches!(&refused, Refused::Failed(why) if why.contains("not in a git repository")),
        "{refused:?}"
    );
    // No pane, no file.
    assert!(
        !runner
            .0
            .asked()
            .iter()
            .any(|line| line.starts_with("bateri"))
    );
    assert!(!state.path().join("work").exists());
}

#[test]
fn where_bateri_says_no_the_words_are_its_own() {
    let bin = bin(&["bateri", "claude"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Recorder::new();
    *runner.pane.lock().unwrap() = (false, "\nbateri: no pane for you\n".to_owned());
    let setup = setup(&bin, true);
    let issues = [fixture_issue()];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 0,
    };
    assert_eq!(
        start(&env, &request(&issues, Folder::Here)).unwrap_err(),
        Refused::Failed("bateri: no pane for you".to_owned())
    );
    // An answer that is not a pane's id is not taken for one, and a pane that may be there keeps
    // the file it was told to read.
    *runner.pane.lock().unwrap() = (true, "ok\n".to_owned());
    assert!(matches!(
        start(&env, &request(&issues, Folder::Here)),
        Err(Refused::Failed(_))
    ));
    let kept = std::fs::read_dir(state.path().join("work"))
        .unwrap()
        .count();
    assert_eq!(kept, 1, "the file of a pane that may be there stays");
}

#[test]
fn where_the_pane_is_refused_what_was_made_for_it_is_taken_away() {
    let bin = bin(&["bateri", "claude"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Recorder::new();
    *runner.pane.lock().unwrap() = (false, "bateri: no pane for you".to_owned());
    let setup = setup(&bin, true);
    let issues = [fixture_issue()];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 0,
    };
    assert!(start(&env, &request(&issues, Folder::Worktree)).is_err());
    // The pen made nothing with `git`, so there is nothing of `git`'s to take away.
    let asked = runner.asked();
    assert!(
        !asked
            .iter()
            .any(|line| line.starts_with("git worktree") || line.starts_with("git branch")),
        "{asked:?}"
    );
    // The file of the issues is not left behind: nothing is in the folder of the files.
    let left = std::fs::read_dir(state.path().join("work"))
        .unwrap()
        .count();
    assert_eq!(left, 0);
}

#[test]
fn an_agent_that_is_not_installed_is_not_started() {
    let bin = bin(&["bateri", "codex"]);
    let state = tempfile::tempdir().unwrap();
    let runner = Recorder::new();
    let setup = setup(&bin, true);
    let issues = [fixture_issue()];
    let env = Env {
        setup: &setup,
        runner: &runner,
        state: Some(state.path()),
        now: 0,
    };
    let refused = start(&env, &request(&issues, Folder::Here)).unwrap_err();
    assert!(
        matches!(&refused, Refused::Unavailable(why) if why.contains("Claude is not installed")),
        "{refused:?}"
    );
    assert!(runner.asked().is_empty());
}
