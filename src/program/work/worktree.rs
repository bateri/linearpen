//! A new `git` worktree for the agent, beside the repository the pen was started in: the folder is
//! `<repository>/../<repository name>-<identifier in lower case>`, on the branch Linear names for the
//! issue (or the identifier, where Linear names none), made from `HEAD` where the branch is new.
//!
//! The name of a branch is text written by others. It is checked here, in the way `git
//! check-ref-format --branch` does it, before any `git` is run, and `git` is asked once more of a
//! name that passes; every word is an argument of its own, never part of a shell line.

use std::path::{Path, PathBuf};

use super::runner::Runner;

/// The longest branch name taken.
const BRANCH_MOST: usize = 200;

/// What a new worktree is: where it goes and the branch it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The top of the repository.
    pub root: PathBuf,
    /// The folder of the worktree.
    pub path: PathBuf,
    pub branch: String,
}

/// Whether `name` is a branch name `git` takes (the rules of `git check-ref-format`), and one that
/// is not an option when it is given to a command: the words say what is wrong, or `Ok`.
///
/// # Errors
///
/// What is wrong with the name.
pub fn check_branch(name: &str) -> Result<(), String> {
    let wrong = |why: &str| Err(format!("the branch name is not one git takes: {why}"));
    if name.is_empty() || name.chars().count() > BRANCH_MOST {
        return wrong("it is empty or too long");
    }
    if name.starts_with('-') {
        return wrong("it begins with a dash");
    }
    if name == "@" {
        return wrong("it is @");
    }
    if name.starts_with('/') || name.ends_with('/') || name.contains("//") {
        return wrong("it has an empty part");
    }
    if name.ends_with('.') {
        return wrong("it ends with a dot");
    }
    for bad in ["..", "@{"] {
        if name.contains(bad) {
            return wrong(&format!("it has {bad}"));
        }
    }
    if let Some(letter) = name
        .chars()
        .find(|c| c.is_control() || " ~^:?*[\\".contains(*c))
    {
        return wrong(&format!("it has {letter:?}"));
    }
    for part in name.split('/') {
        if part.starts_with('.') || part.ends_with(".lock") {
            return wrong("a part of it begins with a dot or ends with .lock");
        }
    }
    Ok(())
}

/// The identifier as the name of a folder and of a branch: lower case, letters, digits and dashes.
fn slug(identifier: &str) -> Option<String> {
    let lower = identifier.to_ascii_lowercase();
    let fine = !lower.is_empty()
        && lower.len() <= 40
        && lower.starts_with(|c: char| c.is_ascii_alphanumeric())
        && lower
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    fine.then_some(lower)
}

/// The top of the repository that holds `folder`.
fn root_of(runner: &dyn Runner, folder: &Path) -> Result<PathBuf, String> {
    let ran = runner
        .run(
            "git",
            &["rev-parse".to_owned(), "--show-toplevel".to_owned()],
            Some(folder),
        )
        .map_err(|_| "git could not be run".to_owned())?;
    let top = ran.out.trim();
    if ran.ok && !top.is_empty() {
        Ok(PathBuf::from(top))
    } else {
        Err("this folder is not in a git repository".to_owned())
    }
}

/// What a new worktree for the issue `identifier` would be, from `folder`. Nothing is made.
///
/// # Errors
///
/// Why there can be none: the branch or the identifier is not a name to use (no `git` is run for
/// it), or the folder is not in a repository.
pub fn offer(
    runner: &dyn Runner,
    folder: &Path,
    identifier: &str,
    branch: Option<&str>,
) -> Result<Offer, String> {
    let slug =
        slug(identifier).ok_or_else(|| "the issue's identifier is not a name to use".to_owned())?;
    let branch = branch
        .filter(|name| !name.is_empty())
        .map_or(slug.clone(), str::to_owned);
    check_branch(&branch)?;
    let root = root_of(runner, folder)?;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "the repository has no name".to_owned())?;
    let beside = root
        .parent()
        .ok_or_else(|| "the repository has no folder beside it".to_owned())?;
    let path = beside.join(format!("{name}-{slug}"));
    Ok(Offer { root, path, branch })
}

/// Makes the worktree: on the branch where there is one, else on a new one from `HEAD`. It says
/// whether the branch is a new one (`undo` takes it away again).
///
/// # Errors
///
/// The words of why it could not be made (the folder is taken, `git` refused).
pub fn create(runner: &dyn Runner, offer: &Offer) -> Result<bool, String> {
    let git = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|word| (*word).to_owned()).collect();
        runner
            .run("git", &args, Some(&offer.root))
            .map_err(|_| "git could not be run".to_owned())
    };
    // The authority on the name is `git`'s own, asked once more of one that passed.
    if !git(&["check-ref-format", "--branch", &offer.branch])?.ok {
        return Err("the branch name is not one git takes".to_owned());
    }
    if offer.path.exists() {
        return Err(format!("{} is taken", offer.path.display()));
    }
    let path = offer.path.display().to_string();
    let exists = git(&[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("refs/heads/{}", offer.branch),
    ])?
    .ok;
    let ran = if exists {
        git(&["worktree", "add", &path, &offer.branch])?
    } else {
        git(&["worktree", "add", "-b", &offer.branch, &path, "HEAD"])?
    };
    if ran.ok {
        Ok(!exists)
    } else {
        Err(ran.reason("git did not make the worktree"))
    }
}

/// Takes away what `create` made, when what was to run in it could not be started: the worktree
/// (which `git` takes away only if nothing in it has changed), and the branch if it was made new
/// (which `git` deletes only if it holds nothing that `HEAD` does not). Nothing of it is reported:
/// what is left is left for the person, who is told that the start failed.
pub fn undo(runner: &dyn Runner, offer: &Offer, new_branch: bool) {
    let git = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|word| (*word).to_owned()).collect();
        runner.run("git", &args, Some(&offer.root))
    };
    let path = offer.path.display().to_string();
    if git(&["worktree", "remove", &path]).is_ok_and(|ran| ran.ok) && new_branch {
        let _ = git(&["branch", "-d", &offer.branch]);
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::Mutex;

    use super::super::runner::Ran;
    use super::*;

    /// A runner that answers as a repository would, and keeps what it was asked.
    struct Asked(Mutex<Vec<String>>);

    impl Runner for Asked {
        fn run(&self, program: &str, args: &[String], _: Option<&Path>) -> std::io::Result<Ran> {
            self.0
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            Ok(Ran {
                ok: true,
                out: "/work/acme-app\n".to_owned(),
                err: String::new(),
            })
        }
    }

    #[test]
    fn the_names_git_refuses_are_refused() {
        for fine in [
            "acm-412",
            "ada/acm-412-add-rate-limiting",
            "a.b",
            "feature/x_y",
            "ünï",
        ] {
            assert_eq!(check_branch(fine), Ok(()), "{fine}");
        }
        for bad in [
            "",
            "-x",
            "--orphan",
            "a..b",
            "a b",
            "a~1",
            "a^",
            "a:b",
            "a?",
            "a*",
            "a[0]",
            "a\\b",
            "/a",
            "a/",
            "a//b",
            "a.",
            "@",
            "a@{b",
            ".hidden",
            "a/.b",
            "x.lock",
            "a/b.lock",
            "tab\there",
            "line\nbreak",
        ] {
            assert!(check_branch(bad).is_err(), "{bad:?}");
        }
        assert!(check_branch(&"a".repeat(201)).is_err());
    }

    #[test]
    fn a_name_that_is_wrong_is_refused_before_git_is_run() {
        let asked = Asked(Mutex::new(Vec::new()));
        for bad in ["-x", "a..b", "has space"] {
            let why = offer(&asked, Path::new("/work/acme-app"), "ACM-412", Some(bad)).unwrap_err();
            assert!(why.contains("branch name"), "{why}");
        }
        assert!(offer(&asked, Path::new("/w"), "../etc", None).is_err());
        assert!(
            asked.0.lock().unwrap().is_empty(),
            "{:?}",
            asked.0.lock().unwrap()
        );
    }

    #[test]
    fn the_folder_is_beside_the_repository_and_named_after_it_and_the_issue() {
        let asked = Asked(Mutex::new(Vec::new()));
        let found = offer(
            &asked,
            Path::new("/work/acme-app/api"),
            "ACM-412",
            Some("ada/acm-412-add-rate-limiting"),
        )
        .unwrap();
        assert_eq!(found.path, Path::new("/work/acme-app-acm-412"));
        assert_eq!(found.branch, "ada/acm-412-add-rate-limiting");
        // Where Linear names no branch, the identifier does.
        let plain = offer(&asked, Path::new("/work/acme-app"), "ACM-7", None).unwrap();
        assert_eq!(plain.branch, "acm-7");
        assert_eq!(asked.0.lock().unwrap()[0], "git rev-parse --show-toplevel");
    }

    /// `git` as the tests have it: with none of the configuration of the machine they run on (a
    /// hook, a signing key, an identity) and a name of its own to commit as.
    fn isolate(command: &mut Command) -> &mut Command {
        command
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
    }

    /// The runner of the program, with `git` isolated: what the code under test runs is the same.
    struct Isolated;

    impl Runner for Isolated {
        fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<Ran> {
            let mut command = super::super::runner::command_of(program, args, cwd);
            let output = isolate(&mut command).output()?;
            Ok(Ran {
                ok: output.status.success(),
                out: String::from_utf8_lossy(&output.stdout).into_owned(),
                err: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    fn git(dir: &Path, args: &[&str]) -> bool {
        isolate(Command::new("git").args(args).current_dir(dir))
            .stdin(std::process::Stdio::null())
            .output()
            .is_ok_and(|out| out.status.success())
    }

    /// A repository of its own in a temporary folder, with one commit.
    fn repository() -> (tempfile::TempDir, PathBuf) {
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("acme-app");
        std::fs::create_dir(&root).unwrap();
        assert!(git(&root, &["init", "-q", "-b", "main"]));
        std::fs::write(root.join("a.txt"), "a").unwrap();
        assert!(git(&root, &["add", "a.txt"]));
        assert!(git(&root, &["commit", "-q", "-m", "first"]));
        // The path git tells may be the real one of a link (macOS keeps /var as /private/var).
        let root = root.canonicalize().unwrap();
        (base, root)
    }

    #[test]
    fn a_worktree_is_made_on_a_new_branch_and_on_one_that_is_there() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let (_base, root) = repository();
        let found = offer(&Isolated, &root, "ACM-412", Some("ada/acm-412-add")).unwrap();
        assert_eq!(found.root, root);
        assert!(create(&Isolated, &found).unwrap(), "the branch is new");
        assert!(found.path.join("a.txt").exists());
        // The folder is taken now.
        assert!(create(&Isolated, &found).unwrap_err().contains("taken"));
        // A branch that is there is used as it is.
        assert!(git(&root, &["branch", "ready"]));
        let again = offer(&Isolated, &root, "ACM-9", Some("ready")).unwrap();
        assert!(!create(&Isolated, &again).unwrap(), "the branch was there");
        assert!(again.path.join("a.txt").exists());
    }

    #[test]
    fn what_was_made_is_taken_away_and_a_branch_that_was_there_is_kept() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let (_base, root) = repository();
        let made = offer(&Isolated, &root, "ACM-412", Some("ada/acm-412-add")).unwrap();
        let new_branch = create(&Isolated, &made).unwrap();
        undo(&Isolated, &made, new_branch);
        assert!(!made.path.exists());
        assert!(!git(
            &root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/heads/ada/acm-412-add"
            ]
        ));
        // Made again, it is free to be.
        assert!(create(&Isolated, &made).unwrap());
        // A branch that was there stays.
        assert!(git(&root, &["branch", "ready"]));
        let kept = offer(&Isolated, &root, "ACM-9", Some("ready")).unwrap();
        let new_branch = create(&Isolated, &kept).unwrap();
        undo(&Isolated, &kept, new_branch);
        assert!(!kept.path.exists());
        assert!(git(
            &root,
            &["rev-parse", "--verify", "--quiet", "refs/heads/ready"]
        ));
    }

    #[test]
    fn a_folder_outside_a_repository_has_no_worktree() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let plain = tempfile::tempdir().unwrap();
        let why = offer(&Isolated, plain.path(), "ACM-1", None).unwrap_err();
        assert!(why.contains("not in a git repository"), "{why}");
    }
}
