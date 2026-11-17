//! Reading the words after a command's name: its options, with or without a value, and what is left
//! over. It knows what the options of a command are and nothing of what they mean.

/// One option of a command.
#[derive(Debug, Clone, Copy)]
pub struct Flag {
    /// Written `--name`.
    pub name: &'static str,
    /// Whether a value follows (`--name value` or `--name=value`).
    pub value: bool,
    /// Whether it may be given more than once.
    pub repeats: bool,
}

/// An option that is on or off.
pub const fn switch(name: &'static str) -> Flag {
    Flag {
        name,
        value: false,
        repeats: false,
    }
}

/// An option with one value.
pub const fn one(name: &'static str) -> Flag {
    Flag {
        name,
        value: true,
        repeats: false,
    }
}

/// An option with a value that may be given more than once.
pub const fn many(name: &'static str) -> Flag {
    Flag {
        name,
        value: true,
        repeats: true,
    }
}

/// What was read: each option given with its value, and the words that are none.
#[derive(Debug, Default)]
pub struct Scanned {
    given: Vec<(&'static str, String)>,
    pub positional: Vec<String>,
    /// `-h` or `--help` was among the words.
    pub help: bool,
}

impl Scanned {
    /// Whether the option was given.
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.given.iter().any(|(given, _)| *given == name)
    }

    /// The value of an option given once.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&str> {
        self.given
            .iter()
            .find(|(given, _)| *given == name)
            .map(|(_, value)| value.as_str())
    }

    /// The values of an option, in the order they were given.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<String> {
        self.given
            .iter()
            .filter(|(given, _)| *given == name)
            .map(|(_, value)| value.clone())
            .collect()
    }
}

/// Reads `args` against the options `spec` of the command `command`. A word that begins with `--`
/// is an option, `-` alone and every other word is a value or a word of the command, and what comes
/// after a lone `--` is the command's whole.
///
/// # Errors
/// An option the command does not have, one that lacks its value or has one it does not take, and
/// one given twice that may be given once.
pub fn scan(command: &str, args: &[String], spec: &[Flag]) -> Result<Scanned, String> {
    let mut out = Scanned::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            out.positional.extend(args.by_ref().cloned());
            break;
        }
        if arg == "-h" || arg == "--help" {
            out.help = true;
            continue;
        }
        let Some(rest) = arg.strip_prefix("--") else {
            if arg.starts_with('-') && arg != "-" {
                return Err(format!(
                    "{arg:?} is not an option of {command} (linearpen {command} --help says which are)"
                ));
            }
            out.positional.push(arg.clone());
            continue;
        };
        let (name, inline) = match rest.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (rest, None),
        };
        let Some(flag) = spec.iter().find(|flag| flag.name == name) else {
            return Err(format!(
                "--{name} is not an option of {command} (linearpen {command} --help says which are)"
            ));
        };
        if !flag.repeats && out.has(flag.name) {
            return Err(format!("--{name} is given more than once"));
        }
        let value = if flag.value {
            match inline {
                Some(value) => value,
                None => args
                    .next()
                    .cloned()
                    .ok_or_else(|| format!("--{name} needs a value"))?,
            }
        } else if inline.is_some() {
            return Err(format!("--{name} takes no value"));
        } else {
            String::new()
        };
        out.given.push((flag.name, value));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: [Flag; 3] = [switch("json"), one("title"), many("label")];

    fn words(of: &[&str]) -> Vec<String> {
        of.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn options_values_and_words_are_told_apart() {
        let got = scan(
            "x",
            &words(&[
                "A-1",
                "--label",
                "a",
                "--json",
                "--title=T",
                "--label=b",
                "-",
            ]),
            &SPEC,
        )
        .unwrap();
        assert!(got.has("json"));
        assert_eq!(got.value("title"), Some("T"));
        assert_eq!(got.values("label"), ["a", "b"]);
        assert_eq!(got.positional, ["A-1", "-"]);
    }

    #[test]
    fn a_value_may_begin_with_dashes_and_a_double_dash_ends_the_options() {
        let got = scan("x", &words(&["--title", "--odd", "--", "--json"]), &SPEC).unwrap();
        assert_eq!(got.value("title"), Some("--odd"));
        assert!(!got.has("json"));
        assert_eq!(got.positional, ["--json"]);
    }

    #[test]
    fn what_a_command_does_not_have_or_lacks_is_refused() {
        let refused = |of: &[&str]| scan("x", &words(of), &SPEC).unwrap_err();
        assert!(refused(&["--nope"]).contains("--nope is not an option of x"));
        assert!(refused(&["-q"]).contains("not an option"));
        assert!(refused(&["--title"]).contains("needs a value"));
        assert!(refused(&["--title", "a", "--title", "b"]).contains("more than once"));
        assert!(refused(&["--json=1"]).contains("takes no value"));
    }

    #[test]
    fn help_is_noticed_wherever_it_stands() {
        assert!(scan("x", &words(&["--json", "-h"]), &SPEC).unwrap().help);
        assert!(scan("x", &words(&["--help"]), &SPEC).unwrap().help);
    }
}
