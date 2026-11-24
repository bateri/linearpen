//! What the person chose in the form that hands issues to an agent (which agent, which folder, where
//! the pane goes, whether the issues are moved), and how wide they left what stands beside the list,
//! kept between runs so that the form opens as it was left and the issue as wide. A file of four short
//! words and a number in the state directory, `0600`, written beside itself and renamed over, so that
//! it is never seen half-written. Nothing of Linear is in it, and no key.
//!
//! It is a convenience and never a reason for anything to fail: a file that is missing, too big, not
//! JSON or with anything in it that is not a short word (or, for the width, a whole number of points
//! in its range) is read as if it said nothing of that, and a file that cannot be written is let be.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// The file, in the state directory.
pub const FILE: &str = "prefs.json";

/// The most that is read of it: it holds four words and a number.
const MOST_BYTES: u64 = 4096;

/// What makes the name of the file written beside the file different for each save (two saves of
/// one process, at once, do not write to one file).
static SAVES: AtomicU64 = AtomicU64::new(0);

/// The longest a word is, in characters.
const MOST_CHARS: usize = 32;

/// The narrowest and the widest what stands beside the list is kept at, points: anything else is not
/// a width a page was left at.
pub const PANEL_LEAST: u32 = 200;
pub const PANEL_MOST: u32 = 4000;

/// The choices, each of which may be unknown. What a word means is for whoever asks: this holds no
/// more than that it is a short word of lowercase letters and dashes. `panel` is the width, in points,
/// that what stands beside the list was left at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Prefs {
    pub agent: Option<String>,
    pub folder: Option<String>,
    pub place: Option<String>,
    pub status: Option<String>,
    pub panel: Option<u32>,
}

/// Whether `width` is one a page was left at: a whole number of points from `PANEL_LEAST` to
/// `PANEL_MOST`.
#[must_use]
pub fn is_panel(width: u32) -> bool {
    (PANEL_LEAST..=PANEL_MOST).contains(&width)
}

/// Whether `text` is a word that is kept: one to `MOST_CHARS` lowercase letters or dashes.
fn is_word(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= MOST_CHARS
        && text.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

/// What the file in `state` says. Nothing is said of a choice that is not there or is not a word.
#[must_use]
pub fn load(state: &Path) -> Prefs {
    let path = state.join(FILE);
    // A file and no more: opening a pipe would wait for whoever writes to it.
    if !fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
        return Prefs::default();
    }
    let Ok(file) = fs::File::open(&path) else {
        return Prefs::default();
    };
    let mut text = String::new();
    // One byte more than the most: a file that is longer is told by it.
    if file.take(MOST_BYTES + 1).read_to_string(&mut text).is_err()
        || text.len() as u64 > MOST_BYTES
    {
        return Prefs::default();
    }
    let Ok(serde_json::Value::Object(members)) = serde_json::from_str(&text) else {
        return Prefs::default();
    };
    let word = |name: &str| {
        members
            .get(name)
            .and_then(serde_json::Value::as_str)
            .filter(|text| is_word(text))
            .map(str::to_owned)
    };
    let panel = members
        .get("panel")
        .and_then(serde_json::Value::as_u64)
        .and_then(|width| u32::try_from(width).ok())
        .filter(|width| is_panel(*width));
    Prefs {
        agent: word("agent"),
        folder: word("folder"),
        place: word("place"),
        status: word("status"),
        panel,
    }
}

/// Keeps `prefs` in `state` (made `0700` where it is made here): the words and the width that are
/// given, and none that is not.
///
/// # Errors
///
/// The words of why it could not be kept.
pub fn save(state: &Path, prefs: &Prefs) -> Result<(), String> {
    let fail = |why: std::io::Error, path: &Path| format!("{}: {why}", path.display());
    if !state.is_dir() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(state)
            .map_err(|why| fail(why, state))?;
    }
    let mut members = serde_json::Map::new();
    for (name, value) in [
        ("agent", &prefs.agent),
        ("folder", &prefs.folder),
        ("place", &prefs.place),
        ("status", &prefs.status),
    ] {
        if let Some(word) = value.as_deref().filter(|word| is_word(word)) {
            members.insert(name.to_owned(), serde_json::Value::from(word));
        }
    }
    if let Some(width) = prefs.panel.filter(|width| is_panel(*width)) {
        members.insert("panel".to_owned(), serde_json::Value::from(width));
    }
    let text = serde_json::to_string(&members).map_err(|why| why.to_string())?;
    let file = state.join(FILE);
    let beside = state.join(format!(
        "{FILE}.{}.{}.tmp",
        std::process::id(),
        SAVES.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_file(&beside);
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&beside)
        .and_then(|mut opened| {
            opened.write_all(text.as_bytes())?;
            opened.write_all(b"\n")?;
            opened.sync_all()
        })
        .and_then(|()| fs::rename(&beside, &file));
    written.map_err(|why| {
        let _ = fs::remove_file(&beside);
        fail(why, &file)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn prefs() -> Prefs {
        Prefs {
            agent: Some("codex".to_owned()),
            folder: Some("worktree".to_owned()),
            place: Some("tab".to_owned()),
            status: Some("keep".to_owned()),
            panel: Some(560),
        }
    }

    #[test]
    fn what_is_kept_is_read_back_and_the_file_is_the_persons_alone() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        save(&state, &prefs()).unwrap();
        assert_eq!(load(&state), prefs());
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&state.join(FILE)), 0o600);
        assert_eq!(mode(&state), 0o700, "a folder made here is the person's");
        // Nothing is left beside it, and a second save is over the first.
        let second = Prefs {
            agent: Some("claude".to_owned()),
            ..prefs()
        };
        save(&state, &second).unwrap();
        assert_eq!(load(&state), second);
        assert_eq!(fs::read_dir(&state).unwrap().count(), 1);
    }

    #[test]
    fn the_file_holds_the_four_words_and_the_width_and_nothing_of_linear_or_the_key() {
        let root = tempfile::tempdir().unwrap();
        save(root.path(), &prefs()).unwrap();
        let text = fs::read_to_string(root.path().join(FILE)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut names: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        names.sort();
        assert_eq!(names, ["agent", "folder", "panel", "place", "status"]);
        assert!(text.len() < 128, "{text}");
        // A word that is not a short word of letters is not written, whoever gave it, nor a width out
        // of its range.
        let odd = Prefs {
            agent: Some("lin_api_0123456789".to_owned()),
            folder: Some("x".repeat(33)),
            place: Some("a b".to_owned()),
            status: Some(String::new()),
            panel: Some(PANEL_MOST + 1),
        };
        save(root.path(), &odd).unwrap();
        assert_eq!(fs::read_to_string(root.path().join(FILE)).unwrap(), "{}\n");
    }

    #[test]
    fn a_file_that_is_missing_broken_or_too_big_says_nothing() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()), Prefs::default(), "missing");
        assert_eq!(load(&root.path().join("none")), Prefs::default());
        let put = |text: &str| fs::write(root.path().join(FILE), text).unwrap();
        for broken in [
            "",
            "not json",
            "[]",
            "3",
            "null",
            "{\"agent\":",
            "{\"agent\": \"claude\"",
        ] {
            put(broken);
            assert_eq!(load(root.path()), Prefs::default(), "{broken:?}");
        }
        put(&format!(
            "{{\"agent\":\"claude\",\"pad\":\"{}\"}}",
            "x".repeat(5000)
        ));
        assert_eq!(load(root.path()), Prefs::default(), "too big");
        // A file that is not text.
        fs::write(root.path().join(FILE), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(load(root.path()), Prefs::default());
        // A directory where the file is.
        fs::remove_file(root.path().join(FILE)).unwrap();
        fs::create_dir(root.path().join(FILE)).unwrap();
        assert_eq!(load(root.path()), Prefs::default());
    }

    #[test]
    fn a_pipe_where_the_file_is_says_nothing_and_does_not_wait() {
        let root = tempfile::tempdir().unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(root.path().join(FILE))
            .status()
            .expect("mkfifo runs");
        assert!(made.success());
        // (Were the pipe opened, this would not return.)
        assert_eq!(load(root.path()), Prefs::default());
    }

    #[test]
    fn saves_at_once_do_not_share_a_file_and_the_last_one_is_whole() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().to_owned();
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let state = state.clone();
                std::thread::spawn(move || {
                    let prefs = Prefs {
                        agent: Some(["claude", "codex", "cursor"][n % 3].to_owned()),
                        place: Some("tab".to_owned()),
                        ..Prefs::default()
                    };
                    for _ in 0..20 {
                        save(&state, &prefs).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let got = load(&state);
        assert!(
            got.agent.is_some() && got.place.as_deref() == Some("tab"),
            "{got:?}"
        );
        assert_eq!(
            fs::read_dir(&state).unwrap().count(),
            1,
            "no file is left beside"
        );
    }

    #[test]
    fn a_member_that_is_not_a_short_word_is_let_go_and_the_others_are_kept() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join(FILE),
            r#"{ "agent": 3, "folder": "worktree", "place": ["tab"], "status": "Keep Going",
                 "panel": "560", "other": "x", "more": { "agent": "codex" } }"#,
        )
        .unwrap();
        assert_eq!(
            load(root.path()),
            Prefs {
                folder: Some("worktree".to_owned()),
                ..Prefs::default()
            }
        );
    }

    #[test]
    fn a_width_is_kept_only_as_a_whole_number_of_points_in_its_range() {
        let root = tempfile::tempdir().unwrap();
        let put = |text: &str| fs::write(root.path().join(FILE), text).unwrap();
        for (text, panel) in [
            (r#"{ "panel": 560 }"#, Some(560)),
            (r#"{ "panel": 200 }"#, Some(PANEL_LEAST)),
            (r#"{ "panel": 4000 }"#, Some(PANEL_MOST)),
            (r#"{ "panel": 199 }"#, None),
            (r#"{ "panel": 4001 }"#, None),
            (r#"{ "panel": -560 }"#, None),
            (r#"{ "panel": 560.5 }"#, None),
            (r#"{ "panel": 99999999999 }"#, None),
            (r#"{ "panel": null }"#, None),
        ] {
            put(text);
            assert_eq!(load(root.path()).panel, panel, "{text}");
        }
        // Saving the form's words keeps the width that was there, as `start` does it (load, then save).
        put(r#"{ "panel": 640, "agent": "codex" }"#);
        let mut kept = load(root.path());
        kept.agent = Some("claude".to_owned());
        save(root.path(), &kept).unwrap();
        assert_eq!(load(root.path()).panel, Some(640));
    }

    #[test]
    fn a_state_folder_that_cannot_be_made_is_an_error_and_not_a_panic() {
        let root = tempfile::tempdir().unwrap();
        let blocked = root.path().join("file");
        fs::write(&blocked, "a file").unwrap();
        assert!(save(&blocked.join("state"), &prefs()).is_err());
        assert!(save(&blocked, &prefs()).is_err());
    }
}
