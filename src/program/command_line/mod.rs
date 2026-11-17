//! The command line: `linearpen`, `linearpen --text`, and the commands (`linearpen list`, `show`,
//! `create`, `update`, `comment`, `teams`, `help`), read into typed values. It knows nothing of the
//! network or of the environment: what the words are asked to do is `cli/`'s.

mod commands;
mod help;
mod scan;

pub use commands::{
    Command, Comment, Create, CycleAsk, Fields, List, ListScope, Setting, Update, Words,
};
use help::USAGE;

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
    /// A command: it is run and the program ends with what it says.
    Do(Command),
    /// Words for the person, on stdout; the program ends with 0.
    Say(String),
    /// Why the command line is not understood, on stderr; the program ends with 2.
    Refuse(String),
}

#[must_use]
pub fn read(args: impl IntoIterator<Item = String>) -> Asked {
    let args: Vec<String> = args.into_iter().collect();
    if let Some(first) = args.first() {
        if first == "help" {
            return match args.get(1).map(String::as_str) {
                None => Asked::Say(USAGE.to_owned()),
                Some(name) if args.len() == 2 => match help::of(name) {
                    Some(words) => Asked::Say(words.to_owned()),
                    None => Asked::Refuse(format!(
                        "linearpen: {name:?} is not a command (linearpen help says which are)"
                    )),
                },
                Some(_) => {
                    Asked::Refuse("linearpen: help is given one command, or none".to_owned())
                }
            };
        }
        if commands::NAMES.contains(&first.as_str()) {
            return match commands::read(first, &args[1..]) {
                commands::Read::Do(command) => Asked::Do(command),
                commands::Read::Say(words) => Asked::Say(words),
                commands::Read::Refuse(why) => Asked::Refuse(why),
            };
        }
    }
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
        assert!(matches!(read_of(&["nonsense"]), Asked::Refuse(_)));
        // A command is a command only as the first word.
        assert!(matches!(read_of(&["--text", "list"]), Asked::Refuse(_)));
    }

    #[test]
    fn help_tells_each_command_and_refuses_what_is_not_one() {
        assert!(matches!(read_of(&["help"]), Asked::Say(words) if words.contains("commands:")));
        assert!(matches!(
            read_of(&["help", "update"]),
            Asked::Say(words) if words.contains("linearpen update")
        ));
        assert!(matches!(read_of(&["help", "nope"]), Asked::Refuse(_)));
        assert!(matches!(
            read_of(&["help", "list", "show"]),
            Asked::Refuse(_)
        ));
        assert!(matches!(read_of(&["show", "--help"]), Asked::Say(_)));
        assert!(matches!(read_of(&["show", "ACM-1"]), Asked::Do(_)));
        assert!(matches!(read_of(&["show"]), Asked::Refuse(_)));
    }

    // The pen makes issues, changes them, comments and moves the ones an agent is started on: the
    // help may not say that it writes less than it does.
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
            "a comment",
            "create, update and comment",
            "never writes a description of an issue that exists",
            "never moves an issue to another team",
            "list, show and teams only read",
        ] {
            assert!(says(named), "the help does not name {named}");
        }
        assert!(!says("one thing") && !says("nothing else is changed"));
        assert!(!says("`--text` only reads"));
    }

    #[test]
    fn every_help_is_in_plain_english_with_no_ellipsis() {
        let all = [USAGE]
            .into_iter()
            .chain(commands::NAMES.iter().filter_map(|name| help::of(name)));
        for words in all {
            assert!(!words.contains("..."), "{words}");
            assert!(!words.contains('\u{2026}'), "{words}");
            assert!(words.is_ascii(), "{words}");
        }
    }
}
