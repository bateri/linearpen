//! The command line: `linearpen`, `linearpen --text`.

const USAGE: &str = "\
linearpen: your Linear issues in the terminal pane

usage: linearpen [--text]

  --text           print your issues as text and exit, also in a terminal that shows views
  -V, --version    print the version
  -h, --help       print this

In a terminal that shows pencere views (bateri) linearpen opens in the pane. In any other it
prints the issues as text and exits. The Linear API key is read from LINEAR_API_KEY, or from the
file `key` in ~/.config/linearpen.

linearpen writes to Linear only what you do in the pane: an issue you make with its New issue
form, a change you make to an issue (its status, priority, assignee, labels, project or title),
and the status of an issue an agent is started on, where the form says In Progress. It never
writes a description, and `linearpen --text` only reads.
";

/// What a run is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Print the issues as text and end, even where views are shown.
    pub text: bool,
}

/// What the command line comes to.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    Run(Line),
    /// Words for the person, on stdout; the program ends with 0.
    Say(String),
    /// Why the command line is not understood, on stderr; the program ends with 2.
    Refuse(String),
}

#[must_use]
pub fn read(args: impl IntoIterator<Item = String>) -> Asked {
    let mut line = Line { text: false };
    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Asked::Say(USAGE.to_owned()),
            "-V" | "--version" => {
                return Asked::Say(format!("linearpen {}\n", env!("CARGO_PKG_VERSION")));
            }
            "--text" => line.text = true,
            other => {
                return Asked::Refuse(format!(
                    "linearpen: {other:?} is not understood (linearpen --help says what is)"
                ));
            }
        }
    }
    Asked::Run(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_of(args: &[&str]) -> Asked {
        read(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn no_arguments_is_a_run_in_the_pane() {
        assert_eq!(read_of(&[]), Asked::Run(Line { text: false }));
    }

    #[test]
    fn text_asks_for_the_text() {
        assert_eq!(read_of(&["--text"]), Asked::Run(Line { text: true }));
    }

    #[test]
    fn help_and_version_say_and_the_rest_is_refused() {
        assert!(matches!(read_of(&["--help"]), Asked::Say(words) if words.contains("--text")));
        assert!(matches!(read_of(&["-V"]), Asked::Say(words) if words.starts_with("linearpen ")));
        assert!(matches!(read_of(&["--json"]), Asked::Refuse(_)));
    }

    // The pen makes issues, changes them and moves the ones an agent is started on: the help may
    // not say that it writes less than it does.
    #[test]
    fn help_says_everything_linearpen_writes() {
        // (The words as one line: where a line of the help ends is not what it says.)
        let says = |named: &str| {
            matches!(read_of(&["--help"]), Asked::Say(words)
                if words.split_whitespace().collect::<Vec<_>>().join(" ").contains(named))
        };
        for named in [
            "New issue",
            "status",
            "priority",
            "assignee",
            "labels",
            "project",
            "title",
            "agent",
        ] {
            assert!(says(named), "the help does not name {named}");
        }
        assert!(says("never writes a description"));
        assert!(!says("one thing") && !says("nothing else is changed"));
    }
}
