//! A new `git` worktree for the agent is the agent's to make: the pen makes none, and asks `git`
//! nothing. A project has its own rules for where a worktree goes and what its branch is called, and
//! the agent reads them (the pen cannot, and a worktree the pen made would not hold the project's
//! files that are kept out of the repository, its notes for agents among them). So the prompt asks
//! the agent for a worktree, of the repository the work is in (the pen's folder, or one under it
//! where the folder is no repository), and the file of issues gives the branch Linear suggests for
//! each issue, for a project that has no rule of its own.
//!
//! What is left here is the check of that branch name. The name is text written by others: it is
//! checked in the way `git check-ref-format --branch` does it before it is written as a suggestion,
//! and the pen gives it to no command.

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

#[cfg(test)]
mod tests {
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
}
