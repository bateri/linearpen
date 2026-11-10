//! The command line: `linearpen`, `linearpen --text`.

const USAGE: &str = "\
linearpen: your Linear issues in the terminal pane

usage: linearpen [--text]

  --text           print your issues as text and exit, also in a terminal that shows views
  -V, --version    print the version
  -h, --help       print this

In a terminal that shows pencere views (bateri) linearpen opens in the pane. In any other it
prints the issues as text and exits. The Linear API key is read from LINEAR_API_KEY, or from the
file `key` in ~/.config/linearpen. linearpen only reads: nothing is changed in Linear.
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
}
