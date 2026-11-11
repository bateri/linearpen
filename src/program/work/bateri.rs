//! bateri's command line, used to make a pane that runs the agent and to know when it ends:
//! `bateri split --right` (or `bateri tab`) with the folder, a label and the program after `--`,
//! which tells the id of the pane it made; `bateri wait` on that id, which returns when the pane's
//! program ends. The new pane is not given the focus (no `--focus`).

use std::path::Path;

use super::runner::Runner;

/// The program the pen runs.
pub const PROGRAM: &str = "bateri";

/// Where the pane goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Beside this pane, to the right.
    Split,
    /// In a tab of its own.
    Tab,
}

impl Place {
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "split" => Some(Self::Split),
            "tab" => Some(Self::Tab),
            _ => None,
        }
    }
}

/// The arguments that make the pane. Every word is its own argument: nothing goes through a shell.
#[must_use]
pub fn arguments(place: Place, folder: &Path, label: &str, program: &[String]) -> Vec<String> {
    let mut words = match place {
        Place::Split => vec!["split".to_owned(), "--right".to_owned()],
        Place::Tab => vec!["tab".to_owned()],
    };
    // The label is Linear's identifier: one that begins with a dash is not taken for an option.
    let label = label.trim_start_matches('-');
    let label = if label.is_empty() { "issue" } else { label };
    words.extend([
        "--cwd".to_owned(),
        folder.display().to_string(),
        "--label".to_owned(),
        label.to_owned(),
        "--".to_owned(),
    ]);
    words.extend(program.iter().cloned());
    words
}

/// Whether `word` is the id of a pane (`p41`).
fn is_pane(word: &str) -> bool {
    word.strip_prefix('p')
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// Why there is no pane.
#[derive(Debug, PartialEq, Eq)]
pub struct NoPane {
    /// The words for the person.
    pub why: String,
    /// bateri went well but did not say which pane it made: there may be one, running in the folder
    /// and on the file that were made for it, which are not to be taken away.
    pub maybe: bool,
}

/// Makes the pane that runs `program` in `folder`, and says its id.
///
/// # Errors
///
/// Why there is no pane.
pub fn launch(
    runner: &dyn Runner,
    place: Place,
    folder: &Path,
    label: &str,
    program: &[String],
) -> Result<String, NoPane> {
    let words = arguments(place, folder, label, program);
    let none = |why: String| NoPane { why, maybe: false };
    let ran = runner
        .run(PROGRAM, &words, None)
        .map_err(|why| none(format!("bateri could not be run: {why}")))?;
    if !ran.ok {
        return Err(none(ran.reason("bateri did not make the pane")));
    }
    let id = ran.out.trim();
    if is_pane(id) {
        Ok(id.to_owned())
    } else {
        Err(NoPane {
            why: "bateri did not say which pane it made".to_owned(),
            maybe: true,
        })
    }
}

/// Waits until the program of `pane` ends. How it ended is not asked: the mark on the issue goes
/// either way.
pub fn wait(runner: &dyn Runner, pane: &str) {
    let _ = runner.run(PROGRAM, &["wait".to_owned(), pane.to_owned()], None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_goes_to_the_right_and_the_program_goes_after_the_dashes() {
        let words = arguments(
            Place::Split,
            Path::new("/work/app"),
            "ACM-412",
            &["claude".to_owned(), "-x look".to_owned()],
        );
        assert_eq!(
            words,
            [
                "split",
                "--right",
                "--cwd",
                "/work/app",
                "--label",
                "ACM-412",
                "--",
                "claude",
                "-x look"
            ]
        );
        // A label that looks like an option is made one that does not.
        let odd = arguments(Place::Tab, Path::new("/w"), "--cwd", &["codex".to_owned()]);
        assert_eq!(odd[3..5], ["--label", "cwd"]);
        let tab = arguments(Place::Tab, Path::new("/w"), "ACM-1", &["codex".to_owned()]);
        assert_eq!(tab[0], "tab");
        assert!(!tab.contains(&"--right".to_owned()), "{tab:?}");
        assert!(!tab.contains(&"--focus".to_owned()));
    }

    #[test]
    fn only_a_pane_id_is_taken_as_the_answer() {
        assert!(is_pane("p41"));
        assert!(!is_pane("p"));
        assert!(!is_pane("41"));
        assert!(!is_pane("p4x"));
        assert!(!is_pane("pane 41"));
    }
}
