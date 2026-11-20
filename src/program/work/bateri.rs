//! bateri's command line, used to make a pane that runs the agent and to know when it ends:
//! `bateri split --right` (or `bateri tab`) with the folder, a label and the program after `--`,
//! which tells the id of the pane it made; `bateri wait` on that id, which returns when the pane's
//! program ends. The new pane is not given the focus (no `--focus`). `bateri ls --json` says when each
//! terminal last wrote (`output_at`), which is how a pane that has stopped is told from one at work.
//! `bateri focus pane` on that id takes the keyboard to it, when the person asks to go to the agent.
//! The same list says the program and the name of each terminal, which is how the agents a pen that
//! has since ended started are found again: the label is the issue's identifier.

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

/// Whether `word` is the id of a pane of this machine (`p41`; not `p9@prod`).
#[must_use]
pub fn is_pane(word: &str) -> bool {
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
        .run_with(PROGRAM, &words, None, &[])
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

/// A terminal as `bateri ls --json` tells it, and only what this program reads of it: the list holds
/// much more of each (its folder, its size, its exit) that is no concern of this program.
#[derive(Debug, Clone, PartialEq)]
pub struct Terminal {
    /// The id of its pane (`p41`).
    pub id: String,
    /// Its program has not ended (`"state": "running"`).
    pub running: bool,
    /// The program in front of it (`claude`), where bateri says.
    pub program: Option<String>,
    /// The name it was given: its own, which `split --label` gives, else the one of the tab it is
    /// in, which is where `tab --label` puts it.
    pub label: Option<String>,
    /// When it last wrote, seconds since the epoch, where bateri says.
    pub output_at: Option<f64>,
    /// When its program started, seconds since the epoch, where bateri says.
    pub started: Option<u64>,
}

/// The terminals of bateri, or nothing where the list cannot be had or read.
#[must_use]
pub fn terminals(runner: &dyn Runner) -> Option<Vec<Terminal>> {
    let ran = runner
        .run(PROGRAM, &["ls".to_owned(), "--json".to_owned()], None)
        .ok()
        .filter(|ran| ran.ok)?;
    parse_terminals(&ran.out)
}

/// When each terminal last wrote, by the id of the pane. A terminal that has no `output_at` or one
/// that is not a number is not here, and a list that cannot be read is nothing: both say nothing of
/// any pane, and a pane of which nothing is said is taken to be at work.
#[must_use]
pub fn output_times(runner: &dyn Runner) -> Option<HashMap<String, f64>> {
    terminals(runner).map(|terminals| times(&terminals))
}

fn times(terminals: &[Terminal]) -> HashMap<String, f64> {
    terminals
        .iter()
        .filter_map(|terminal| Some((terminal.id.clone(), terminal.output_at?)))
        .collect()
}

/// The terminals of the list: each that has an id. A label that is only blanks is none. The tabs
/// are in `windows`, which is `null` where the bateri app is not connected, and a window or tab of
/// another machine is `{"elsewhere": true}` with no id: a terminal in no tab that is told has only
/// its own label.
fn parse_terminals(text: &str) -> Option<Vec<Terminal>> {
    use serde_json::Value;
    let value: Value = serde_json::from_str(text).ok()?;
    let text_of = |value: &Value, key: &str| -> Option<String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let tab_labels: HashMap<String, String> = value
        .get("windows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|window| window.get("tabs").and_then(Value::as_array))
        .flatten()
        .filter_map(|tab| Some((text_of(tab, "id")?, text_of(tab, "label")?)))
        .collect();
    let terminals = value
        .get("terminals")?
        .as_array()?
        .iter()
        .filter_map(|terminal| {
            let id = terminal.get("id").and_then(Value::as_str)?.to_owned();
            let label = text_of(terminal, "label")
                .or_else(|| text_of(terminal, "tab").and_then(|tab| tab_labels.get(&tab).cloned()));
            Some(Terminal {
                id,
                running: terminal.get("state").and_then(Value::as_str) == Some("running"),
                program: text_of(terminal, "program"),
                label,
                output_at: terminal
                    .get("output_at")
                    .and_then(Value::as_f64)
                    .filter(|at| at.is_finite()),
                started: terminal.get("started").and_then(Value::as_u64),
            })
        })
        .collect();
    Some(terminals)
}

/// Takes the keyboard to `pane`, the id `launch` said. bateri does it only from the pane the person
/// types in, and says why where it does not (the pane has ended, the keyboard is elsewhere).
///
/// # Errors
///
/// bateri's words of why it did not, or ours where it could not be run.
pub fn focus(runner: &dyn Runner, pane: &str) -> Result<(), String> {
    if !is_pane(pane) {
        return Err("the agent's pane is not known".to_owned());
    }
    let words = ["focus".to_owned(), "pane".to_owned(), pane.to_owned()];
    let ran = runner
        .run(PROGRAM, &words, None)
        .map_err(|why| format!("bateri could not be run: {why}"))?;
    if ran.ok {
        Ok(())
    } else {
        Err(ran.reason("bateri did not go to the agent's pane"))
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

    fn parse_output_times(text: &str) -> Option<HashMap<String, f64>> {
        parse_terminals(text).map(|terminals| times(&terminals))
    }

    /// A list as bateri writes it (the keys of 0.27, trimmed): an agent in a tab of its own, whose
    /// name is the tab's (`tab --label`), one split beside the pen, whose name is its own
    /// (`split --label`), a shell in a tab named by hand, and a pane that ended.
    const LISTED: &str = r#"{"caller":"p491","typing":"p491","windows":[
        {"id":"w1","visible":true,"groups":[],"tabs":[
          {"id":"t110","group":null,"keys":"⌘1","label":null,"title":"linearpen","selected":false,
           "zoomed":null,"tree":{"pane":{"id":"p491"}}},
          {"id":"t128","group":null,"keys":"⌘2","label":"ACM-412","title":"ACM-412 Linear issue",
           "selected":true,"zoomed":null,"tree":{"pane":{"id":"p750"}}},
          {"id":"t130","group":null,"keys":null,"label":"  ","title":"zsh","selected":false,
           "zoomed":null,"tree":{"pane":{"id":"p760"}}}]},
        {"elsewhere":true}],
      "terminals":[
        {"id":"p491","state":"running","activity":"running","program":"linearpen","directory":"/w",
         "cols":151,"rows":31,"exit":null,"secret":false,"label":null,"started":1763500000,
         "front_started":null,"front_is_own":false,"output_at":1763560000,"opened_by":null,
         "placed":"tab","window":"w1","group":null,"tab":"t110"},
        {"id":"p750","state":"running","activity":"running","program":"claude","directory":"/w",
         "cols":151,"rows":34,"exit":null,"secret":false,"label":null,"started":1763500100,
         "front_started":1763500100,"front_is_own":true,"output_at":1763560001.5,
         "opened_by":"p491","placed":"tab","window":"w1","group":null,"tab":"t128"},
        {"id":"p751","state":"running","activity":"running","program":"codex","directory":"/w",
         "cols":70,"rows":31,"exit":null,"secret":false,"label":"ACM-7","started":1763500200,
         "front_started":1763500200,"front_is_own":true,"output_at":null,"opened_by":"p491",
         "placed":"tab","window":"w1","group":null,"tab":"t110"},
        {"id":"p760","state":"running","activity":"idle","program":"zsh","label":null,"tab":"t130"},
        {"id":"p697","state":"ended","activity":"exited","program":null,"exit":{"signal":9},
         "label":"ACM-20","placed":"list","window":null,"group":null,"tab":null}
      ]}"#;

    #[test]
    fn a_terminal_is_named_by_its_own_label_else_by_its_tabs() {
        let terminals = parse_terminals(LISTED).unwrap();
        let of = |id: &str| terminals.iter().find(|t| t.id == id).unwrap().clone();
        let tabbed = of("p750");
        assert!(tabbed.running);
        assert_eq!(tabbed.program.as_deref(), Some("claude"));
        assert_eq!(tabbed.label.as_deref(), Some("ACM-412"));
        assert_eq!(tabbed.output_at, Some(1_763_560_001.5));
        assert_eq!(tabbed.started, Some(1_763_500_100));
        // Split beside the pen: its own name, not the pen's tab's (which has none).
        assert_eq!(of("p751").label.as_deref(), Some("ACM-7"));
        assert_eq!(of("p751").output_at, None);
        // A tab's blank name is no name; nor is a pen's tab that has none.
        assert_eq!(of("p760").label, None);
        assert_eq!(of("p491").label, None);
        let ended = of("p697");
        assert!(!ended.running);
        assert_eq!(
            (ended.program, ended.label.as_deref()),
            (None, Some("ACM-20"))
        );
        // Only p491 and p750 say when they last wrote.
        assert_eq!(times(&terminals).len(), 2);
    }

    #[test]
    fn where_the_app_is_away_only_the_terminals_own_names_are_known() {
        let away = r#"{"windows":null,"terminals":[
            {"id":"p750","state":"running","program":"claude","label":null,"tab":"t128"},
            {"id":"p751","state":"running","program":"codex","label":"ACM-7","tab":null}]}"#;
        let terminals = parse_terminals(away).unwrap();
        assert_eq!(terminals[0].label, None);
        assert_eq!(terminals[1].label.as_deref(), Some("ACM-7"));
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

    /// A runner that answers every program with `ok` and `err`, and keeps what it was asked.
    struct Answers {
        ok: bool,
        err: &'static str,
        asked: std::sync::Mutex<Vec<String>>,
    }

    impl Runner for Answers {
        fn run_with(
            &self,
            program: &str,
            args: &[String],
            _: Option<&Path>,
            _: &[(&str, &str)],
        ) -> std::io::Result<super::super::runner::Ran> {
            self.asked
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            Ok(super::super::runner::Ran {
                ok: self.ok,
                out: String::new(),
                err: self.err.to_owned(),
            })
        }
    }

    fn answers(ok: bool, err: &'static str) -> Answers {
        Answers {
            ok,
            err,
            asked: std::sync::Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn going_to_a_pane_is_bateri_focus_pane_and_its_words_where_it_does_not() {
        let fine = answers(true, "");
        assert_eq!(focus(&fine, "p41"), Ok(()));
        assert_eq!(*fine.asked.lock().unwrap(), ["bateri focus pane p41"]);
        let refused = answers(false, "the keyboard is in another pane\n");
        assert_eq!(
            focus(&refused, "p41"),
            Err("the keyboard is in another pane".to_owned())
        );
        let quiet = answers(false, "");
        assert_eq!(
            focus(&quiet, "p41"),
            Err("bateri did not go to the agent's pane".to_owned())
        );
        // What is not a pane's id is not given to bateri.
        let never = answers(true, "");
        for odd in ["", "41", "--help", "p4 x", "p9@prod"] {
            assert!(focus(&never, odd).is_err(), "{odd:?}");
        }
        assert!(never.asked.lock().unwrap().is_empty());
    }

    #[test]
    fn only_a_pane_id_is_taken_as_the_answer() {
        assert!(is_pane("p41"));
        assert!(!is_pane("p9@prod"));
        assert!(!is_pane("p"));
        assert!(!is_pane("41"));
        assert!(!is_pane("p4x"));
        assert!(!is_pane("pane 41"));
    }
}
