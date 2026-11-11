//! bateri's command line, used to make a pane that runs the agent and to know when it ends:
//! `bateri split --right` (or `bateri tab`) with the folder, a label and the program after `--`,
//! which tells the id of the pane it made; `bateri wait` on that id, which returns when the pane's
//! program ends. The new pane is not given the focus (no `--focus`). `bateri ls --json` says when each
//! terminal last wrote (`output_at`), which is how a pane that has stopped is told from one at work.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

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

/// How long a pane has to be silent before the agent in it is taken to have stopped: what the agents
/// write while they work is, measured, a line or more each second, and what they wait for the person
/// with they do not write at all.
pub const IDLE_AFTER: Duration = Duration::from_secs(10);

/// Whether a pane that last wrote at `output_at` (seconds since the epoch) has been silent for longer
/// than `IDLE_AFTER` at `now`. A moment in the future (the clocks differ) is not silence.
#[must_use]
pub fn idle(output_at: f64, now: f64) -> bool {
    now - output_at > IDLE_AFTER.as_secs_f64()
}

/// When each terminal last wrote, by the id of the pane: what `bateri ls --json` says, which holds
/// much more of each terminal (its program, its state, its label) that is no concern of this program.
/// A terminal that has no `output_at` or one that is not a number is not here, and a list that
/// cannot be read is nothing: both say nothing of any pane, and a pane of which nothing is said is
/// taken to be at work.
#[must_use]
pub fn output_times(runner: &dyn Runner) -> Option<HashMap<String, f64>> {
    let ran = runner
        .run(PROGRAM, &["ls".to_owned(), "--json".to_owned()], None)
        .ok()
        .filter(|ran| ran.ok)?;
    parse_output_times(&ran.out)
}

fn parse_output_times(text: &str) -> Option<HashMap<String, f64>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let mut times = HashMap::new();
    for terminal in value.get("terminals")?.as_array()? {
        let id = terminal.get("id").and_then(serde_json::Value::as_str);
        let at = terminal
            .get("output_at")
            .and_then(serde_json::Value::as_f64)
            .filter(|at| at.is_finite());
        if let (Some(id), Some(at)) = (id, at) {
            times.insert(id.to_owned(), at);
        }
    }
    Some(times)
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
    fn the_times_are_read_from_the_list_of_terminals_and_what_else_it_holds_is_let_be() {
        let text = r#"{ "terminals": [
            { "id": "p41", "state": "running", "activity": "running", "program": "claude",
              "exit": null, "output_at": 1762870000, "label": "ACM-412", "more": { "a": [1] } },
            { "id": "p42", "state": "ended", "activity": "exited", "exit": 0, "output_at": 1762870000.5 }
        ], "version": "0.27.0" }"#;
        let times = parse_output_times(text).unwrap();
        assert_eq!(times.len(), 2);
        assert!((times["p41"] - 1_762_870_000.0).abs() < 1e-6);
        assert!((times["p42"] - 1_762_870_000.5).abs() < 1e-6);
    }

    #[test]
    fn a_terminal_without_a_time_says_nothing_and_a_list_that_is_not_one_is_nothing() {
        let text = r#"{ "terminals": [
            { "id": "p1" }, { "id": "p2", "output_at": null }, { "id": "p3", "output_at": "soon" },
            { "output_at": 5 }, { "id": 7, "output_at": 5 }, 3, { "id": "p4", "output_at": 9 }
        ] }"#;
        let times = parse_output_times(text).unwrap();
        assert_eq!(times.keys().collect::<Vec<_>>(), ["p4"]);
        assert_eq!(parse_output_times(r#"{"terminals":[]}"#).unwrap().len(), 0);
        for broken in [
            "",
            "not json",
            "[]",
            "{}",
            r#"{"terminals":3}"#,
            r#"{"terminals":{}}"#,
        ] {
            assert!(parse_output_times(broken).is_none(), "{broken}");
        }
    }

    #[test]
    fn a_pane_is_idle_after_ten_silent_seconds_and_not_before() {
        let now = 1_762_870_100.0;
        assert!(!idle(now, now));
        assert!(!idle(now - 9.9, now));
        assert!(!idle(now - 10.0, now));
        assert!(idle(now - 10.5, now));
        assert!(idle(now - 3_600.0, now));
        // A time that is to come is the clocks' difference, not a silence.
        assert!(!idle(now + 30.0, now));
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
