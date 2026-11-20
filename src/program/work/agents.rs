//! The agents the program can start: which of them are installed, and the command that starts one.
//! An agent is found in the `PATH` and is run with its prompt as its last argument, and with no flag
//! that lets it do what it would ask about. Claude alone is given one more word before the prompt,
//! the name of its session (`--name=`), which is the issue's identifier and title: what it shows in
//! its prompt box, its `/resume` list and the terminal's title. The name is Linear's text in a
//! command line, so it is one word, has no control character and is short (see [`session_name`]).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

impl Agent {
    /// In the order the page lists them.
    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Cursor];

    /// The name the page and the calls use.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }

    /// What the person is told.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
        }
    }

    /// The program that is run.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor-agent",
        }
    }

    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.id() == id)
    }

    /// The agent whose program `program` is (the name bateri says of the program in a pane).
    #[must_use]
    pub fn by_program(program: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|agent| agent.program() == program)
    }

    /// The command that starts the agent on `prompt`: the program, for Claude the name of its
    /// session where there is one (`--name=<name>`, one word, so a name that begins with a dash is
    /// not taken for an option), and the prompt. The other agents are given no name.
    #[must_use]
    pub fn command(self, name: Option<&str>, prompt: &str) -> Vec<String> {
        let mut words = vec![self.program().to_owned()];
        if let (Self::Claude, Some(name)) = (self, name) {
            words.push(format!("--name={name}"));
        }
        words.push(prompt.to_owned());
        words
    }
}

/// The most characters of the name of an agent's session.
pub const NAME_MOST: usize = 100;

/// The name of the session of the agent started on an issue: `ACM-412 - Add rate limiting`, one of
/// the two where the other is empty, and none where both are. Both are Linear's: a control character
/// (a line end, an escape that a terminal's title would take for a command) is made a space, every
/// run of blank one space, and a name longer than [`NAME_MOST`] is cut and ends with `…`.
#[must_use]
pub fn session_name(identifier: &str, title: &str) -> Option<String> {
    let clean = |text: &str| {
        text.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let name = [clean(identifier), clean(title)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" - ");
    if name.is_empty() {
        return None;
    }
    if name.chars().count() <= NAME_MOST {
        return Some(name);
    }
    let kept: String = name.chars().take(NAME_MOST - 1).collect();
    Some(format!("{}…", kept.trim_end()))
}

/// Whether `program` is a file that can be run in one of the directories of the `PATH`.
#[must_use]
pub fn on_path(path: &[PathBuf], program: &str) -> bool {
    path.iter().any(|dir| runnable(&dir.join(program)))
}

fn runnable(file: &Path) -> bool {
    std::fs::metadata(file)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The agents installed, in the order of `Agent::ALL`.
#[must_use]
pub fn installed(path: &[PathBuf]) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|agent| on_path(path, agent.program()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn program(dir: &Path, name: &str, mode: u32) {
        let file = dir.join(name);
        fs::write(&file, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn the_agents_installed_are_the_ones_found_as_programs_in_the_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        program(first.path(), "cursor-agent", 0o755);
        program(second.path(), "claude", 0o755);
        // A file that cannot be run is not an agent.
        program(second.path(), "codex", 0o644);
        let path = vec![first.path().to_owned(), second.path().to_owned()];
        assert_eq!(installed(&path), vec![Agent::Claude, Agent::Cursor]);
        assert!(on_path(&path, "claude"));
        assert!(!on_path(&path, "codex"));
        assert!(installed(&[]).is_empty());
    }

    #[test]
    fn an_agent_is_started_with_its_prompt_and_only_claude_is_given_a_name() {
        assert_eq!(
            Agent::Cursor.command(Some("ACM-412 - Add rate limiting"), "Work on it"),
            vec!["cursor-agent".to_owned(), "Work on it".to_owned()]
        );
        assert_eq!(
            Agent::Codex.command(Some("ACM-412 - Add rate limiting"), "x"),
            ["codex", "x"]
        );
        assert_eq!(
            Agent::Claude.command(Some("ACM-412 - Add rate limiting"), "Work on it"),
            ["claude", "--name=ACM-412 - Add rate limiting", "Work on it"]
        );
        // A name that begins with a dash is still the value of `--name`, in the same word.
        assert_eq!(
            Agent::Claude.command(Some("--dangerously-skip-permissions"), "x"),
            ["claude", "--name=--dangerously-skip-permissions", "x"]
        );
        for agent in Agent::ALL {
            let words = agent.command(None, "x");
            assert_eq!(words, [agent.program(), "x"]);
            assert_eq!(Agent::parse(agent.id()), Some(agent));
        }
        assert_eq!(Agent::parse("gemini"), None);
    }

    #[test]
    fn the_name_is_the_identifier_and_the_title_on_one_line_and_short() {
        assert_eq!(
            session_name("ACM-412", "Add rate limiting").as_deref(),
            Some("ACM-412 - Add rate limiting")
        );
        // One of the two where the other is empty, and none where both are.
        assert_eq!(session_name("", " Add  it ").as_deref(), Some("Add it"));
        assert_eq!(session_name("ACM-412", " \n").as_deref(), Some("ACM-412"));
        assert_eq!(session_name(" ", "\t\u{7}"), None);
        // A control character is a space: no line end, escape or bell reaches the terminal's title.
        let odd = session_name("ACM-1", "Fix\nthe\r\nlogin\u{1b}]0;owned\u{7} page\t now").unwrap();
        assert_eq!(odd, "ACM-1 - Fix the login ]0;owned page now");
        assert!(!odd.chars().any(char::is_control), "{odd:?}");
        // A long title is cut, by characters, and says it was.
        let long = session_name("ACM-2", &"ş".repeat(300)).unwrap();
        assert_eq!(long.chars().count(), NAME_MOST);
        assert!(
            long.starts_with("ACM-2 - şş") && long.ends_with("ş…"),
            "{long}"
        );
        // A blank where it is cut does not stay before the `…`.
        assert_eq!(
            session_name("ACM-30", &"word ".repeat(40)).unwrap(),
            format!("ACM-30 - {}…", "word ".repeat(18).trim_end())
        );
        // Exactly as long as it may be is not cut.
        let full = "x".repeat(NAME_MOST - "ACM-4 - ".len());
        assert_eq!(
            session_name("ACM-4", &full).unwrap(),
            format!("ACM-4 - {full}")
        );
    }
}
