//! What the program keeps for the person between runs: the key, and the files of what it hands to
//! an agent. It knows nothing of the page; it takes and gives plain Rust values.
//!
//! Where things are is read once, in `main.rs`, from the environment, and given here as a lookup:
//! nothing of `state/` or `linear/` reads the environment itself, so a test of either is of the
//! directory it is given and never of the home of whoever runs it.

pub mod credentials;

use std::path::PathBuf;

/// The directories the program uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Paths {
    /// Where the key is: `$LINEARPEN_CONFIG_DIR`, else `$XDG_CONFIG_HOME/linearpen`, else
    /// `~/.config/linearpen`.
    pub config: Option<PathBuf>,
    /// Where what is handed to an agent is kept: `$XDG_STATE_HOME/linearpen`, else
    /// `~/.local/state/linearpen`.
    pub state: Option<PathBuf>,
}

impl Paths {
    /// The directories, given the way to look a variable up (the environment, in the program).
    pub fn resolve(get: &dyn Fn(&str) -> Option<String>) -> Self {
        let set = |name: &str| get(name).filter(|value| !value.is_empty());
        let under = |name: &str, rest: &[&str]| {
            set(name).map(|base| {
                rest.iter()
                    .fold(PathBuf::from(base), |p, part| p.join(part))
            })
        };
        let config = set("LINEARPEN_CONFIG_DIR")
            .map(PathBuf::from)
            .or_else(|| under("XDG_CONFIG_HOME", &["linearpen"]))
            .or_else(|| under("HOME", &[".config", "linearpen"]));
        let state = under("XDG_STATE_HOME", &["linearpen"])
            .or_else(|| under("HOME", &[".local", "state", "linearpen"]));
        Self { config, state }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(vars: &[(&str, &str)]) -> Paths {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Paths::resolve(&|name| vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone()))
    }

    #[test]
    fn the_config_directory_is_the_variable_then_xdg_then_home() {
        let all = [
            ("LINEARPEN_CONFIG_DIR", "/c"),
            ("XDG_CONFIG_HOME", "/x"),
            ("HOME", "/h"),
        ];
        assert_eq!(paths(&all).config, Some(PathBuf::from("/c")));
        assert_eq!(paths(&all[1..]).config, Some(PathBuf::from("/x/linearpen")));
        assert_eq!(
            paths(&all[2..]).config,
            Some(PathBuf::from("/h/.config/linearpen"))
        );
        assert_eq!(paths(&[]).config, None);
        // An empty variable is not set.
        assert_eq!(
            paths(&[("LINEARPEN_CONFIG_DIR", ""), ("HOME", "/h")]).config,
            Some(PathBuf::from("/h/.config/linearpen"))
        );
    }

    #[test]
    fn the_state_directory_is_xdg_then_home() {
        let all = [("XDG_STATE_HOME", "/s"), ("HOME", "/h")];
        assert_eq!(paths(&all).state, Some(PathBuf::from("/s/linearpen")));
        assert_eq!(
            paths(&all[1..]).state,
            Some(PathBuf::from("/h/.local/state/linearpen"))
        );
        assert_eq!(paths(&[]).state, None);
    }
}
