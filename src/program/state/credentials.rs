//! The key: where it is looked for, how it is read and how it is kept.
//!
//! In the environment (`LINEAR_API_KEY`), and failing that in the file `key` of the config
//! directory. The Connect screen tries a key with a question to Linear and, if Linear accepts it,
//! keeps it in that file: the directory `0700` where it is made, the file `0600`, written beside
//! the file and renamed over it so that it is never seen half-written or wider than that. A file
//! that other users can read is read all the same, and said so; its mode is not narrowed.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::linear::{Key, KeyError};

/// The file of the key, in the config directory.
pub const FILE: &str = "key";

/// Where a key came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Environment,
    File(PathBuf),
}

/// A key that was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub key: Key,
    pub source: Source,
    /// The file can be read by other users: its mode has a bit of the group's or the others'.
    pub loose: bool,
}

/// Why there is no key to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// Not in the environment and not in the file (or there is no place for a file).
    Missing { file: Option<PathBuf> },
    /// There is a key, and it is not one.
    Unusable { source: Source, why: KeyError },
    /// The file is there and cannot be read.
    Unreadable { file: PathBuf, why: String },
}

/// The file of the key in `config`.
#[must_use]
pub fn file_in(config: &Path) -> PathBuf {
    config.join(FILE)
}

/// The key in `from_env` (the value of `LINEAR_API_KEY`), or else the one in the file of `config`.
pub fn find(from_env: Option<&str>, config: Option<&Path>) -> Result<Found, Problem> {
    if let Some(text) = from_env.filter(|text| !text.trim().is_empty()) {
        return Key::new(text)
            .map(|key| Found {
                key,
                source: Source::Environment,
                loose: false,
            })
            .map_err(|why| Problem::Unusable {
                source: Source::Environment,
                why,
            });
    }
    let Some(file) = config.map(file_in) else {
        return Err(Problem::Missing { file: None });
    };
    let mut opened = match fs::File::open(&file) {
        Ok(opened) => opened,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            return Err(Problem::Missing { file: Some(file) });
        }
        Err(why) => {
            return Err(Problem::Unreadable {
                file,
                why: why.to_string(),
            });
        }
    };
    let unreadable = |why: std::io::Error, file: &Path| Problem::Unreadable {
        file: file.to_owned(),
        why: why.to_string(),
    };
    let loose = opened
        .metadata()
        .map_err(|why| unreadable(why, &file))?
        .permissions()
        .mode()
        & 0o077
        != 0;
    // A key is some 50 characters: what is far past that is not one, and is not read whole.
    let mut text = String::new();
    (&mut opened)
        .take(1024)
        .read_to_string(&mut text)
        .map_err(|why| unreadable(why, &file))?;
    if text.trim().is_empty() {
        return Err(Problem::Missing { file: Some(file) });
    }
    Key::new(&text)
        .map(|key| Found {
            key,
            source: Source::File(file.clone()),
            loose,
        })
        .map_err(|why| Problem::Unusable {
            source: Source::File(file),
            why,
        })
}

/// Keeps `key` in the file of `config`, which is made `0600` (and its directory `0700`, if it is
/// made here). Gives the file's path.
#[allow(dead_code)] // The Connect call of the page keeps the key with it.
pub fn save(config: &Path, key: &Key) -> Result<PathBuf, String> {
    let fail = |why: std::io::Error, path: &Path| format!("{}: {why}", path.display());
    if !config.is_dir() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(config)
            .map_err(|why| fail(why, config))?;
    }
    let file = file_in(config);
    let beside = config.join(format!("{FILE}.tmp"));
    let _ = fs::remove_file(&beside);
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&beside)
        .and_then(|mut opened| {
            opened.write_all(key.expose().as_bytes())?;
            opened.write_all(b"\n")?;
            opened.sync_all()
        })
        .and_then(|()| fs::rename(&beside, &file));
    written.map_err(|why| {
        let _ = fs::remove_file(&beside);
        fail(why, &file)
    })?;
    Ok(file)
}

impl Problem {
    /// What to tell the person, which says where a key can be given.
    #[must_use]
    pub fn words(&self) -> String {
        const WHERE: &str =
            "Make a personal API key with Read permission in Linear (Settings, Account, Security).";
        match self {
            Self::Missing { file: Some(file) } => format!(
                "linearpen has no Linear API key. Set LINEAR_API_KEY, or put the key in {}.\n{WHERE}",
                file.display()
            ),
            Self::Missing { file: None } => {
                format!("linearpen has no Linear API key. Set LINEAR_API_KEY.\n{WHERE}")
            }
            Self::Unusable { source, why } => {
                format!(
                    "The Linear API key in {} is not usable: {why}.",
                    source.words()
                )
            }
            Self::Unreadable { file, why } => {
                format!(
                    "The Linear API key file {} cannot be read: {why}.",
                    file.display()
                )
            }
        }
    }
}

impl Source {
    /// Where the key is, for the person.
    #[must_use]
    pub fn words(&self) -> String {
        match self {
            Self::Environment => "LINEAR_API_KEY".to_owned(),
            Self::File(file) => file.display().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("linearpen-credentials-")
            .tempdir()
            .unwrap()
    }

    fn write(path: &Path, text: &str, mode: u32) {
        fs::write(path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn the_environment_comes_first_and_the_file_is_not_read() {
        let home = dir();
        write(&home.path().join(FILE), "lin_api_file\n", 0o600);
        let found = find(Some("lin_api_env"), Some(home.path())).unwrap();
        assert_eq!(found.key.expose(), "lin_api_env");
        assert_eq!(found.source, Source::Environment);
    }

    #[test]
    fn an_empty_variable_falls_to_the_file() {
        let home = dir();
        write(&home.path().join(FILE), "lin_api_file\n", 0o600);
        let found = find(Some("  "), Some(home.path())).unwrap();
        assert_eq!(found.key.expose(), "lin_api_file");
        assert_eq!(found.source, Source::File(home.path().join(FILE)));
        assert!(!found.loose);
    }

    #[test]
    fn no_key_anywhere_is_missing_and_says_where_it_goes() {
        let home = dir();
        let problem = find(None, Some(home.path())).unwrap_err();
        assert_eq!(
            problem,
            Problem::Missing {
                file: Some(home.path().join(FILE))
            }
        );
        let words = problem.words();
        assert!(words.contains("LINEAR_API_KEY"));
        assert!(words.contains(&home.path().join(FILE).display().to_string()));
        assert_eq!(find(None, None), Err(Problem::Missing { file: None }));
    }

    #[test]
    fn an_empty_file_is_missing() {
        let home = dir();
        write(&home.path().join(FILE), "\n", 0o600);
        assert!(matches!(
            find(None, Some(home.path())),
            Err(Problem::Missing { .. })
        ));
    }

    #[test]
    fn a_file_that_others_can_read_is_read_and_called_loose_and_not_narrowed() {
        let home = dir();
        let file = home.path().join(FILE);
        write(&file, "lin_api_file\n", 0o644);
        let found = find(None, Some(home.path())).unwrap();
        assert!(found.loose);
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn a_key_that_is_not_one_is_unusable_and_the_words_do_not_hold_it() {
        let home = dir();
        write(&home.path().join(FILE), "has space and secret\n", 0o600);
        let problem = find(None, Some(home.path())).unwrap_err();
        assert!(matches!(problem, Problem::Unusable { .. }));
        assert!(!problem.words().contains("secret"));
        let problem = find(Some("a b secret"), None).unwrap_err();
        assert!(!problem.words().contains("secret"));
    }

    #[test]
    fn saving_makes_the_directory_0700_and_the_file_0600_and_it_reads_back() {
        let home = dir();
        let config = home.path().join("a").join("linearpen");
        let key = Key::new("lin_api_saved").unwrap();
        let file = save(&config, &key).unwrap();
        assert_eq!(file, config.join(FILE));
        assert_eq!(
            fs::metadata(&config).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "lin_api_saved\n");
        assert!(!config.join("key.tmp").exists());
        assert_eq!(find(None, Some(&config)).unwrap().key, key);
    }

    #[test]
    fn saving_over_a_wide_file_leaves_a_0600_one_and_an_existing_directory_alone() {
        let home = dir();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o755)).unwrap();
        write(&home.path().join(FILE), "lin_api_old\n", 0o644);
        let key = Key::new("lin_api_new").unwrap();
        let file = save(home.path(), &key).unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(home.path()).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(fs::read_to_string(&file).unwrap(), "lin_api_new\n");
    }
}
