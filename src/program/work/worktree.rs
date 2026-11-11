//! A new `git` worktree for the agent is the agent's to make: the pen makes none. A project has its
//! own rules for where a worktree goes and what its branch is called, and the agent reads them (the
//! pen cannot, and a worktree the pen made would not hold the project's files that are kept out of
//! the repository, its notes for agents among them). So the prompt asks the agent for a worktree,
//! and the file of issues gives the branch Linear suggests for each issue, for a project that has no
//! rule of its own.
//!
//! What is left here is whether the pen's folder is in a repository at all (where it is not, a
//! worktree is not offered) and the check of a branch name. The name is text written by others: it
//! is checked in the way `git check-ref-format --branch` does it before it is written as a
//! suggestion, and the pen gives it to no command.

use std::path::Path;

use super::runner::Runner;

/// The longest branch name taken.
const BRANCH_MOST: usize = 200;

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

/// The branch Linear names for an issue, where it names one that `git` would take: what the file of
/// issues says of it. A name that is wrong is left out, not mended.
#[must_use]
pub fn suggested(branch: Option<&str>) -> Option<&str> {
    branch.filter(|name| check_branch(name).is_ok())
}

/// Whether the agent can be asked for a new worktree from `folder`: it is in a `git` repository.
///
/// # Errors
///
/// The words of why not.
pub fn available(runner: &dyn Runner, folder: &Path) -> Result<(), String> {
    let ran = runner
        .run(
            "git",
            &["rev-parse".to_owned(), "--show-toplevel".to_owned()],
            Some(folder),
        )
        .map_err(|_| "git could not be run".to_owned())?;
    if ran.ok && !ran.out.trim().is_empty() {
        Ok(())
    } else {
        Err("this folder is not in a git repository".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use super::super::runner::Ran;
    use super::*;

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
    fn a_name_linear_gives_is_suggested_only_where_git_would_take_it() {
        assert_eq!(suggested(Some("ada/acm-412-add")), Some("ada/acm-412-add"));
        for bad in [None, Some(""), Some("-x"), Some("a..b"), Some("has space")] {
            assert_eq!(suggested(bad), None, "{bad:?}");
        }
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
    fn a_worktree_can_be_asked_for_in_a_repository_and_not_outside_one() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let (_base, root) = repository();
        assert_eq!(available(&Isolated, &root), Ok(()));
        let plain = tempfile::tempdir().unwrap();
        let why = available(&Isolated, plain.path()).unwrap_err();
        assert!(why.contains("not in a git repository"), "{why}");
    }
}
